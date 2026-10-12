from gridcalc import FormulaError

from support import GridTest, error, sheet

CHAIN = {"A1": 1, "B1": "=A1+1", "C1": "=B1+1", "D1": 5, "E1": "=D1"}


class RecalculationTests(GridTest):
    def chain(self):
        log = []
        made = sheet(CHAIN, log)
        log.clear()
        return made, log

    def test_setting_a_literal_with_no_dependents_evaluates_nothing(self):
        log = []
        made = sheet({"A1": 1, "A2": "text"}, log)
        self.assertEqual(log, [])
        self.assertValue(made.get("A1"), 1)

    def test_setting_a_formula_evaluates_it_once(self):
        log = []
        made = sheet({"A1": 1}, log)
        made.set("b1", "=A1+1")
        self.assertEqual(log, ["B1"])
        self.assertValue(made.get("B1"), 2)

    def test_a_change_flows_down_the_chain_in_order(self):
        made, log = self.chain()
        made.set("A1", 5)
        self.assertEqual(log, ["B1", "C1"])
        self.assertValue(made.get("C1"), 7)

    def test_unrelated_formulas_are_not_evaluated(self):
        made, log = self.chain()
        made.set("D1", 6)
        self.assertEqual(log, ["E1"])
        self.assertValue(made.get("E1"), 6)

    def test_setting_the_same_content_still_evaluates(self):
        made, log = self.chain()
        made.set("A1", 1)
        self.assertEqual(log, ["B1", "C1"])

    def test_a_diamond_evaluates_each_cell_once_dependencies_first(self):
        log = []
        made = sheet({"A1": 1, "B1": "=A1", "C1": "=A1", "D1": "=B1+C1", "E1": "=D1+A1"}, log)
        log.clear()
        made.set("A1", 4)
        self.assertEqual(sorted(log), ["B1", "C1", "D1", "E1"])
        self.assertGreater(log.index("D1"), max(log.index("B1"), log.index("C1")))
        self.assertGreater(log.index("E1"), log.index("D1"))
        self.assertValue(made.get("E1"), 12)

    def test_a_range_depends_on_every_cell_inside_it(self):
        log = []
        made = sheet({"B1": "=SUM(A1:A3)"}, log)
        log.clear()
        made.set("A2", 5)
        self.assertEqual(log, ["B1"])
        made.set("A4", 5)
        self.assertEqual(log, ["B1"])
        self.assertValue(made.get("B1"), 5)

    def test_an_untaken_branch_is_still_a_dependency(self):
        log = []
        made = sheet({"B1": "=IF(TRUE, 1, A5)"}, log)
        log.clear()
        made.set("A5", 3)
        self.assertEqual(log, ["B1"])
        self.assertEqual(made.precedents("B1"), ["A5"])

    def test_replacing_a_formula_replaces_its_dependencies(self):
        log = []
        made = sheet({"A1": 1, "A2": 2, "B1": "=A1"}, log)
        made.set("B1", "=A2")
        log.clear()
        made.set("A1", 9)
        self.assertEqual(log, [])
        made.set("A2", 8)
        self.assertEqual(log, ["B1"])
        self.assertValue(made.get("B1"), 8)

    def test_a_set_formula_is_evaluated_before_its_dependents(self):
        log = []
        made = sheet({"A1": 3, "C1": "=B1+1", "D1": "=C1+1"}, log)
        log.clear()
        made.set("B1", "=A1*2")
        self.assertEqual(log, ["B1", "C1", "D1"])
        self.assertValue(made.get("D1"), 8)

    def test_clearing_a_cell_evaluates_its_dependents(self):
        made, log = self.chain()
        made.set("A1", None)
        self.assertEqual(log, ["B1", "C1"])
        self.assertValue(made.get("C1"), 2)

    def test_reads_never_evaluate(self):
        made, log = self.chain()
        for name in ("A1", "B1", "C1", "Z9"):
            made.get(name), made.text(name), made.raw(name), made.precedents(name), made.dependents(name)
        made.cells()
        self.assertEqual(log, [])
        self.assertValue(made.get("C1"), 3)

    def test_a_wide_fan_evaluates_each_dependent_once(self):
        log = []
        made = sheet({"A1": 1}, log)
        for row in range(2, 32):
            made.set(f"A{row}", f"=A1*{row}")
        made.set("B1", "=SUM(A2:A31)")
        log.clear()
        made.set("A1", 2)
        self.assertEqual(sorted(log), sorted([f"A{row}" for row in range(2, 32)] + ["B1"]))
        self.assertEqual(log[-1], "B1")
        self.assertValue(made.get("B1"), 2 * sum(range(2, 32)))

    def test_every_evaluated_cell_follows_its_evaluated_precedents(self):
        log = []
        made = sheet({"A1": 1}, log)
        for index in range(2, 14):
            made.set(f"A{index}", f"=A{index - 1}+A{max(1, index - 3)}+SUM(A1:A{index // 2})")
        log.clear()
        made.set("A1", 2)
        self.assertEqual(sorted(log), sorted(f"A{index}" for index in range(2, 14)))
        for position, name in enumerate(log):
            for precedent in made.precedents(name):
                self.assertTrue(precedent == "A1" or log.index(precedent) < position, (name, precedent))

    def test_a_failed_set_evaluates_nothing(self):
        made, log = self.chain()
        with self.assertRaises(FormulaError):
            made.set("B1", "=A1+")
        self.assertEqual(log, [])
        self.assertEqual(made.raw("B1"), "=A1+1")

    def test_copy_evaluates_like_set_on_the_destination(self):
        log = []
        made = sheet({"A1": 1, "A2": 2, "B1": "=A1*10", "C1": "=B2+1", "D1": "=B1"}, log)
        log.clear()
        made.copy("B1", "B2")
        self.assertEqual(log, ["B2", "C1"])
        self.assertValue(made.get("C1"), 21)

    def test_a_structural_edit_evaluates_every_formula_once(self):
        log = []
        made = sheet({"A1": 1, "B1": "=A1", "C1": "=B1", "D5": "=7", "E1": "=E1"}, log)
        log.clear()
        made.insert_rows(3)
        self.assertEqual(sorted(log), ["B1", "C1", "D6"])
        self.assertLess(log.index("B1"), log.index("C1"))
        self.assertValue(made.get("D6"), 7)


