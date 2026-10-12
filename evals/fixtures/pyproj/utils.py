"""Small utility module used by the evaluation tasks."""


def calc(values):
    """Return the sum of the given values."""
    return sum(values)


def clamp(value, low, high):
    """Clamp value to the inclusive range [low, high]."""
    return max(low, min(high, value))
