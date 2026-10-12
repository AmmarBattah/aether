import heapq
import math
from collections import namedtuple

from .errors import CellError
from .evaluate import formula_value
from .parser import parse, references
from .refs import MAX_COL, MAX_ROW, cell_name, parse_cell, parse_column
from .rewrite import deleted, inserted, keep, rewrite, shifted
from .tokens import tokenize
from .values import display, normalize

Formula = namedtuple("Formula", "text ast precedents")
CYCLE = CellError("#CYCLE!")


def compile_formula(text, transform=keep):
    """Parse the text after `=`; references are rewritten into their canonical spelling."""
    canonical = rewrite(text, transform)
    ast = parse(tokenize(canonical))
    return Formula(canonical, ast, tuple(sorted(references(ast))))


def _content(raw):
    if raw is None or raw == "":
        return None
    if isinstance(raw, str):
        return compile_formula(raw[1:]) if raw.startswith("=") else raw
    if isinstance(raw, (bool, int)):
        return raw
    if not isinstance(raw, float) or not math.isfinite(raw):
        raise ValueError(f"unsupported cell content {raw!r}")
    return normalize(raw)


def _count(count):
    if type(count) is not int or count < 1:
        raise ValueError(f"count must be a positive int, not {count!r}")
    return count


def _row(row):
    if type(row) is not int or not 1 <= row <= MAX_ROW:
        raise ValueError(f"bad row {row!r}")
    return row


class Sheet:
    def __init__(self, on_evaluate=None):
        self._cells = {}
        self._values = {}
        self._on_evaluate = on_evaluate

    def set(self, name, raw):
        key = parse_cell(name)
        self._store(key, _content(raw))
        self._recalculate([key])

    def copy(self, src, dst):
        source, target = parse_cell(src), parse_cell(dst)
        content = self._cells.get(source)
        if isinstance(content, Formula):
            content = compile_formula(content.text, shifted(target[0] - source[0], target[1] - source[1]))
        self._store(target, content)
        self._recalculate([target])

    def _store(self, key, content):
        self._values.pop(key, None)
        if content is None:
            self._cells.pop(key, None)
        else:
            self._cells[key] = content

    def insert_rows(self, before, count=1):
        self._insert("row", _row(before), _count(count))

    def insert_cols(self, before, count=1):
        self._insert("col", parse_column(before), _count(count))

    def delete_rows(self, first, count=1):
        self._delete("row", _row(first), _count(count), MAX_ROW)

    def delete_cols(self, first, count=1):
        self._delete("col", parse_column(first), _count(count), MAX_COL)

    def _insert(self, axis, before, count):
        index = 0 if axis == "row" else 1
        limit = MAX_ROW if axis == "row" else MAX_COL
        if any(key[index] >= before and key[index] + count > limit for key in self._cells):
            raise ValueError("insert would move a cell outside the grid")

        def move(key):
            return key if key[index] < before else _moved(key, index, count)
        self._restructure(move, inserted(axis, before, count))

    def _delete(self, axis, first, count, limit):
        if first + count - 1 > limit:
            raise ValueError("the deleted band must lie inside the grid")
        index = 0 if axis == "row" else 1

        def move(key):
            if first <= key[index] < first + count:
                return None
            return key if key[index] < first else _moved(key, index, -count)
        self._restructure(move, deleted(axis, first, count))

    def _restructure(self, move, transform):
        """Build the whole new sheet first, so an edit that raises changes nothing."""
        cells = {}
        for key, content in self._cells.items():
            target = move(key)
            if target is None:
                continue
            assert target not in cells, f"two cells moved to {target}"
            cells[target] = compile_formula(content.text, transform) if isinstance(content, Formula) else content
        self._cells, self._values = cells, {}
        self._recalculate(None)

    def _formulas(self):
        return {key: content for key, content in self._cells.items() if isinstance(content, Formula)}

    def _recalculate(self, seeds):
        """Evaluate the formula cells reachable from seeds (all when None), dependencies first."""
        formulas = self._formulas()
        dependents = {}
        for key, formula in formulas.items():
            for precedent in formula.precedents:
                dependents.setdefault(precedent, set()).add(key)
        order = _evaluation_order(formulas, dependents)
        affected = set(formulas) if seeds is None else _reachable(seeds, dependents) & set(formulas)
        for key in affected - set(order):
            self._values[key] = CYCLE
        for key in order:
            if key not in affected:
                continue
            self._values[key] = formula_value(formulas[key].ast, self._lookup)
            if self._on_evaluate is not None:
                self._on_evaluate(cell_name(*key))
        assert set(self._values) == set(formulas), "every formula cell must hold a value"

    def _lookup(self, row, col):
        content = self._cells.get((row, col))
        return self._values[(row, col)] if isinstance(content, Formula) else content

    def get(self, name):
        return self._lookup(*parse_cell(name))

    def raw(self, name):
        content = self._cells.get(parse_cell(name))
        return "=" + content.text if isinstance(content, Formula) else content

    def text(self, name):
        return display(self.get(name))

    def cells(self):
        return [cell_name(*key) for key in sorted(self._cells)]

    def precedents(self, name):
        content = self._cells.get(parse_cell(name))
        return [cell_name(*key) for key in content.precedents] if isinstance(content, Formula) else []

    def dependents(self, name):
        key = parse_cell(name)
        return [cell_name(*other) for other, formula in sorted(self._formulas().items()) if key in formula.precedents]


def _moved(key, index, delta):
    return (key[0] + delta, key[1]) if index == 0 else (key[0], key[1] + delta)


def _reachable(seeds, dependents):
    seen, stack = set(seeds), list(seeds)
    while stack:
        for dependent in dependents.get(stack.pop(), ()):
            if dependent not in seen:
                seen.add(dependent)
                stack.append(dependent)
    return seen


def _evaluation_order(formulas, dependents):
    """Formula cells with dependencies first; cells in or behind a cycle are left out."""
    waiting = {key: sum(precedent in formulas for precedent in formula.precedents)
               for key, formula in formulas.items()}
    ready = [key for key, count in waiting.items() if count == 0]
    heapq.heapify(ready)
    order = []
    while ready:
        key = heapq.heappop(ready)
        order.append(key)
        for dependent in dependents.get(key, ()):
            waiting[dependent] -= 1
            if waiting[dependent] == 0:
                heapq.heappush(ready, dependent)
    assert len(order) == len(set(order)) <= len(formulas), "a cell was ordered twice"
    return order
