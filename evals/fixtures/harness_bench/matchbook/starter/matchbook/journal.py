"""Journal records for the matching engine."""


def make_record(sequence, operation, arguments):
    return {"sequence": sequence, "operation": operation, "arguments": arguments}


def read_record(record):
    return record["operation"], record["arguments"]
