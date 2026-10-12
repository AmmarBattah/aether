from support import GridTest, error

CELLS = {"A1": 1, "A2": 2, "A3": "x", "A4": True, "B1": 4, "B2": 2.5}


class CollectingTests(GridTest):
    def test_sum_skips_text_booleans_and_empties_in_references(self):
        self.assertFormula("=SUM(A1:B4)", 9.5, CELLS)
        self.assertFormula("=SUM(A4)", 0, CELLS)
        self.assertFormula("=SUM(A3)", 0, CELLS)
        self.assertFormula("=SUM(A1,A1,5)", 7, CELLS)

    def test_sum_coerces_arguments_that_are_not_references(self):
        self.assertFormula("=SUM(TRUE,2)", 3)
        self.assertFormula('=SUM("a")', error("#VALUE!"))
        self.assertFormula("=SUM(A4+0, 1)", 2, CELLS)

    def test_sum_returns_the_first_error_in_row_major_order(self):
        cells = {"A1": 1, "A2": "=1/0", "B1": "=#REF!+1"}
        self.assertFormula("=SUM(A1:B2)", error("#REF!"), cells)
        self.assertFormula("=SUM(A2,B1)", error("#DIV/0!"), cells)
        self.assertFormula('=SUM("a",A2)', error("#VALUE!"), cells)

    def test_count_counts_numbers_and_ignores_errors(self):
        cells = {**CELLS, "C1": "=1/0"}
        self.assertFormula("=COUNT(A1:C4)", 4, cells)
        self.assertFormula('=COUNT(1/0, "a", TRUE, 2, A4)', 1, cells)
        self.assertFormula("=COUNT(#REF!)", 0)

    def test_average(self):
        self.assertFormula("=AVERAGE(A1:A4)", 1.5, CELLS)
        self.assertFormula("=AVERAGE(2,4)", 3)
        self.assertFormula("=AVERAGE(A3:A4)", error("#DIV/0!"), CELLS)

    def test_min_and_max(self):
        self.assertFormula("=MIN(A1:B4)", 1, CELLS)
        self.assertFormula("=MAX(A1:B4)", 4, CELLS)
        self.assertFormula("=MAX(A3:A4)", 0, CELLS)
        self.assertFormula("=MIN(TRUE, 5)", 1)

    def test_an_empty_reference_is_skipped_but_an_empty_result_is_zero(self):
        self.assertFormula("=MAX(-5, A9)", -5)
        self.assertFormula("=MAX(IF(TRUE,A9), -5)", 0)

    def test_concat_joins_display_text_in_row_major_order(self):
        cells = {"A1": 1, "B1": "x", "B2": 2.5}
        self.assertFormula('=CONCAT(A1:B2, "-", TRUE, 1/4)', "1x2.5-TRUE0.25", cells)

    def test_concat_returns_the_first_error(self):
        self.assertFormula('=CONCAT("a", 1/0, #REF!)', error("#DIV/0!"))


class LogicTests(GridTest):
    def test_if_coerces_its_condition(self):
        self.assertFormula('=IF(1, "y", "n")', "y")
        self.assertFormula('=IF(A9, "y", "n")', "n")
        self.assertFormula('=IF(0, "y")', False)
        self.assertFormula('=IF("a", 1, 2)', error("#VALUE!"))
        self.assertFormula("=IF(1/0, 1, 2)", error("#DIV/0!"))

    def test_if_evaluates_only_the_chosen_branch(self):
        self.assertFormula("=IF(TRUE, 1, 1/0)", 1)
        self.assertFormula("=IF(FALSE, 1/0, 2)", 2)
        self.assertFormula("=IF(FALSE, 1, 1/0)", error("#DIV/0!"))

    def test_iferror(self):
        self.assertFormula('=IFERROR(1/0, "x")', "x")
        self.assertFormula("=IFERROR(5, 1/0)", 5)
        self.assertFormula("=IFERROR(1/0, #REF!)", error("#REF!"))
        self.assertFormula("=IFERROR(A1:A2, 7)", 7)
        self.assertFormula("=IFERROR(nosuch(), 1)", 1)

    def test_and_or_not(self):
        self.assertFormula("=AND(1, TRUE, 5)", True)
        self.assertFormula("=AND(1, 0)", False)
        self.assertFormula("=OR(0, A9)", False)
        self.assertFormula("=OR(0, 2)", True)
        self.assertFormula("=NOT(0)", True)

    def test_logic_functions_are_not_lazy_about_errors(self):
        self.assertFormula("=OR(TRUE, 1/0)", error("#DIV/0!"))
        self.assertFormula('=AND("a", 1/0)', error("#VALUE!"))
        self.assertFormula('=NOT("a")', error("#VALUE!"))
        self.assertFormula("=AND(A1:A2)", error("#VALUE!"), CELLS)


