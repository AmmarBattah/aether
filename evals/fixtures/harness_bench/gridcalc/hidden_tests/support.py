"""Shared helpers for the held-out gridcalc checks."""
import unittest

from gridcalc import CellError, Sheet

SCRATCH = "Z99"


def sheet(cells=None, log=None):
    made = Sheet(None if log is None else log.append)
    for name, raw in (cells or {}).items():
        made.set(name, raw)
    return made


def value(formula, cells=None):
    """Value of a formula typed into an otherwise unused cell."""
    made = sheet(cells)
    made.set(SCRATCH, formula)
    return made.get(SCRATCH)


def error(code):
    return CellError(code)


class GridTest(unittest.TestCase):
    def assertValue(self, actual, expected):
        self.assertEqual((type(actual), actual), (type(expected), expected))

    def assertFormula(self, formula, expected, cells=None):
        self.assertValue(value(formula, cells), expected)
