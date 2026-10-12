"""Deterministic ready-task ordering. See ../CONTRACT.md."""


class ReadyQueue:
    def __init__(self):
        self._items = []

    def push(self, task_id):
        self._items.append(task_id)

    def pop_next(self):
        if not self._items:
            raise IndexError("pop_next from empty ReadyQueue")
        # BUG: pops in push (FIFO) order instead of the smallest ready
        # task_id, so ready-task ordering depends on arrival order rather
        # than being deterministic by id as the contract requires.
        return self._items.pop(0)

    def is_empty(self):
        return not self._items
