from gridcalc import CellError, FormulaError

from support import GridTest, error, sheet, value


class PrecedenceTests(GridTest):
    def test_multiplication_binds_tighter_than_addition(self):
        self.assertFormula("=1+2*3", 7)
        self.assertFormula("=(1+2)*3", 9)

    def test_power_is_right_associative(self):
        self.assertFormula("=2^3^2", 512)

    def test_unary_minus_binds_looser_than_power(self):
        self.assertFormula("=-2^2", -4)
        self.assertFormula("=(-2)^2", 4)

    def test_power_takes_a_signed_exponent(self):
        self.assertFormula("=2^-1", 0.5)
        self.assertFormula("=2^-3^2", 0.001953125)

    def test_subtraction_and_division_associate_left(self):
        self.assertFormula("=10-4-3", 3)
        self.assertFormula("=100/10/5", 2)

    def test_percent_is_a_postfix_operator(self):
        self.assertFormula("=50%", 0.5)
        self.assertFormula("=200*10%", 20)
        self.assertFormula("=5%%", 0.0005)
        self.assertFormula("=4^50%", 2)

    def test_stacked_unary_signs(self):
        self.assertFormula("=--3", 3)
        self.assertFormula("=3--2", 5)
        self.assertFormula("=-+-2", 2)

    def test_concatenation_binds_looser_than_addition(self):
        self.assertFormula("=1&2+3", "15")

    def test_comparison_binds_loosest(self):
        self.assertFormula('="a"&"b"="ab"', True)
        self.assertFormula("=1+1=2", True)

    def test_comparisons_chain_left_to_right(self):
        self.assertFormula("=1<2<3", False)
        self.assertFormula("=3>2=TRUE", True)


class TokenTests(GridTest):
    def test_doubled_quote_is_one_quote_character(self):
        self.assertFormula('="say ""hi"""', 'say "hi"')

    def test_empty_string_literal_is_text_not_empty(self):
        self.assertFormula('=""', "")

    def test_spaces_and_case_are_insignificant(self):
        self.assertFormula("=  sUm( 1 ,2 ) +  true", 4)

    def test_boolean_and_ref_error_literals(self):
        self.assertFormula("=TRUE", True)
        self.assertFormula("=false", False)
        self.assertFormula("=#ref!", error("#REF!"))

    def test_dollar_markers_do_not_change_the_value(self):
        self.assertFormula("=$A$1+A$1+$A1", 6, {"A1": 2})

    def test_two_letter_columns(self):
        self.assertFormula("=AA1+ZZ9999+z1", 12, {"AA1": 5, "Z1": 7})

    def test_a_name_followed_by_a_parenthesis_is_a_function_call(self):
        self.assertFormula("=A1(2)", error("#NAME?"), {"A1": 5})
        self.assertFormula("=A1 (2)", error("#NAME?"), {"A1": 5})

    def test_unknown_function_is_name_error_whatever_its_arguments(self):
        self.assertFormula("=nosuch()", error("#NAME?"))
        self.assertFormula("=NoSuch(1/0, A1:B2, 3)", error("#NAME?"))


class FormulaErrorTests(GridTest):
    def assertRejected(self, *formulas):
        made = sheet({"B2": "=1+1"})
        for formula in formulas:
            with self.subTest(formula=formula):
                with self.assertRaises(FormulaError):
                    made.set("B2", formula)
                self.assertEqual(made.raw("B2"), "=1+1")
                self.assertValue(made.get("B2"), 2)

    def test_incomplete_expressions_are_rejected(self):
        self.assertRejected("=", "=1+", "=(1", "=1)", "=*2", "=SUM(1,", "=SUM(1,)", '="abc')

    def test_trailing_text_is_rejected(self):
        self.assertRejected("=1 2", "=1,2", "=A1 B1", '="a""b" "c"')

    def test_malformed_numbers_are_rejected(self):
        self.assertRejected("=.5", "=5.", "=1e5", "=1.2.3")

    def test_names_outside_the_grid_are_rejected(self):
        self.assertRejected("=A0", "=AAA1", "=A10000", "=A01", "=foo", "=A", "=$A")

    def test_unknown_characters_are_rejected(self):
        self.assertRejected("=1 # 2", "=1\t+1", "=A1 : B2", "=A1:", "=1 == 1", "='a'")

    def test_wrong_argument_counts_are_rejected(self):
        self.assertRejected("=SUM()", "=IF(1)", "=IF(1,2,3,4)", "=NOT(1,2)", "=ROUND(1)", "=iferror(1)", "=LEN()")

    def test_formula_error_is_a_value_error(self):
        self.assertTrue(issubclass(FormulaError, ValueError))
        self.assertRejected("=+")


