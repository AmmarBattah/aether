"""Errors raised by the scheduler package. See ../CONTRACT.md."""


class SchedulerError(Exception):
    """Invalid task graph: duplicate/unknown id, or a dependency cycle."""
