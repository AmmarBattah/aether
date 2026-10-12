"""Journal record shape and whole-journal replay validation."""
import copy

from .errors import Invalid
from .validate import PARAMETERS

RECORD_KEYS = {"sequence", "operation", "arguments"}


def make_record(sequence, operation, arguments):
    assert operation in PARAMETERS, f"unknown operation {operation!r}"
    assert tuple(arguments) == PARAMETERS[operation], "arguments must be the method's parameters"
    return {"sequence": sequence, "operation": operation, "arguments": copy.deepcopy(arguments)}


def parse_record(record, expected_sequence):
    """Return (operation, arguments) for a well-formed record or raise Invalid."""
    if type(record) is not dict or set(record) != RECORD_KEYS:
        raise Invalid("journal record must have exactly sequence, operation and arguments")
    if type(record["sequence"]) is not int or record["sequence"] != expected_sequence:
        raise Invalid("journal sequence is not contiguous")
    operation, arguments = record["operation"], record["arguments"]
    if type(operation) is not str or operation not in PARAMETERS:
        raise Invalid("unknown journal operation")
    if type(arguments) is not dict or set(arguments) != set(PARAMETERS[operation]):
        raise Invalid("journal arguments do not match the operation")
    return operation, {name: copy.deepcopy(arguments[name]) for name in PARAMETERS[operation]}