class ArithmeticTests(GridTest):
    def test_empty_cells_coerce_to_zero_and_empty_text(self):
        self.assertFormula("=A1+1", 1)
        self.assertFormula('=A1&"x"', "x")
        self.assertFormula("=A1*5", 0)

    def test_booleans_coerce_to_one_and_zero(self):
        self.assertFormula("=TRUE+TRUE", 2)
        self.assertFormula("=-TRUE", -1)
        self.assertFormula("=FALSE*9", 0)

    def test_text_is_never_a_number(self):
        self.assertFormula('="5"+1', error("#VALUE!"))
        self.assertFormula("=A1+1", error("#VALUE!"), {"A1": "5"})
        self.assertFormula("=A1*2", error("#VALUE!"), {"A1": "abc"})

    def test_unary_signs_coerce_like_arithmetic(self):
        self.assertFormula('=-"a"', error("#VALUE!"))
        self.assertFormula('=+"a"', error("#VALUE!"))
        self.assertFormula("=+TRUE", 1)

    def test_division_and_power_errors(self):
        self.assertFormula("=1/0", error("#DIV/0!"))
        self.assertFormula("=0^-1", error("#DIV/0!"))
        self.assertFormula("=(-8)^0.5", error("#NUM!"))
        self.assertFormula("=0^0", 1)

    def test_power_overflow_is_num_error(self):
        self.assertFormula("=10.5^400", error("#NUM!"))
        self.assertFormula("=2^10", 1024)

    def test_the_left_operand_error_wins(self):
        self.assertFormula('="a"+1/0', error("#VALUE!"))
        self.assertFormula('=1/0+"a"', error("#DIV/0!"))
        self.assertFormula("=(1/0)&#REF!", error("#DIV/0!"))
        self.assertFormula("=#REF!=1/0", error("#REF!"))

    def test_concatenation_uses_display_text(self):
        self.assertFormula('=1/3&""', "0.333333")
        self.assertFormula("=TRUE&1", "TRUE1")
        self.assertFormula('=2.50&"x"', "2.5x")
        self.assertFormula('=1/4&""', "0.25")

    def test_integral_results_are_ints(self):
        self.assertFormula("=4/2", 2)
        self.assertFormula("=0.5+0.5", 1)
        self.assertFormula("=7/2", 3.5)
        self.assertFormula("=-0.5+0.5", 0)
        self.assertFormula("=1.50*2", 3)

    def test_a_range_is_not_a_single_value(self):
        cells = {"A1": 1, "A2": 2}
        self.assertFormula("=A1:A2", error("#VALUE!"), cells)
        self.assertFormula("=A1:A2+1", error("#VALUE!"), cells)
        self.assertFormula("=ABS(A1:A2)", error("#VALUE!"), cells)
        self.assertFormula("=IF(TRUE,A1:A2)", error("#VALUE!"), cells)
        self.assertFormula("=SUM((A1:A2))", 3, cells)

    def test_an_empty_formula_result_is_zero(self):
        self.assertFormula("=A1", 0)
        self.assertFormula("=IF(TRUE,A1)", 0)
        self.assertFormula('=IF(TRUE,A1)&"x"', "x")


