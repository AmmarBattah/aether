import re
from collections import namedtuple

MAX_ROW, MAX_COL = 9999, 702
CELL = re.compile(r"([A-Za-z]{1,2})([1-9][0-9]{0,3})")
COLUMN = re.compile(r"[A-Za-z]{1,2}")
Corner = namedtuple("Corner", "row col row_abs col_abs")


def column_number(letters):
    number = 0
    for letter in letters.upper():
        number = number * 26 + ord(letter) - ord("A") + 1
    assert 1 <= number <= MAX_COL, f"column {letters!r} is outside the grid"
    return number


def column_letters(number):
    assert 1 <= number <= MAX_COL, f"column {number} is outside the grid"
    letters = ""
    while number:
        number, remainder = divmod(number - 1, 26)
        letters = chr(ord("A") + remainder) + letters
    return letters


def parse_cell(name):
    match = CELL.fullmatch(name) if isinstance(name, str) else None
    if match is None:
        raise ValueError(f"bad cell name {name!r}")
    return int(match[2]), column_number(match[1])


def parse_column(letters):
    if not isinstance(letters, str) or not COLUMN.fullmatch(letters):
        raise ValueError(f"bad column {letters!r}")
    return column_number(letters)


def cell_name(row, col):
    assert in_grid(row, col), f"cell ({row}, {col}) is outside the grid"
    return f"{column_letters(col)}{row}"


def in_grid(row, col):
    return 1 <= row <= MAX_ROW and 1 <= col <= MAX_COL


def order_corners(first, second):
    """Top-left first: column parts and row parts swap separately, each keeping its `$`."""
    if first.col > second.col:
        first, second = (first._replace(col=second.col, col_abs=second.col_abs),
                         second._replace(col=first.col, col_abs=first.col_abs))
    if first.row > second.row:
        first, second = (first._replace(row=second.row, row_abs=second.row_abs),
                         second._replace(row=first.row, row_abs=first.row_abs))
    assert first.row <= second.row and first.col <= second.col, "corners are still out of order"
    return first, second


def corner_text(corner):
    column = ("$" if corner.col_abs else "") + column_letters(corner.col)
    return column + ("$" if corner.row_abs else "") + str(corner.row)
