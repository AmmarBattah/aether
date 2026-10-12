"""A deterministic, manually-advanced virtual clock. See ../CONTRACT.md."""


class Clock:
    def __init__(self, start=0.0):
        self._now = start

    def now(self):
        return self._now

    def advance(self, seconds):
        if seconds < 0:
            raise ValueError("cannot advance a clock backwards")
        # BUG: overwrites the virtual time instead of accumulating it, so a
        # sequence of small advances loses all previously elapsed time.
        self._now = seconds
        return self._now
