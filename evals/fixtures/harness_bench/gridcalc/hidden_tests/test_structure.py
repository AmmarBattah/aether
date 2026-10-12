from support import GridTest, error, sheet

COLUMN = {"A1": 1, "A2": 2, "A3": 3, "A4": 4, "A5": 5}


def column(**extra):
    return sheet({**COLUMN, **extra})


class InsertRowTests(GridTest):
    def test_cells_move_down(self):
        made = column()
        made.insert_rows(3)
        self.assertEqual(made.cells(), ["A1", "A2", "A4", "A5", "A6"])
        self.assertValue(made.get("A4"), 3)
        self.assertIsNone(made.get("A3"))

    def test_count_moves_several_rows(self):
        made = column()
        made.insert_rows(1, 3)
        self.assertEqual(made.cells(), ["A4", "A5", "A6", "A7", "A8"])

    def test_references_follow_their_cells(self):
        made = column(B1="=A3+A1")
        made.insert_rows(2)
        self.assertEqual(made.raw("B1"), "=A4+A1")
        self.assertValue(made.get("B1"), 4)

    def test_a_moved_formula_keeps_pointing_at_its_cells(self):
        made = column(B4="=A4*2+A1")
        made.insert_rows(2, 2)
        self.assertEqual(made.raw("B6"), "=A6*2+A1")
        self.assertValue(made.get("B6"), 9)
        self.assertIsNone(made.raw("B4"))

    def test_a_range_grows_when_rows_are_inserted_inside_it(self):
        made = column(B1="=SUM(A1:A5)")
        made.insert_rows(3)
        self.assertEqual(made.raw("B1"), "=SUM(A1:A6)")
        made.set("A3", 10)
        self.assertValue(made.get("B1"), 25)

    def test_a_range_moves_when_rows_are_inserted_at_its_first_row(self):
        made = column(B7="=SUM(A2:A4)")
        made.insert_rows(2)
        self.assertEqual(made.raw("B8"), "=SUM(A3:A5)")
        self.assertValue(made.get("B8"), 9)

    def test_a_range_is_unchanged_by_rows_inserted_after_its_last_row(self):
        made = column(B1="=SUM(A1:A5)")
        made.insert_rows(6)
        self.assertEqual(made.raw("B1"), "=SUM(A1:A5)")
        made.set("A6", 100)
        self.assertValue(made.get("B1"), 15)

    def test_a_range_grows_when_rows_are_inserted_at_its_last_row(self):
        made = column(B1="=SUM(A1:A5)")
        made.insert_rows(5)
        self.assertEqual(made.raw("B1"), "=SUM(A1:A6)")
        self.assertValue(made.get("B1"), 15)

    def test_dollar_markers_are_kept_and_still_move(self):
        made = column(B1="=$A$3+A$3+$A3")
        made.insert_rows(1)
        self.assertEqual(made.raw("B2"), "=$A$4+A$4+$A4")
        self.assertValue(made.get("B2"), 9)

    def test_everything_but_references_is_kept_as_typed(self):
        made = column(B1='= sum( a1:a5 ) & " A3 "')
        made.insert_rows(3)
        self.assertEqual(made.raw("B1"), '= sum( A1:A6 ) & " A3 "')
        self.assertValue(made.get("B1"), "15 A3 ")

    def test_strings_and_escaped_quotes_are_never_rewritten(self):
        made = column(B1='="A1:A5 ""A2"" "&A2')
        made.insert_rows(1)
        self.assertEqual(made.raw("B2"), '="A1:A5 ""A2"" "&A3')
        self.assertValue(made.get("B2"), 'A1:A5 "A2" 2')

    def test_a_function_named_like_a_cell_is_not_rewritten(self):
        made = column(B1="=IFERROR(A3(A3), A3)")
        made.insert_rows(1)
        self.assertEqual(made.raw("B2"), "=IFERROR(A3(A4), A4)")
        self.assertValue(made.get("B2"), 3)

    def test_precedents_and_dependents_follow_the_edit(self):
        made = column(B1="=SUM(A2:A3)")
        made.insert_rows(3, 2)
        self.assertEqual(made.precedents("B1"), ["A2", "A3", "A4", "A5"])
        self.assertEqual(made.dependents("A4"), ["B1"])
        self.assertValue(made.get("B1"), 5)


