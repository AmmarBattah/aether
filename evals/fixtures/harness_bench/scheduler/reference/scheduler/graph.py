"""Dependency graph validation and cycle detection.

See ../../starter/CONTRACT.md. Standard three-color DFS: WHITE (unvisited),
GRAY (on the current recursion stack), BLACK (fully explored and known
acyclic). A back edge to a GRAY node is a real cycle; revisiting a BLACK node
through a second path (e.g. a diamond) is fine and must not raise. Every node
is tried as a DFS root, not just nodes nothing depends on, so a
self-contained cycle with no node outside it is still found.
"""
from .errors import SchedulerError

_WHITE, _GRAY, _BLACK = 0, 1, 2


class DependencyGraph:
    def __init__(self):
        self._deps = {}  # node_id -> set of node_ids it depends on

    def add_node(self, node_id):
        if node_id in self._deps:
            raise SchedulerError(f"duplicate task id: {node_id}")
        self._deps[node_id] = set()

    def add_edge(self, node_id, depends_on):
        self._deps.setdefault(node_id, set())
        self._deps.setdefault(depends_on, set())
        self._deps[node_id].add(depends_on)

    def nodes(self):
        return sorted(self._deps)

    def dependencies_of(self, node_id):
        return sorted(self._deps.get(node_id, ()))

    def check_acyclic(self):
        color = {node: _WHITE for node in self._deps}
        for node in self.nodes():
            if color[node] == _WHITE:
                self._visit(node, [], color)

    def _visit(self, node, path, color):
        color[node] = _GRAY
        for dep in self.dependencies_of(node):
            if color[dep] == _GRAY:
                raise SchedulerError(f"cycle detected: {' -> '.join(path + [node, dep])}")
            if color[dep] == _WHITE:
                self._visit(dep, path + [node], color)
        color[node] = _BLACK
