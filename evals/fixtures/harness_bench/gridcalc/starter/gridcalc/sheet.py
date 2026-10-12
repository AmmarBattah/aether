from .formula import evaluate, tokenize
from .refs import cell_name, parse_cell


class Sheet:
    def __init__(self, on_evaluate=None):
        self._cells = {}
        self._on_evaluate = on_evaluate

    def set(self, name, raw):
        key = parse_cell(name)
        if raw is None or raw == "":
            self._cells.pop(key, None)
            return
        if isinstance(raw, str) and raw.startswith("="):
            tokenize(raw[1:])
        self._cells[key] = raw

    def _value(self, row, col):
        raw = self._cells.get((row, col))
        if not (isinstance(raw, str) and raw.startswith("=")):
            return raw
        if self._on_evaluate is not None:
            self._on_evaluate(cell_name(row, col))
        return evaluate(raw[1:], self._value)

    def get(self, name):
        return self._value(*parse_cell(name))

    def raw(self, name):
        return self._cells.get(parse_cell(name))

    def text(self, name):
        value = self.get(name)
        return "" if value is None else str(value)

    def cells(self):
        return [cell_name(*key) for key in sorted(self._cells)]

    def precedents(self, name):
        return []

    def dependents(self, name):
        return []

    def copy(self, src, dst):
        self.set(dst, self.raw(src))

    def insert_rows(self, before, count=1):
        raise NotImplementedError("structural edits are not implemented yet")

    def delete_rows(self, first, count=1):
        raise NotImplementedError("structural edits are not implemented yet")

    def insert_cols(self, before, count=1):
        raise NotImplementedError("structural edits are not implemented yet")

    def delete_cols(self, first, count=1):
        raise NotImplementedError("structural edits are not implemented yet")