class DeleteRowTests(GridTest):
    def test_deleted_cells_vanish_and_lower_cells_move_up(self):
        made = column()
        made.delete_rows(2)
        self.assertEqual(made.cells(), ["A1", "A2", "A3", "A4"])
        self.assertEqual([made.get(name) for name in made.cells()], [1, 3, 4, 5])

    def test_a_reference_to_a_deleted_cell_becomes_ref_error(self):
        made = column(B1="=A3+A5")
        made.delete_rows(3)
        self.assertEqual(made.raw("B1"), "=#REF!+A4")
        self.assertValue(made.get("B1"), error("#REF!"))

    def test_references_above_the_band_are_unchanged(self):
        made = column(B1="=A1+A2")
        made.delete_rows(3, 2)
        self.assertEqual(made.raw("B1"), "=A1+A2")
        self.assertValue(made.get("B1"), 3)

    def test_a_range_shrinks_when_inner_rows_are_deleted(self):
        made = column(B1="=SUM(A1:A5)")
        made.delete_rows(2, 2)
        self.assertEqual(made.raw("B1"), "=SUM(A1:A3)")
        self.assertValue(made.get("B1"), 10)

    def test_a_range_losing_its_top_rows_keeps_the_rest(self):
        made = column(B9="=SUM(A2:A5)")
        made.delete_rows(1, 2)
        self.assertEqual(made.raw("B7"), "=SUM(A1:A3)")
        self.assertValue(made.get("B7"), 12)

    def test_a_range_losing_its_bottom_rows_keeps_the_rest(self):
        made = column(B1="=SUM(A2:A5)")
        made.delete_rows(4, 3)
        self.assertEqual(made.raw("B1"), "=SUM(A2:A3)")
        self.assertValue(made.get("B1"), 5)

    def test_a_range_deleted_entirely_becomes_ref_error(self):
        made = column(B1="=SUM(A2:A3)", C1="=COUNT(A2:A3)", D1="=SUM(A1:A5)")
        made.delete_rows(2, 2)
        self.assertEqual(made.raw("B1"), "=SUM(#REF!)")
        self.assertValue(made.get("B1"), error("#REF!"))
        self.assertValue(made.get("C1"), 0)
        self.assertEqual(made.raw("D1"), "=SUM(A1:A3)")

    def test_corner_markers_survive_a_shrink(self):
        made = column(B1="=SUM($A$1:A$5)")
        made.delete_rows(5)
        self.assertEqual(made.raw("B1"), "=SUM($A$1:A$4)")
        self.assertValue(made.get("B1"), 10)

    def test_iferror_catches_a_deleted_reference(self):
        made = column(B1='=IFERROR(A3, "gone")')
        made.delete_rows(3)
        self.assertEqual(made.raw("B1"), '=IFERROR(#REF!, "gone")')
        self.assertValue(made.get("B1"), "gone")

    def test_insert_then_delete_restores_the_sheet(self):
        made = column(B2="= Sum( $a$1:a5 ) + a3", B5="=B2*2")
        before = {name: (made.raw(name), made.get(name)) for name in made.cells()}
        made.insert_rows(3, 4)
        self.assertEqual(made.raw("B2"), "= Sum( $A$1:A9 ) + A7")
        made.delete_rows(3, 4)
        self.assertEqual({name: (made.raw(name), made.get(name)) for name in made.cells()}, before)


