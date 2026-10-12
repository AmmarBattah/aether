"""Errors raised by the jobrunner package. See ../CONTRACT.md."""


class JobError(Exception):
    """Invalid job, step, or hook definition, or a checkpoint/job mismatch."""


class CheckpointError(JobError):
    """A checkpoint on disk is unreadable and cannot be repaired."""


class InjectedFault(Exception):
    """Raised by a fault hook to simulate the process dying at a boundary.

    The runner must never catch this: it stands in for a crash, so the only
    thing that may survive it is what is already durable on disk.
    """
