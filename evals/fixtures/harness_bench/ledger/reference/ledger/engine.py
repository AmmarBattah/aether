"""Ledger orchestrator: idempotent, journaled operations.

Full contract in ../../starter/CONTRACT.md. This is the reference
implementation used only to validate the grader; it is never copied into a
solver workspace.
"""
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
        return [dict(e) for e in self._history if account_id in _accounts_in(e)]

    def apply(self, request_id, op, **kwargs):
        if op not in _OPS:
            raise LedgerError(f"unknown op: {op}")
        if request_id in self._seen:
            return dict(self._seen[request_id])
        snapshot = self._accounts.snapshot()
        result = self._dispatch(op, request_id, kwargs)
        try:
            self._journal.append(result)
        except Exception:
            # The op already mutated in-memory balances above; if it cannot
            # be durably recorded, roll back so state matches the journal.
            self._accounts.restore(snapshot)
            raise
        self._record(result)
        return dict(result)

    def _record(self, result):
        self._history.append(result)
        self._seen[result["request_id"]] = result

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
        rejection = self._transfer_rejection_reason(src, dst, amount)
        if rejection is not None:
            return _rejected(request_id, "transfer", rejection, kwargs)
        # Both accounts and sufficient funds are confirmed above before any
        # mutation happens, so a rejected transfer never touches a balance.
        self._accounts.debit(src, amount)
        self._accounts.credit(dst, amount)
        return _applied(request_id, "transfer", kwargs)

    def _transfer_rejection_reason(self, src, dst, amount):
        if not self._accounts.exists(src):
            return "unknown src account"
        if not self._accounts.exists(dst):
            return "unknown dst account"
        if self._accounts.balance(src) < amount:
            return "insufficient funds"
        return None

    @classmethod
    def replay(cls, journal_path):
        engine = cls(journal_path)
        reader = Journal(journal_path)
        try:
            entries = reader.read_all()
        finally:
            reader.close()
        for entry in entries:
            if entry["status"] == "applied":
                _reapply_to_accounts(engine._accounts, entry)
            engine._record(entry)
        return engine

    def close(self):
        self._journal.close()


def _reapply_to_accounts(accounts, entry):
    op = entry["op"]
    kwargs = {k: v for k, v in entry.items() if k not in _META_KEYS}
    if op == "open_account":
        accounts.open(kwargs["account"], kwargs.get("opening_balance", 0))
    elif op == "deposit":
        accounts.credit(kwargs["account"], kwargs["amount"])
    elif op == "withdraw":
        accounts.debit(kwargs["account"], kwargs["amount"])
    elif op == "transfer":
        accounts.debit(kwargs["src"], kwargs["amount"])
        accounts.credit(kwargs["dst"], kwargs["amount"])


def _applied(request_id, op, kwargs):
    return {"request_id": request_id, "status": "applied", "reason": None, "op": op, **kwargs}


def _rejected(request_id, op, reason, kwargs):
    return {"request_id": request_id, "status": "rejected", "reason": reason, "op": op, **kwargs}


def _accounts_in(entry):
    if entry["op"] == "transfer":
        return {entry["src"], entry["dst"]}
    return {entry["account"]}
