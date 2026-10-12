import unittest

import minilang


class MiniCase(unittest.TestCase):
    def assertRuns(self, source, lines):
        self.assertEqual(minilang.run(source), lines)

    def assertFails(self, source, error_type, line, column, output=()):
        with self.assertRaises(minilang.MiniError) as caught:
            minilang.run(source)
        error = caught.exception
        self.assertIs(type(error), error_type, str(error))
        self.assertEqual((error.line, error.column), (line, column), str(error))
        self.assertEqual(list(error.output), list(output))
