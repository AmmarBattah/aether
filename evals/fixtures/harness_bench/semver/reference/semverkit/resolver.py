from .errors import InvalidVersion, ResolutionError
from .ranges import Range
from .version import parse


def _name(value):
    if not isinstance(value, str) or not value:
        raise ValueError(f"package names must be nonempty strings, got {value!r}")
    return value


def _ranges(dependencies):
    if not isinstance(dependencies, dict):
        raise ValueError("dependencies must be a dict")
    return {_name(name): Range(text) for name, text in dependencies.items()}


def _index(registry):
    if not isinstance(registry, dict):
        raise ValueError("registry must be a dict")
    index = {}
    for name, versions in registry.items():
        _name(name)
        if not isinstance(versions, dict):
            raise ValueError(f"versions of {name!r} must be a dict")
        entries = sorted(((parse(text), text, _ranges(deps)) for text, deps in versions.items()),
                         key=lambda entry: entry[0], reverse=True)
        for higher, lower in zip(entries, entries[1:]):
            if higher[0] == lower[0]:
                raise InvalidVersion(f"{name} has equal-precedence versions {higher[1]!r} and {lower[1]!r}")
        index[name] = entries
    return index


def resolve(registry, requirements):
    index = _index(registry)
    root = _ranges(requirements)
    decided = {}
    if not _search(index, root, decided):
        raise ResolutionError("no version assignment satisfies every requirement")
    return {name: text for name, (_, text, _) in decided.items()}


def _search(index, root, decided):
    required = set(root).union(*(deps for _, _, deps in decided.values()))
    pending = sorted(required - decided.keys())
    if not pending:
        return True
    name = pending[0]
    constraints = [root[name]] if name in root else []
    constraints += [deps[name] for _, _, deps in decided.values() if name in deps]
    for entry in index.get(name, []):
        version, _, deps = entry
        if not all(constraint.test(version) for constraint in constraints):
            continue
        decided[name] = entry
        consistent = all(rng.test(decided[dep][0]) for dep, rng in deps.items() if dep in decided)
        if consistent and _search(index, root, decided):
            return True
        del decided[name]
    return False
