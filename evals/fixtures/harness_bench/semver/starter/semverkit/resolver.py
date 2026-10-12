from .errors import ResolutionError
from .ranges import Range
from .version import parse


def resolve(registry, requirements):
    decided = {}
    queue = list(requirements.items())
    while queue:
        name, range_text = queue.pop(0)
        selector = Range(range_text)
        if name in decided:
            if not selector.test(decided[name][0]):
                raise ResolutionError(f"conflicting requirements on {name}")
            continue
        candidates = sorted(((parse(text), text) for text in registry.get(name, {})), reverse=True)
        for version, text in candidates:
            if selector.test(version):
                decided[name] = (version, text)
                queue.extend(registry[name][text].items())
                break
        else:
            raise ResolutionError(f"no version of {name} satisfies {range_text}")
    return {name: text for name, (_, text) in decided.items()}
