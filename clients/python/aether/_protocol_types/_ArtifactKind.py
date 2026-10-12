# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._ArtifactKindCommandOutput import ArtifactKindCommandOutput
from ._ArtifactKindCommit import ArtifactKindCommit
from ._ArtifactKindDiff import ArtifactKindDiff
from ._ArtifactKindFile import ArtifactKindFile
from ._ArtifactKindOther import ArtifactKindOther
from ._ArtifactKindResult import ArtifactKindResult


ArtifactKind = (
    ArtifactKindFile |
    ArtifactKindCommandOutput |
    ArtifactKindDiff |
    ArtifactKindCommit |
    ArtifactKindResult |
    ArtifactKindOther
)
