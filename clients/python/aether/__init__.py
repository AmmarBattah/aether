"""The Aether daemon protocol as a Python library.

`protocol` is generated from the Rust types by `scripts/generate_clients.py`;
`client` is the hand-written connection on top of it.
"""

from . import protocol
from .client import (
    AetherClient,
    AetherError,
    Subscription,
    SubscriptionLaggedError,
    default_socket_path,
)

__all__ = [
    "AetherClient", "AetherError", "Subscription", "SubscriptionLaggedError",
    "default_socket_path", "protocol",
]
