import unittest

from minilang import MiniNameError, run


class BasicTests(unittest.TestCase):
    def test_arithmetic_and_strings(self):
        self.assertEqual(run('print(1 + 2 * 3, "a" + "b", 10 - 4);'), ["7 ab 6"])

    def test_if_and_while(self):
        source = """
        let total = 0;
        let i = 1;
        while i <= 4 {
          if i == 3 { total = total + 30; } else { total = total + i; }
          i = i + 1;
        }
        print(total);
        """
        self.assertEqual(run(source), ["37"])

    def test_recursive_function(self):
        source = "fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); } print(fib(10));"
        self.assertEqual(run(source), ["55"])

    def test_lists(self):
        source = "let xs = [1, 2]; push(xs, 3); xs[0] = 5; print(xs, len(xs), xs[2]);"
        self.assertEqual(run(source), ["[5, 2, 3] 3 3"])

    def test_undeclared_name(self):
        with self.assertRaises(MiniNameError) as caught:
            run("let a = 1;\nprint(b);")
        self.assertEqual((caught.exception.line, caught.exception.column), (2, 7))


if __name__ == "__main__":
    unittest.main()
