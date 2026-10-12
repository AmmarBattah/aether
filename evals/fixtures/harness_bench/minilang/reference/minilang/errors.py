class MiniError(Exception):
    def __init__(self, message, line, column):
        super().__init__(f"{line}:{column}: {message}")
        self.message, self.line, self.column = message, line, column
        self.output = []


class MiniSyntaxError(MiniError):
    pass


class MiniNameError(MiniError):
    pass


class MiniTypeError(MiniError):
    pass


class MiniRuntimeError(MiniError):
    pass


def at(error_type, token, message):
    return error_type(message, token.line, token.column)
