"""Dependency graph validation and cycle detection. See ../CONTRACT.md."""
from .errors import SchedulerError


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
        visited = set()
        # BUG: only starts a DFS from "root" nodes (nothing depends on
        # them). A cycle with no node outside it (e.g. a <-> b with no
        # third node depending on either) then has no root to start from
        # and is never visited, so it is silently missed. Separately, the
        # inner visit() below treats *any* revisit as a cycle instead of
        # tracking which nodes are still on the current path, so a diamond
        # (a shared dependency reached through two branches) is falsely
        # reported as a cycle even though the graph is acyclic.
        for root in self._roots():
            self._visit(root, [], visited)

    def _roots(self):
        depended_on = set()
        for deps in self._deps.values():
            depended_on.update(deps)
        return [n for n in self.nodes() if n not in depended_on]

    def _visit(self, node, path, visited):
        if node in visited:
            raise SchedulerError(f"cycle detected: {' -> '.join(path + [node])}")
        visited.add(node)
        for dep in self.dependencies_of(node):
            self._visit(dep, path + [node], visited)
