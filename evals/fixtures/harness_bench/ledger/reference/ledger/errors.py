"""Errors raised by the ledger package. See ../CONTRACT.md."""


class LedgerError(Exception):
    """Invalid ledger usage: unknown op, bad account, bad amount."""
