"""Deterministic ready-task ordering. See ../../starter/CONTRACT.md."""
import bisect


class ReadyQueue:
    def __init__(self):
        self._items = []

    def push(self, task_id):
        pos = bisect.bisect_left(self._items, task_id)
        self._items.insert(pos, task_id)

    def pop_next(self):
        if not self._items:
            raise IndexError("pop_next from empty ReadyQueue")
        return self._items.pop(0)

    def is_empty(self):
        return not self._items