class ColumnTests(GridTest):
    def row(self, **extra):
        return sheet({"A1": 1, "B1": 2, "C1": 3, **extra})

    def test_insert_cols_moves_cells_and_grows_ranges(self):
        made = self.row(D1="=SUM(A1:C1)")
        made.insert_cols("B")
        self.assertEqual(made.cells(), ["A1", "C1", "D1", "E1"])
        self.assertEqual(made.raw("E1"), "=SUM(A1:D1)")
        self.assertValue(made.get("E1"), 6)

    def test_column_names_roll_over_from_z_to_aa(self):
        made = sheet({"Y1": 1, "Z1": 2, "AA1": "=Z1+Y1"})
        made.insert_cols("z", 1)
        self.assertEqual(made.cells(), ["Y1", "AA1", "AB1"])
        self.assertEqual(made.raw("AB1"), "=AA1+Y1")
        self.assertValue(made.get("AB1"), 3)

    def test_insert_many_cols_crosses_into_two_letter_names(self):
        made = sheet({"A1": 1, "B1": "=A1+C1", "C1": 5})
        made.insert_cols("B", 26)
        self.assertEqual(made.raw("AB1"), "=A1+AC1")
        self.assertValue(made.get("AB1"), 6)

    def test_delete_cols_rewrites_references(self):
        made = self.row(D1="=B1+C1", E1="=SUM(A1:C1)")
        made.delete_cols("b")
        self.assertEqual(made.raw("C1"), "=#REF!+B1")
        self.assertEqual(made.raw("D1"), "=SUM(A1:B1)")
        self.assertValue(made.get("D1"), 4)

    def test_a_block_range_shrinks_on_one_axis_only(self):
        made = sheet({"A1": 1, "B2": 2, "C3": 3, "D4": 4, "E1": "=SUM(A1:D4)"})
        made.delete_cols("C", 2)
        self.assertEqual(made.raw("C1"), "=SUM(A1:B4)")
        self.assertValue(made.get("C1"), 3)
        made.delete_rows(1)
        self.assertEqual(made.cells(), ["B1"])


class EditValidationTests(GridTest):
    def test_insert_past_the_last_row_changes_nothing(self):
        made = sheet({"A9999": 1, "A1": "=A9999"})
        with self.assertRaises(ValueError):
            made.insert_rows(2)
        self.assertEqual(made.cells(), ["A1", "A9999"])
        self.assertEqual(made.raw("A1"), "=A9999")
        self.assertValue(made.get("A1"), 1)

    def test_insert_that_pushes_a_reference_off_the_grid_changes_nothing(self):
        made = sheet({"A1": "=B9999+1", "A2": 2, "A7000": 3})
        with self.assertRaises(ValueError):
            made.insert_rows(5000)
        self.assertEqual(made.raw("A1"), "=B9999+1")
        self.assertEqual(made.cells(), ["A1", "A2", "A7000"])

    def test_insert_past_the_last_column_changes_nothing(self):
        made = sheet({"ZZ1": 1, "A1": 2})
        with self.assertRaises(ValueError):
            made.insert_cols("A")
        self.assertEqual(made.cells(), ["A1", "ZZ1"])

    def test_edit_arguments_are_validated(self):
        made = sheet(COLUMN)
        bad_calls = [lambda: made.insert_rows(0), lambda: made.insert_rows(True), lambda: made.insert_rows(1, 0),
                     lambda: made.insert_rows(1, 1.0), lambda: made.delete_rows(9999, 2), lambda: made.delete_rows("A"),
                     lambda: made.insert_cols("AAA"), lambda: made.insert_cols(1), lambda: made.delete_cols("ZZ", 2),
                     lambda: made.delete_cols("A", True), lambda: made.insert_rows(10000)]
        for index, call in enumerate(bad_calls):
            with self.subTest(index=index), self.assertRaises(ValueError):
                call()
        self.assertEqual(made.cells(), ["A1", "A2", "A3", "A4", "A5"])
        made.delete_rows(9999)
        made.delete_cols("zz")
        self.assertEqual(made.cells(), ["A1", "A2", "A3", "A4", "A5"])
