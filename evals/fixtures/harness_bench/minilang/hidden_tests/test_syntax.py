import unittest

from minilang import MiniRuntimeError, MiniSyntaxError, MiniTypeError
from support import MiniCase


class LexerTests(MiniCase):
    def test_comments_and_tabs(self):
        self.assertRuns('# note\n\tprint(1); # trailing\nprint(2);', ["1", "2"])

    def test_large_integer_literal(self):
        self.assertRuns("print(123456789012345678901234567890 + 1);", ["123456789012345678901234567891"])

    def test_leading_zero_literal_is_rejected(self):
        self.assertFails("let x = 1;\nlet y = 007;", MiniSyntaxError, 2, 9)

    def test_string_escapes(self):
        self.assertRuns(r'print("a\tb\\c\"d"); print(len("x\ny"));', ['a\tb\\c"d', "3"])

    def test_unknown_escape_is_reported_at_backslash(self):
        self.assertFails(r'print("ab\qc");', MiniSyntaxError, 1, 10)

    def test_unterminated_string_is_reported_at_opening_quote(self):
        self.assertFails('print(1);\nlet s = "abc\n";', MiniSyntaxError, 2, 9)

    def test_unexpected_character(self):
        self.assertFails("let a = 1;\nlet b = a & 2;", MiniSyntaxError, 2, 11)

    def test_non_ascii_identifier_is_rejected(self):
        self.assertFails("let café = 1;", MiniSyntaxError, 1, 8)

    def test_keyword_is_not_an_identifier(self):
        self.assertFails("let nil = 1;", MiniSyntaxError, 1, 5)


class PrecedenceTests(MiniCase):
    def test_arithmetic_precedence(self):
        self.assertRuns("print(1 + 2 * 3 - 4 / 2, (1 + 2) * 3);", ["5 9"])

    def test_power_binds_tighter_than_prefix_minus(self):
        self.assertRuns("print(-2 ** 2, (-2) ** 2);", ["-4 4"])

    def test_power_is_right_associative(self):
        self.assertRuns("print(2 ** 3 ** 2);", ["512"])

    def test_power_right_operand_may_be_negated(self):
        self.assertFails("print(1);\nprint(2 ** -1);", MiniRuntimeError, 2, 9, ["1"])

    def test_power_binds_tighter_than_multiplication(self):
        self.assertRuns("print(2 * 3 ** 2, -3 * -2);", ["18 6"])

    def test_not_is_looser_than_comparison(self):
        self.assertRuns("print(not 1 == 2, not nil and 1);", ["true 1"])

    def test_and_binds_tighter_than_or(self):
        self.assertRuns('print(1 or 2 and nil, nil and 1 or "x");', ["1 x"])

    def test_comparisons_do_not_chain(self):
        self.assertFails("print(1 < 2 < 3);", MiniSyntaxError, 1, 13)

    def test_postfix_binds_tighter_than_prefix_minus(self):
        self.assertRuns("let xs = [3, 4]; print(-xs[1] + 1);", ["-3"])

    def test_call_results_can_be_indexed_and_called(self):
        self.assertRuns("fn f() { return [fn (x) { return x * 2; }]; } print(f()[0](21));", ["42"])


class StatementSyntaxTests(MiniCase):
    def test_invalid_assignment_target(self):
        self.assertFails("let a = 1;\na + 1 = 3;", MiniSyntaxError, 2, 7)

    def test_call_is_not_an_assignment_target(self):
        self.assertFails("fn f() {}\nf() = 3;", MiniSyntaxError, 2, 5)

    def test_duplicate_parameter(self):
        self.assertFails("fn f(a, b, a) {}", MiniSyntaxError, 1, 12)

    def test_return_outside_function(self):
        self.assertFails('print("never");\nif true { return 1; }', MiniSyntaxError, 2, 11)

    def test_break_outside_loop(self):
        self.assertFails("print(1);\nbreak;", MiniSyntaxError, 2, 1)

    def test_break_does_not_cross_function_boundary(self):
        self.assertFails("while false {\n  fn f() { continue; }\n}", MiniSyntaxError, 2, 12)

    def test_missing_semicolon_at_end_of_input(self):
        self.assertFails("print(1);\nprint(2)", MiniSyntaxError, 2, 9)

    def test_missing_closing_brace(self):
        self.assertFails("if true {\n  print(1);\n", MiniSyntaxError, 3, 1)

    def test_syntax_errors_prevent_execution(self):
        self.assertFails('print("ran");\nlet = 2;', MiniSyntaxError, 2, 5)

    def test_anonymous_function_expression_statement(self):
        self.assertRuns("let f = fn (a, b) { return a - b; }; print(f(5, 3), f);", ["2 <fn>"])

    def test_bare_return_and_missing_return_give_nil(self):
        self.assertRuns("fn a() { return; } fn b() {} print(a(), b());", ["nil nil"])

    def test_elif_chain(self):
        self.assertRuns(
            'fn s(n) { if n < 0 { return "neg"; } elif n == 0 { return "zero"; } else { return "pos"; } }'
            " print(s(-1), s(0), s(5));",
            ["neg zero pos"])


class CallSyntaxTests(MiniCase):
    def test_calling_a_non_function(self):
        self.assertFails("let x = 3;\nprint(x(1));", MiniTypeError, 2, 8)


if __name__ == "__main__":
    unittest.main()
