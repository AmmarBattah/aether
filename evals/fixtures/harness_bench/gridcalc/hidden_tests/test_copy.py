from support import GridTest, error, sheet


class CopyTests(GridTest):
    def test_relative_parts_shift_and_dollar_parts_stay(self):
        made = sheet({"A1": "=B1+$B1+B$1+$B$1"})
        made.copy("A1", "C3")
        self.assertEqual(made.raw("C3"), "=D3+$B3+D$1+$B$1")
        self.assertEqual(made.raw("A1"), "=B1+$B1+B$1+$B$1")

    def test_the_copy_is_evaluated_at_its_new_place(self):
        made = sheet({"A1": 1, "A2": 2, "B1": "=A1*10"})
        made.copy("B1", "B2")
        self.assertValue(made.get("B2"), 20)
        self.assertEqual(made.precedents("B2"), ["A2"])

    def test_literals_and_empty_cells_copy(self):
        made = sheet({"A1": "word", "A2": 2.5, "B1": 9})
        made.copy("A1", "C1")
        made.copy("a2", "c2")
        made.copy("D9", "B1")
        self.assertValue(made.get("C1"), "word")
        self.assertValue(made.get("C2"), 2.5)
        self.assertEqual(made.cells(), ["A1", "C1", "A2", "C2"])

    def test_a_reference_shifted_off_the_grid_becomes_ref_error(self):
        made = sheet({"A1": 1, "B2": "=A1+B1"})
        made.copy("B2", "A2")
        self.assertEqual(made.raw("A2"), "=#REF!+A1")
        self.assertValue(made.get("A2"), error("#REF!"))

    def test_a_reference_shifted_past_the_last_column_becomes_ref_error(self):
        made = sheet({"A1": "=B1"})
        made.copy("A1", "ZY1")
        made.copy("A1", "ZZ1")
        self.assertEqual(made.raw("ZY1"), "=ZZ1")
        self.assertEqual(made.raw("ZZ1"), "=#REF!")

    def test_a_range_shifts_as_a_whole(self):
        made = sheet({"B3": 1, "C4": 2, "C1": "=SUM(A1:B2)"})
        made.copy("C1", "D3")
        self.assertEqual(made.raw("D3"), "=SUM(B3:C4)")
        self.assertValue(made.get("D3"), 3)

    def test_a_range_with_one_corner_off_the_grid_becomes_ref_error(self):
        made = sheet({"C2": "=SUM(A1:B2)+A2"})
        made.copy("C2", "C1")
        self.assertEqual(made.raw("C1"), "=SUM(#REF!)+A1")
        self.assertValue(made.get("C1"), error("#REF!"))

    def test_a_shifted_range_is_rewritten_top_left_first(self):
        made = sheet({"A1": "=SUM(B1:$D1)", "A2": "=SUM(A$5:A3)"})
        made.copy("A1", "F1")
        self.assertEqual(made.raw("F1"), "=SUM($D1:G1)")
        self.assertEqual(made.raw("A2"), "=SUM(A3:A$5)")
        made.copy("A2", "B12")
        self.assertEqual(made.raw("B12"), "=SUM(B$5:B13)")

    def test_spacing_case_and_strings_survive_a_copy(self):
        made = sheet({"A2": 4, "B1": '= Sum( a1 , 1.50 ) & "a1"'})
        made.copy("B1", "B2")
        self.assertEqual(made.raw("B2"), '= Sum( A2 , 1.50 ) & "a1"')
        self.assertValue(made.get("B2"), "5.5a1")

    def test_copying_a_cell_onto_itself_keeps_it(self):
        made = sheet({"A1": 3, "B1": "=A1+#REF!", "C1": "=A1*2"})
        made.copy("C1", "C1")
        made.copy("B1", "B2")
        self.assertEqual(made.raw("C1"), "=A1*2")
        self.assertValue(made.get("C1"), 6)
        self.assertEqual(made.raw("B2"), "=A2+#REF!")

    def test_bad_names_are_rejected_and_nothing_changes(self):
        made = sheet({"A1": "=B1", "B1": 2})
        for src, dst in (("A1", "A0"), ("$A$1", "B2"), ("A1", "AAA1"), ("A1", None)):
            with self.subTest(src=src, dst=dst), self.assertRaises(ValueError):
                made.copy(src, dst)
        self.assertEqual(made.cells(), ["A1", "B1"])
        self.assertValue(made.get("A1"), 2)


