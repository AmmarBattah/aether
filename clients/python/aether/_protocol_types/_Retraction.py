# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Retraction(TypedDict, total=False):
    """
    Why a repository-knowledge sweep curated a record out: the committed tree
    named here no longer states it, so the `file@sha` label it carried is no
    longer a promise anyone can check.

    Distinct from a person's `forget` on purpose. A person's curation outranks
    the file however many times the note comes back; a retraction is undone by
    the note reappearing in the tree.
    """
    at: str
    commit: str
    file: str
