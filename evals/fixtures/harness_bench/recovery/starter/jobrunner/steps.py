"""Job step definition and validation. See ../CONTRACT.md."""
from .errors import JobError


class Step:
    """One resumable unit of work.

    `action(context)` performs the side effect and returns a dict of context
    updates. `probe(context)` reports whether that side effect is already
    present, which is how the runner resolves a crash between the action and
    its completion checkpoint without repeating the effect.
    """

    __slots__ = ("name", "action", "probe")

    def __init__(self, name, action, probe):
        if not isinstance(name, str) or not name:
            raise JobError(f"step name must be a non-empty string, got {name!r}")
        if not callable(action):
            raise JobError(f"step {name!r} action must be callable, got {action!r}")
        if not callable(probe):
            raise JobError(f"step {name!r} probe must be callable, got {probe!r}")
        self.name = name
        self.action = action
        self.probe = probe

    def __repr__(self):
        return f"Step({self.name!r})"


def validate_steps(steps):
    """Return `steps` as a tuple, raising JobError for an unusable job."""
    ordered = tuple(steps)
    if not ordered:
        raise JobError("a job needs at least one step")
    seen = set()
    for step in ordered:
        if not isinstance(step, Step):
            raise JobError(f"job steps must be Step instances, got {step!r}")
        if step.name in seen:
            raise JobError(f"duplicate step name: {step.name!r}")
        seen.add(step.name)
    assert len(seen) == len(ordered), "every step name must be unique"
    return ordered
