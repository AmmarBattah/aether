"""Errors raised by the linestore package. See ../CONTRACT.md."""


class LineStoreError(Exception):
    """Base class for every error this package raises."""


class FramingError(LineStoreError):
    """The byte stream is not a valid sequence of frames."""


class ProtocolError(LineStoreError):
    """A frame payload is not a valid request or response message."""


class StorageError(LineStoreError):
    """The write-ahead log is corrupt, or was used incorrectly."""
