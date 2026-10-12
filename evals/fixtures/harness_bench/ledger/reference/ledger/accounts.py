"""In-memory account balances. Full contract in ../../starter/CONTRACT.md."""
from .errors import LedgerError


class Accounts:
    def __init__(self):
        self._balances = {}

    def exists(self, account_id):
        return account_id in self._balances

    def open(self, account_id, opening_balance=0):
        if account_id in self._balances:
            raise LedgerError(f"account already exists: {account_id}")
        if opening_balance < 0:
            raise LedgerError("opening balance cannot be negative")
        self._balances[account_id] = opening_balance

    def balance(self, account_id):
        if account_id not in self._balances:
            raise LedgerError(f"unknown account: {account_id}")
        return self._balances[account_id]

    def credit(self, account_id, amount):
        if amount <= 0:
            raise LedgerError("credit amount must be positive")
        self._balances[account_id] = self.balance(account_id) + amount

    def debit(self, account_id, amount):
        if amount <= 0:
            raise LedgerError("debit amount must be positive")
        current = self.balance(account_id)
        if amount > current:
            raise LedgerError(f"insufficient funds: {account_id} has {current}, needs {amount}")
        self._balances[account_id] = current - amount

    def snapshot(self):
        """A point-in-time copy of all balances, for restore() on rollback."""
        return dict(self._balances)

    def restore(self, snapshot):
        self._balances = dict(snapshot)