class NumericTests(GridTest):
    def test_abs_and_int(self):
        self.assertFormula("=ABS(-2.5)", 2.5)
        self.assertFormula("=INT(-2.5)", -3)
        self.assertFormula("=INT(2.9)", 2)
        self.assertFormula("=INT(TRUE)", 1)
        self.assertFormula('=ABS("x")', error("#VALUE!"))

    def test_round_is_half_away_from_zero(self):
        self.assertFormula("=ROUND(2.5, 0)", 3)
        self.assertFormula("=ROUND(-2.5, 0)", -3)
        self.assertFormula("=ROUND(-0.4, 0)", 0)

    def test_round_uses_the_decimal_representation(self):
        self.assertFormula("=ROUND(1.005, 2)", 1.01)
        self.assertFormula("=ROUND(2.675, 2)", 2.68)

    def test_round_digits(self):
        self.assertFormula("=ROUND(1250, -2)", 1300)
        self.assertFormula("=ROUND(1.5, 0.5)", error("#VALUE!"))
        self.assertFormula('=ROUND(1.5, "a")', error("#VALUE!"))
        self.assertFormula("=ROUND(3.14159, 2)", 3.14)

    def test_mod_takes_the_sign_of_the_divisor(self):
        self.assertFormula("=MOD(7, 3)", 1)
        self.assertFormula("=MOD(-7, 3)", 2)
        self.assertFormula("=MOD(7, -3)", -2)
        self.assertFormula("=MOD(5.5, 2)", 1.5)
        self.assertFormula("=MOD(5, 0)", error("#DIV/0!"))

    def test_arguments_are_checked_left_to_right(self):
        self.assertFormula('=ROUND("a", 1/0)', error("#VALUE!"))
        self.assertFormula('=ROUND(1/0, "a")', error("#DIV/0!"))
        self.assertFormula("=MOD(#REF!, 1/0)", error("#REF!"))
        self.assertFormula('=LEFT(1/0, "a")', error("#DIV/0!"))


class TextTests(GridTest):
    def test_len_upper_lower_coerce_to_text(self):
        self.assertFormula('=LEN("héllo")', 5)
        self.assertFormula("=LEN(123.5)", 5)
        self.assertFormula("=LEN(TRUE)", 4)
        self.assertFormula("=LEN(A9)", 0)
        self.assertFormula("=LEN(1/3)", 8)
        self.assertFormula('=UPPER("aB1")', "AB1")
        self.assertFormula("=LOWER(TRUE)", "true")
        self.assertFormula("=LEN(1/0)", error("#DIV/0!"))

    def test_left_and_right(self):
        self.assertFormula('=LEFT("hello", 2)', "he")
        self.assertFormula('=RIGHT("hello", 2)', "lo")
        self.assertFormula('=LEFT("hi", 10)', "hi")
        self.assertFormula('=RIGHT("hi", 10)', "hi")
        self.assertFormula('=RIGHT("hi", 0)', "")
        self.assertFormula("=RIGHT(12345, 2)", "45")

    def test_left_and_right_need_a_whole_nonnegative_count(self):
        self.assertFormula('=LEFT("hi", -1)', error("#VALUE!"))
        self.assertFormula('=LEFT("hi", 1.5)', error("#VALUE!"))
        self.assertFormula('=RIGHT("hi", "a")', error("#VALUE!"))
        self.assertFormula('=LEFT("hi", 1)', "h")


class InspectTests(GridTest):
    def test_isblank_is_true_only_for_a_reference_to_an_empty_cell(self):
        cells = {"A1": 1, "A5": "=A9"}
        self.assertFormula("=ISBLANK(A9)", True, cells)
        self.assertFormula("=ISBLANK((A9))", True, cells)
        self.assertFormula("=ISBLANK(A1)", False, cells)
        self.assertFormula('=ISBLANK("")', False, cells)
        self.assertFormula("=ISBLANK(A8:A9)", False, cells)
        self.assertFormula("=ISBLANK(A5)", False, cells)

    def test_isnumber_and_istext(self):
        self.assertFormula("=ISNUMBER(1)", True)
        self.assertFormula("=ISNUMBER(TRUE)", False)
        self.assertFormula('=ISNUMBER("1")', False)
        self.assertFormula("=ISNUMBER(A9)", False)
        self.assertFormula('=ISTEXT("")', True)
        self.assertFormula("=ISTEXT(A9)", False)

    def test_inspecting_an_error_is_not_an_error(self):
        self.assertFormula("=ISNUMBER(1/0)", False)
        self.assertFormula("=ISTEXT(#REF!)", False)
        self.assertFormula("=ISERROR(1/0)", True)
        self.assertFormula("=ISERROR(1)", False)
        self.assertFormula("=ISERROR(nosuch())", True)
        self.assertFormula("=ISERROR(A1:A2)", True)

    def test_function_names_are_case_insensitive_and_nest(self):
        self.assertFormula("=sum(Max(1,2), min(3,4))", 5)
        self.assertFormula('=If(Sum(A1:A2)>2, "big", "small")', "big", CELLS)
