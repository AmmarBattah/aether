import sys
import unittest

import minilang
from minilang import MiniNameError, MiniRuntimeError, MiniTypeError
from support import MiniCase

COUNTER = """
fn counter() {
  let n = 0;
  return fn () { n = n + 1; return n; };
}
"""


class ScopeTests(MiniCase):
    def test_redeclaring_in_same_scope(self):
        self.assertFails("let a = 1;\nprint(a);\nlet a = 2;", MiniNameError, 3, 5, ["1"])

    def test_shadowing_in_block(self):
        self.assertRuns("let a = 1; if true { let a = 2; print(a); } print(a);", ["2", "1"])

    def test_let_initializer_sees_outer_binding(self):
        self.assertRuns("let a = 1; if true { let a = a + 10; print(a); }", ["11"])

    def test_function_redeclaring_name(self):
        self.assertFails("let f = 1;\nfn f() {}", MiniNameError, 2, 4)

    def test_parameter_and_local_share_scope(self):
        self.assertFails("fn f(a) {\n  let a = 2;\n}\nf(1);", MiniNameError, 2, 7)

    def test_builtins_can_be_shadowed(self):
        self.assertRuns('let len = fn (x) { return 42; }; print(len("abc"));', ["42"])

    def test_block_scope_ends(self):
        self.assertFails("if true { let inner = 1; }\nprint(inner);", MiniNameError, 2, 7)

    def test_assignment_updates_outer_variable(self):
        self.assertRuns("let total = 0; let i = 0; while i < 4 { total = total + i; i = i + 1; } print(total);", ["6"])

    def test_while_iterations_get_fresh_scope(self):
        self.assertRuns(
            "let fs = []; let i = 0; while i < 3 { let j = i; push(fs, fn () { return j; }); i = i + 1; }"
            " print(fs[0](), fs[1](), fs[2]());",
            ["0 1 2"])

    def test_break_and_continue(self):
        self.assertRuns(
            "let i = 0; let seen = []; while true { i = i + 1; if i % 2 == 0 { continue; } if i > 7 { break; }"
            " push(seen, i); } print(seen);",
            ["[1, 3, 5, 7]"])

    def test_return_from_inside_loop(self):
        self.assertRuns("fn first(xs) { let i = 0; while true { if xs[i] > 2 { return i; } i = i + 1; } }"
                        " print(first([1, 5, 3]));", ["1"])


class ClosureTests(MiniCase):
    def test_closures_keep_independent_state(self):
        self.assertRuns(COUNTER + "let a = counter(); let b = counter(); a(); a(); print(a(), b());", ["3 1"])

    def test_closures_see_later_assignments(self):
        self.assertRuns("let x = 1; fn get() { return x; } x = 2; print(get());", ["2"])

    def test_closure_assignment_updates_captured_variable(self):
        self.assertRuns("let x = 1; fn bump() { x = x * 10; } bump(); bump(); print(x);", ["100"])

    def test_function_declared_later_in_scope_is_visible(self):
        self.assertRuns("fn a() { return b(); } fn b() { return 7; } print(a());", ["7"])

    def test_mutual_recursion(self):
        self.assertRuns(
            "fn even(n) { if n == 0 { return true; } return odd(n - 1); }"
            " fn odd(n) { if n == 0 { return false; } return even(n - 1); } print(even(10), odd(7));",
            ["true true"])

    def test_call_arguments_evaluate_left_to_right(self):
        self.assertRuns('fn t(v) { print(v); return v; } fn add(a, b) { return a + b; } print(add(t(1), t(2)));',
                        ["1", "2", "3"])


class CallTests(MiniCase):
    def test_too_few_arguments(self):
        self.assertFails("fn f(a, b) { return a; }\nprint(f(1));", MiniTypeError, 2, 8)

    def test_too_many_arguments(self):
        self.assertFails("let f = fn (a) { return a; };\nf(1, 2);", MiniTypeError, 2, 2)

    def test_arity_checked_after_arguments(self):
        self.assertFails('fn f() {}\nf(print("arg"));', MiniTypeError, 2, 2, ["arg"])

    def test_builtin_arity_error(self):
        self.assertFails("print(len());", MiniTypeError, 1, 10)

    def test_builtin_type_error(self):
        self.assertFails("let n = 5;\npush(n, 1);", MiniTypeError, 2, 5)


class RecursionLimitTests(MiniCase):
    RECURSE = "fn depth(n) {\n  if n == 1 { return 1; }\n  return 1 + depth(n - 1);\n}\n"

    def test_one_hundred_active_calls_are_allowed(self):
        self.assertRuns(self.RECURSE + "print(depth(100));", ["100"])

    def test_one_hundred_and_first_call_fails_at_its_paren(self):
        self.assertFails('print("start");\n' + self.RECURSE + "print(depth(101));",
                         MiniRuntimeError, 4, 19, ["start"])

    def test_unbounded_recursion_is_a_runtime_error(self):
        self.assertFails("fn loop() { return loop(); }\nloop();", MiniRuntimeError, 1, 24)

    def test_deep_frames_do_not_hit_python_limit(self):
        source = ("fn walk(n) { let i = 0; while i < 1 { if n > 1 { let r = [walk(n - 1)]; return r[0] + 1; }"
                  " i = i + 1; } return 1; } print(walk(100));")
        previous = sys.getrecursionlimit()
        try:
            sys.setrecursionlimit(1000)
            self.assertEqual(minilang.run(source), ["100"])
            self.assertEqual(sys.getrecursionlimit(), 1000)
        finally:
            sys.setrecursionlimit(previous)

    def test_builtin_calls_do_not_count(self):
        self.assertRuns(self.RECURSE.replace("1 + depth", "len([1]) + depth") + "print(depth(100));", ["100"])


class DisplayTests(MiniCase):
    def test_print_joins_display_forms(self):
        self.assertRuns('print(1, "two", true, nil, [], print);', ["1 two true nil [] <builtin print>"])

    def test_print_without_arguments(self):
        self.assertRuns("print();", [""])

    def test_nested_strings_are_quoted_and_escaped(self):
        self.assertRuns(r'print(["a\"b", "c\\d", "e\nf", ["\t"]]);', [r'["a\"b", "c\\d", "e\nf", ["\t"]]'])

    def test_str_matches_print(self):
        self.assertRuns('let s = str(["x", 1, nil]); print(s, len(s));', ['["x", 1, nil] 13'])

    def test_self_containing_list(self):
        self.assertRuns("let a = [1]; push(a, a); let b = [a]; print(a, b);", ["[1, [...]] [[1, [...]]]"])

    def test_shared_list_is_not_a_cycle(self):
        self.assertRuns("let a = [1]; print([a, a]);", ["[[1], [1]]"])

    def test_function_display(self):
        self.assertRuns("fn named() {} let anon = fn () {}; print(named, anon, str(named));",
                        ["<fn named> <fn> <fn named>"])

    def test_push_returns_nil(self):
        self.assertRuns("let xs = []; print(push(xs, 1), xs);", ["nil [1]"])


class IsolationTests(MiniCase):
    def test_runs_do_not_share_state(self):
        self.assertRuns("let x = 1; print(x);", ["1"])
        self.assertRuns("let x = 2; print(x);", ["2"])

    def test_error_output_is_a_snapshot(self):
        with self.assertRaises(MiniRuntimeError) as caught:
            minilang.run('print("a");\nprint("b");\nprint(1 / 0);\nprint("c");')
        self.assertEqual(caught.exception.output, ["a", "b"])


if __name__ == "__main__":
    unittest.main()