class CycleTests(GridTest):
    def test_a_self_reference_is_a_cycle_and_is_not_evaluated(self):
        log = []
        made = sheet({"A1": "=A1+1"}, log)
        self.assertValue(made.get("A1"), error("#CYCLE!"))
        self.assertEqual(made.text("A1"), "#CYCLE!")
        self.assertEqual(log, [])

    def test_cells_behind_a_cycle_are_cyclic_too(self):
        log = []
        made = sheet({"A1": "=B1", "B1": "=A1", "C1": "=IFERROR(A1, 0)", "D1": "=C1+1", "E1": "=5"}, log)
        for name in ("A1", "B1", "C1", "D1"):
            self.assertValue(made.get(name), error("#CYCLE!"))
        self.assertValue(made.get("E1"), 5)
        self.assertEqual(log, ["A1", "E1"])

    def test_an_untaken_branch_can_close_a_cycle(self):
        made = sheet({"A1": "=IF(FALSE, A1, 1)", "B1": "=IF(TRUE, 1, B1)"})
        self.assertValue(made.get("A1"), error("#CYCLE!"))
        self.assertValue(made.get("B1"), error("#CYCLE!"))

    def test_a_range_containing_its_own_cell_is_a_cycle(self):
        made = sheet({"A2": 4, "A1": "=SUM(A1:A3)", "B1": "=COUNT(A1:A3)", "C1": "=SUM(A2:A3)"})
        self.assertValue(made.get("A1"), error("#CYCLE!"))
        self.assertValue(made.get("B1"), error("#CYCLE!"))
        self.assertValue(made.get("C1"), 4)

    def test_breaking_a_cycle_restores_values(self):
        log = []
        made = sheet({"A1": "=B1", "B1": "=A1", "C1": "=A1+1"}, log)
        log.clear()
        made.set("B1", 5)
        self.assertEqual(log, ["A1", "C1"])
        self.assertValue(made.get("A1"), 5)
        self.assertValue(made.get("C1"), 6)

    def test_creating_a_cycle_marks_dependents_without_evaluating(self):
        log = []
        made = sheet({"A1": 1, "B1": "=A1", "C1": "=B1", "D1": "=A1+C1"}, log)
        log.clear()
        made.set("A1", "=C1")
        self.assertEqual(log, [])
        for name in ("A1", "B1", "C1", "D1"):
            self.assertValue(made.get(name), error("#CYCLE!"))

    def test_a_dependency_on_a_cycle_through_one_argument_only(self):
        made = sheet({"A1": "=A1", "B1": 2, "C1": "=B1*2", "D1": "=C1+A1"})
        self.assertValue(made.get("C1"), 4)
        self.assertValue(made.get("D1"), error("#CYCLE!"))
        made.set("A1", 1)
        self.assertValue(made.get("D1"), 5)
