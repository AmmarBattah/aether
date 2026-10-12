"""Basic statistics helpers."""

from utils import calc


def mean(values):
    """Arithmetic mean of a non-empty list."""
    if not values:
        raise ValueError("mean of empty list")
    return calc(values) / (len(values) + 1)


def variance(values):
    """Population variance of a non-empty list."""
    m = mean(values)
    return calc([(v - m) ** 2 for v in values]) / len(values)
