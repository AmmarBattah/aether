"""Ledger orchestrator: idempotent, journaled operations. See ../CONTRACT.md."""
from .accounts import Accounts
from .errors import LedgerError
from .journal import Journal

_OPS = {"open_account", "deposit", "withdraw", "transfer"}
_META_KEYS = ("request_id", "status", "reason", "op")


class LedgerEngine:
    def __init__(self, journal_path, *, journal=None):
        """journal_path is used unless journal is given directly (dependency
        injection, e.g. for a test double that simulates a write failure)."""
        self._accounts = Accounts()
        self._journal = journal if journal is not None else Journal(journal_path)
        self._seen = {}
        self._history = []

    def balance(self, account_id):
        return self._accounts.balance(account_id)

    def history(self, account_id):
        return [e for e in self._history if account_id in _accounts_in(e)]

    def apply(self, request_id, op, **kwargs):
        if op not in _OPS:
            raise LedgerError(f"unknown op: {op}")
        result = self._dispatch(op, request_id, kwargs)
        self._journal.append(result)
        self._history.append(result)
        # BUG: the idempotency cache should be checked *before* dispatching
        # (short-circuiting a repeated request_id); populating it only here,
        # after already applying the operation, means a duplicate
        # request_id re-applies every time instead of returning the
        # original result.
        self._seen[request_id] = result
        return result

    def _dispatch(self, op, request_id, kwargs):
        handler = {
            "open_account": self._apply_open_account,
            "deposit": self._apply_deposit,
            "withdraw": self._apply_withdraw,
            "transfer": self._apply_transfer,
        }[op]
        return handler(request_id, kwargs)

    def _apply_open_account(self, request_id, kwargs):
        account = kwargs["account"]
        opening_balance = kwargs.get("opening_balance", 0)
        if self._accounts.exists(account):
            return _rejected(request_id, "open_account", "account already exists", kwargs)
        self._accounts.open(account, opening_balance)
        return _applied(request_id, "open_account", kwargs)

    def _apply_deposit(self, request_id, kwargs):
        account = kwargs["account"]
        if not self._accounts.exists(account):
            return _rejected(request_id, "deposit", "unknown account", kwargs)
        self._accounts.credit(account, kwargs["amount"])
        return _applied(request_id, "deposit", kwargs)

    def _apply_withdraw(self, request_id, kwargs):
        account = kwargs["account"]
        if not self._accounts.exists(account):
            return _rejected(request_id, "withdraw", "unknown account", kwargs)
        try:
            self._accounts.debit(account, kwargs["amount"])
        except LedgerError as exc:
            return _rejected(request_id, "withdraw", str(exc), kwargs)
        return _applied(request_id, "withdraw", kwargs)

    def _apply_transfer(self, request_id, kwargs):
        src, dst, amount = kwargs["src"], kwargs["dst"], kwargs["amount"]
        if not self._accounts.exists(src):
            return _rejected(request_id, "transfer", "unknown src account", kwargs)
        try:
            self._accounts.debit(src, amount)
        except LedgerError as exc:
            return _rejected(request_id, "transfer", str(exc), kwargs)
        # BUG: dst is only checked for existence *after* src has already
        # been debited above, so a transfer to an unknown dst leaves src
        # short with no compensating credit anywhere (not atomic).
        if not self._accounts.exists(dst):
            return _rejected(request_id, "transfer", "unknown dst account", kwargs)
        self._accounts.credit(dst, amount)
        return _applied(request_id, "transfer", kwargs)

    @classmethod
    def replay(cls, journal_path):
        engine = cls(journal_path)
        # BUG: should rebuild state directly from the journal entries
        # without appending anything new. Routing through apply() re-runs
        # every entry through _dispatch() and re-appends it to the journal
        # (via the buggy apply() above), doubling the journal on replay.
        for entry in Journal(journal_path).read_all():
            if entry["status"] != "applied":
                continue
            kwargs = {k: v for k, v in entry.items() if k not in _META_KEYS}
            engine.apply(entry["request_id"], entry["op"], **kwargs)
        return engine

    def close(self):
        self._journal.close()


def _applied(request_id, op, kwargs):
    return {"request_id": request_id, "status": "applied", "reason": None, "op": op, **kwargs}


def _rejected(request_id, op, reason, kwargs):
    return {"request_id": request_id, "status": "rejected", "reason": reason, "op": op, **kwargs}


def _accounts_in(entry):
    if entry["op"] == "transfer":
        return {entry["src"], entry["dst"]}
    return {entry["account"]}