class ComparisonTests(GridTest):
    def test_numbers_compare_numerically(self):
        self.assertFormula("=2>1", True)
        self.assertFormula("=2>=2", True)
        self.assertFormula("=1<>1", False)
        self.assertFormula("=10<9", False)

    def test_text_compares_case_insensitively(self):
        self.assertFormula('="abc"="ABC"', True)
        self.assertFormula('="a"<"B"', True)
        self.assertFormula('="Z"<"a"', False)
        self.assertFormula('="10"<"9"', True)

    def test_types_order_number_text_boolean(self):
        self.assertFormula('=1<"a"', True)
        self.assertFormula('="z"<FALSE', True)
        self.assertFormula('=99999<""', True)
        self.assertFormula('=TRUE>"zzz"', True)
        self.assertFormula("=1=TRUE", False)
        self.assertFormula('="1"=1', False)

    def test_empty_matches_the_other_operand_type(self):
        self.assertFormula("=A1=0", True)
        self.assertFormula('=A1=""', True)
        self.assertFormula("=A1=FALSE", True)
        self.assertFormula("=A1=B1", True)
        self.assertFormula("=A1=TRUE", False)

    def test_empty_orders_like_its_stand_in(self):
        self.assertFormula("=A1<1", True)
        self.assertFormula("=A1<-1", False)
        self.assertFormula('=A1<"a"', True)
        self.assertFormula("=A1<TRUE", True)

    def test_booleans_order_false_before_true(self):
        self.assertFormula("=FALSE<TRUE", True)
        self.assertFormula("=TRUE<=FALSE", False)

    def test_comparing_errors_propagates_the_left_one(self):
        self.assertFormula("=1/0=1/0", error("#DIV/0!"))
        self.assertFormula("=1<#REF!", error("#REF!"))


class DisplayTests(GridTest):
    def display(self, raw):
        made = sheet({"A1": raw})
        return made.text("A1")

    def test_floats_round_to_six_places(self):
        self.assertEqual(self.display("=1/3"), "0.333333")
        self.assertEqual(self.display("=2/3"), "0.666667")
        self.assertEqual(self.display(1234.5), "1234.5")

    def test_rounding_is_half_away_from_zero_on_the_repr(self):
        self.assertEqual(self.display(0.0000005), "0.000001")
        self.assertEqual(self.display(-0.0000005), "-0.000001")
        self.assertEqual(self.display(1.0000005), "1.000001")

    def test_negative_zero_is_written_as_zero(self):
        self.assertEqual(self.display(-0.0000001), "0")
        self.assertEqual(self.display(0.0000001), "0")

    def test_other_values_display(self):
        self.assertEqual(self.display(True), "TRUE")
        self.assertEqual(self.display("=1/0"), "#DIV/0!")
        self.assertEqual(self.display(1000000), "1000000")
        self.assertEqual(self.display("word"), "word")
        self.assertEqual(sheet().text("A1"), "")


class ContentTests(GridTest):
    def test_literals_keep_their_type(self):
        made = sheet({"A1": True, "A2": 3, "A3": 2.5, "A4": "5", "A5": 2.0})
        self.assertValue(made.get("A1"), True)
        self.assertValue(made.get("A2"), 3)
        self.assertValue(made.get("A3"), 2.5)
        self.assertValue(made.get("A4"), "5")
        self.assertValue(made.get("A5"), 2)

    def test_none_and_empty_string_clear_the_cell(self):
        made = sheet({"A1": 1, "A2": 2, "A3": 3})
        made.set("A1", None)
        made.set("A2", "")
        self.assertEqual(made.cells(), ["A3"])
        self.assertIsNone(made.get("A1"))
        self.assertIsNone(made.raw("A2"))

    def test_unsupported_content_is_rejected_and_changes_nothing(self):
        made = sheet({"A1": 7})
        for raw in (float("nan"), float("inf"), [1], {"a": 1}, b"x", 1j):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                made.set("A1", raw)
        self.assertValue(made.get("A1"), 7)

    def test_cell_names_are_case_insensitive_and_validated(self):
        made = sheet({"a1": 4, "aa10": 6})
        self.assertValue(made.get("A1"), 4)
        self.assertEqual(made.cells(), ["A1", "AA10"])
        for name in ("$A$1", "A0", "AAA1", "A10000", "A01", "1A", "", "A", 1, None, "A1:A2", " A1"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                made.get(name)

    def test_cell_errors_compare_by_code(self):
        self.assertEqual(value("=1/0"), CellError("#DIV/0!"))
        self.assertNotEqual(value("=1/0"), CellError("#VALUE!"))
        self.assertNotEqual(value("=1/0"), "#DIV/0!")
        self.assertEqual(value("=1/0").code, "#DIV/0!")
