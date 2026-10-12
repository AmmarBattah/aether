import re

CELL = re.compile(r"([A-Z])([0-9]+)")


def parse_cell(name):
    match = CELL.fullmatch(name.upper())
    if match is None:
        raise ValueError(f"bad cell name {name!r}")
    return int(match[2]), ord(match[1]) - ord("A") + 1


def cell_name(row, col):
    return f"{chr(ord('A') + col - 1)}{row}"
