class InvalidVersion(ValueError):
    """Text is not a strict SemVer 2.0.0 version."""


class InvalidRange(ValueError):
    """Text is not a valid range."""


class ResolutionError(ValueError):
    """No assignment satisfies every requirement."""
