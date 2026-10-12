CODES = ("#DIV/0!", "#VALUE!", "#REF!", "#NAME?", "#NUM!", "#CYCLE!")


class FormulaError(ValueError):
    pass


class CellError:
    __slots__ = ("code",)

    def __init__(self, code):
        assert code in CODES, f"unknown error code {code!r}"
        self.code = code

    def __eq__(self, other):
        return isinstance(other, CellError) and other.code == self.code

    def __hash__(self):
        return hash(("CellError", self.code))

    def __repr__(self):
        return f"CellError({self.code!r})"
