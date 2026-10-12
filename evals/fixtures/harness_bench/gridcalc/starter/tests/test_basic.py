import unittest

from gridcalc import CellError, FormulaError, Sheet


class BasicSheetTests(unittest.TestCase):
    def test_literals_round_trip(self):
        sheet = Sheet()
        sheet.set("A1", 3)
        sheet.set("B1", "word")
        self.assertEqual(sheet.get("A1"), 3)
        self.assertEqual(sheet.get("B1"), "word")
        self.assertIsNone(sheet.get("C1"))

    def test_arithmetic_precedence(self):
        sheet = Sheet()
        sheet.set("A1", 2)
        sheet.set("B1", "=A1+2*3")
        self.assertEqual(sheet.get("B1"), 8)

    def test_sum_over_a_range(self):
        sheet = Sheet()
        for row, number in enumerate((1, 2, 3), start=1):
            sheet.set(f"A{row}", number)
        sheet.set("B1", "=SUM(A1:A3)")
        self.assertEqual(sheet.get("B1"), 6)

    def test_clearing_a_cell(self):
        sheet = Sheet()
        sheet.set("A1", 1)
        sheet.set("A1", None)
        self.assertEqual(sheet.cells(), [])

    def test_errors_are_values_and_exceptions(self):
        self.assertEqual(CellError("#DIV/0!"), CellError("#DIV/0!"))
        self.assertTrue(issubclass(FormulaError, ValueError))


if __name__ == "__main__":
    unittest.main()
