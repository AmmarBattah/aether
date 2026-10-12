class Node:
    """AST node; `token` locates errors and `fields` hold the children."""

    __slots__ = ("kind", "token", "fields")

    def __init__(self, kind, token, **fields):
        self.kind, self.token, self.fields = kind, token, fields

    def __getattr__(self, name):
        try:
            return self.fields[name]
        except KeyError:
            raise AttributeError(name) from None
