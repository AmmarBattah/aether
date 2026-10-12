from .refs import MAX_COL, MAX_ROW, corner_text, in_grid, order_corners
from .tokens import REF_ERROR, tokenize

AXIS_LIMIT = {"row": MAX_ROW, "col": MAX_COL}


def rewrite(text, transform):
    """Formula text with every reference replaced by transform(first, second); None means #REF!."""
    pieces, cursor = [], 0
    for token in tokenize(text):
        if token.kind not in ("ref", "referr"):
            continue
        moved = transform(*token.value) if token.kind == "ref" else None
        pieces += [text[cursor:token.start], REF_ERROR if moved is None else _reference_text(*moved)]
        cursor = token.end
    assert cursor <= len(text), "reference spans ran past the text"
    return "".join(pieces) + text[cursor:]


def _reference_text(first, second):
    assert in_grid(first.row, first.col), f"reference left the grid: {first}"
    return corner_text(first) if second is None else f"{corner_text(first)}:{corner_text(second)}"


def keep(first, second):
    return first, second


def inserted(axis, before, count):
    """Transform for inserting `count` rows or columns before `before`."""
    def move(corner):
        position = getattr(corner, axis)
        if position < before:
            return corner
        if position + count > AXIS_LIMIT[axis]:
            raise ValueError("insert would move a reference outside the grid")
        return corner._replace(**{axis: position + count})
    return lambda first, second: (move(first), None if second is None else move(second))


def deleted(axis, start, count):
    """Transform for deleting the band start..start+count-1 of rows or columns."""
    end = start + count - 1

    def transform(first, second):
        low, high = getattr(first, axis), getattr(second or first, axis)
        if start <= low and high <= end:
            return None
        new_low = start if start <= low <= end else low - count if low > end else low
        new_high = start - 1 if start <= high <= end else high - count if high > end else high
        assert new_low <= new_high, "a surviving range cannot be empty"
        first = first._replace(**{axis: new_low})
        return first, None if second is None else second._replace(**{axis: new_high})
    return transform


def shifted(rows, cols):
    """Transform for copying a formula `rows` down and `cols` right."""
    def move(corner):
        row = corner.row if corner.row_abs else corner.row + rows
        col = corner.col if corner.col_abs else corner.col + cols
        return corner._replace(row=row, col=col) if in_grid(row, col) else None

    def transform(first, second):
        moved = move(first)
        if second is None:
            return None if moved is None else (moved, None)
        other = move(second)
        return None if moved is None or other is None else order_corners(moved, other)
    return transform