class FormulaTextTests(GridTest):
    def test_references_are_upper_cased_and_the_rest_is_kept(self):
        made = sheet({"C1": "=a1+$b$2", "C2": "=sUm(a1)", "C3": "=1.50+a1", "C4": "=#ref!"})
        self.assertEqual(made.raw("C1"), "=A1+$B$2")
        self.assertEqual(made.raw("C2"), "=sUm(A1)")
        self.assertEqual(made.raw("C3"), "=1.50+A1")
        self.assertEqual(made.raw("C4"), "=#REF!")

    def test_the_contract_example(self):
        made = sheet({"C1": '= sum( b3:$a1 ) & "a1"'})
        self.assertEqual(made.raw("C1"), '= sum( $A1:B3 ) & "a1"')

    def test_typed_ranges_are_written_top_left_first(self):
        made = sheet({"D1": "=SUM(B3:A1)", "D2": "=sum($B3:A1)", "D3": "=SUM(a$3:$b1)", "A1": 1, "B3": 2})
        self.assertEqual(made.raw("D1"), "=SUM(A1:B3)")
        self.assertEqual(made.raw("D2"), "=sum(A1:$B3)")
        self.assertEqual(made.raw("D3"), "=SUM(A1:$B$3)")
        self.assertValue(made.get("D3"), 3)

    def test_raw_returns_literals_unchanged(self):
        made = sheet({"A1": 2.0, "A2": "text", "A3": False, "A4": "=1+1"})
        self.assertValue(made.raw("A1"), 2)
        self.assertValue(made.raw("A2"), "text")
        self.assertValue(made.raw("A3"), False)
        self.assertEqual(made.raw("A4"), "=1+1")
        self.assertIsNone(made.raw("A5"))


class ReadTests(GridTest):
    def test_cells_are_listed_in_row_major_order(self):
        made = sheet({"B2": 1, "A2": 1, "AA1": 1, "Z1": 1, "A10": 1, "B1": "=A2"})
        self.assertEqual(made.cells(), ["B1", "Z1", "AA1", "A2", "B2", "A10"])

    def test_precedents_expand_ranges_once_in_row_major_order(self):
        made = sheet({"D1": "=SUM(B2:C3)+A1+B2+IF(FALSE, A1, 0)", "D2": 5})
        self.assertEqual(made.precedents("D1"), ["A1", "B2", "C2", "B3", "C3"])
        self.assertEqual(made.precedents("D2"), [])
        self.assertEqual(made.precedents("D3"), [])

    def test_dependents_are_direct_only(self):
        made = sheet({"A1": 1, "D1": "=B1", "C1": "=SUM(A1:A3)", "B1": "=A1", "A5": "=A1+A1"})
        self.assertEqual(made.dependents("A1"), ["B1", "C1", "A5"])
        self.assertEqual(made.dependents("A2"), ["C1"])
        self.assertEqual(made.dependents("D1"), [])

    def test_returned_lists_are_not_live(self):
        made = sheet({"A1": 1, "B1": "=A1"})
        made.cells().clear()
        made.precedents("B1").clear()
        made.dependents("A1").clear()
        self.assertEqual(made.cells(), ["A1", "B1"])
        self.assertEqual(made.precedents("B1"), ["A1"])
        self.assertEqual(made.dependents("A1"), ["B1"])
