import unittest

from minilang import MiniNameError, MiniRuntimeError, MiniTypeError
from support import MiniCase


class TruthTests(MiniCase):
    def test_only_false_and_nil_are_falsy(self):
        self.assertRuns(
            'fn t(v) { if v { return "T"; } return "F"; } print(t(0), t(""), t([]), t(false), t(nil));',
            ["T T T F F"])

    def test_logical_operators_return_deciding_operand(self):
        self.assertRuns('print(0 or 5, nil or "d", 0 and "x", false and 1, [] or 2);', ['0 d x false []'])

    def test_logical_operators_short_circuit(self):
        self.assertRuns('fn side() { print("side"); return 1; } let a = false and side(); let b = 1 or side(); print(a, b);',
                        ["false 1"])

    def test_not_returns_boolean(self):
        self.assertRuns("print(not 0, not nil, not not 3);", ["false true true"])


class ArithmeticTests(MiniCase):
    def test_division_truncates_toward_zero(self):
        self.assertRuns("print(7 / 2, -7 / 2, 7 / -2, -7 / -2);", ["3 -3 -3 3"])

    def test_remainder_takes_dividend_sign(self):
        self.assertRuns("print(7 % 3, -7 % 3, 7 % -3, -7 % -3);", ["1 -1 1 -1"])

    def test_division_identity_holds(self):
        self.assertRuns("let a = -17; let b = 5; print(a == (a / b) * b + a % b);", ["true"])

    def test_division_by_zero_at_operator(self):
        self.assertFails("print(1);\nlet z = 0;\nprint(10 / z);", MiniRuntimeError, 3, 10, ["1"])

    def test_remainder_by_zero_at_operator(self):
        self.assertFails("print(5 % 0);", MiniRuntimeError, 1, 9)

    def test_booleans_are_not_integers(self):
        self.assertFails("print(1 + true);", MiniTypeError, 1, 9)

    def test_negating_a_boolean(self):
        self.assertFails("let b = true;\nprint(-b);", MiniTypeError, 2, 7)

    def test_mixed_string_and_int_addition(self):
        self.assertFails('print("a" + 1);', MiniTypeError, 1, 11)

    def test_operands_evaluate_before_type_check(self):
        self.assertFails('fn p(v) { print(v); return v; } p(1) - p("s");', MiniTypeError, 1, 38, ["1", "s"])

    def test_string_and_list_concatenation(self):
        self.assertRuns('let a = [1]; let b = a + [2]; push(a, 9); print("ab" + "cd", a, b);', ["abcd [1, 9] [1, 2]"])

    def test_string_repetition_is_not_supported(self):
        self.assertFails('print("ab" * 2);', MiniTypeError, 1, 12)

    def test_big_integer_power(self):
        self.assertRuns("print(2 ** 100);", ["1267650600228229401496703205376"])


class EqualityTests(MiniCase):
    def test_different_types_are_never_equal(self):
        self.assertRuns('print(1 == true, 0 == false, nil == false, "1" == 1, 1 != true);',
                        ["false false false false true"])

    def test_lists_compare_elementwise(self):
        self.assertRuns('print([1, [2, "a"]] == [1, [2, "a"]], [1, 2] == [1, 2, 3], [1] == [true]);',
                        ["true false false"])

    def test_functions_equal_only_themselves(self):
        self.assertRuns("fn f() {} fn g() {} let h = f; print(f == h, f == g, print == print);", ["true false true"])

    def test_ordering_compares_ints_or_strings(self):
        self.assertRuns('print(2 < 10, "10" < "2", "b" >= "a", 3 <= 3);', ["true true true true"])

    def test_ordering_mixed_types_fails_at_operator(self):
        self.assertFails('print(1 < "2");', MiniTypeError, 1, 9)

    def test_ordering_booleans_fails(self):
        self.assertFails("print(false < true);", MiniTypeError, 1, 13)


class IndexTests(MiniCase):
    def test_list_and_string_indexing(self):
        self.assertRuns('let xs = [10, "b"]; let s = "hey"; print(xs[1], s[0] + s[2], len(s[1]));', ["b hy 1"])

    def test_negative_index_is_out_of_range(self):
        self.assertFails("let xs = [1, 2];\nprint(xs[-1]);", MiniRuntimeError, 2, 9)

    def test_index_past_end(self):
        self.assertFails('print("ab"[2]);', MiniRuntimeError, 1, 11)

    def test_boolean_index_is_a_type_error(self):
        self.assertFails("let xs = [1, 2];\nprint(xs[true]);", MiniTypeError, 2, 9)

    def test_indexing_non_indexable(self):
        self.assertFails("let n = 5;\nprint(n[0]);", MiniTypeError, 2, 8)

    def test_list_index_assignment_is_shared(self):
        self.assertRuns("let a = [1, 2]; let b = a; b[0] = 7; print(a);", ["[7, 2]"])

    def test_string_index_assignment_is_rejected(self):
        self.assertFails('let s = "ab";\ns[0] = "c";', MiniTypeError, 2, 2)

    def test_index_assignment_evaluation_order(self):
        self.assertRuns(
            'let xs = [0, 0]; fn t(v) { print(v); return v; } t(xs)[t(1)] = t(5); print(xs);',
            ["[0, 0]", "1", "5", "[0, 5]"])

    def test_index_assignment_out_of_range_after_evaluating_value(self):
        self.assertFails('let xs = [];\nxs[0] = print("v");', MiniRuntimeError, 2, 3, ["v"])


class UndeclaredNameTests(MiniCase):
    def test_reading_undeclared_name(self):
        self.assertFails("print(1);\nprint(y + 1);", MiniNameError, 2, 7, ["1"])

    def test_assigning_undeclared_name_evaluates_value_first(self):
        self.assertFails('ghost = print("value");', MiniNameError, 1, 1, ["value"])


if __name__ == "__main__":
    unittest.main()
