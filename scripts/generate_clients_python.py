"""Separate Python protocol modules without changing their public imports."""

import ast
import keyword

if __package__:
    from .generate_clients_python_types import type_files
else:
    from generate_clients_python_types import type_files


def files(source, banner):
    lines = source.splitlines()
    functions = [node for node in ast.parse(source).body if isinstance(node, ast.FunctionDef)]
    if not functions:
        raise ValueError("generated Python protocol has no command builders")
    groups = {}
    for function in functions:
        operation = function.name.split("_", 1)[0]
        if keyword.iskeyword(operation):
            operation += "_"
        groups.setdefault(operation, []).append(function)

    header = [f"# {line}" for line in banner]
    output, root = type_files(source, banner)
    output["_protocol_commands/__init__.py"] = "\n".join(header) + "\n"
    for operation, members in sorted(groups.items()):
        annotations = []
        for member in members:
            annotations.extend(arg.annotation for arg in member.args.kwonlyargs)
            annotations.append(member.returns)
        names = sorted({node.id for annotation in annotations if annotation is not None
                        for node in ast.walk(annotation) if isinstance(node, ast.Name)}
                       - {"str", "int", "float", "bool", "dict", "list"})
        # The facade defines every protocol type before importing builders.
        # Real imports keep get_type_hints working in the builder's module.
        imports = (["from ..protocol import ("] + [f"    {name}," for name in names]
                   + [")"]) if names else []
        body = []
        for member in members:
            body.extend(lines[member.lineno - 1:member.end_lineno] + [""])
        rendered = "\n".join(header + ["from __future__ import annotations", ""]
                             + imports + [""] + body).rstrip() + "\n"
        path = f"_protocol_commands/{operation}.py"
        if len(rendered.splitlines()) > 600:
            raise ValueError(f"generated builder module exceeds 600 lines: {path}")
        output[path] = rendered
        root.extend([f"from ._protocol_commands.{operation} import ("]
                    + [f"    {member.name}," for member in members] + [")", ""])
    output["protocol.py"] = "\n".join(root).rstrip() + "\n"
    for path, content in output.items():
        if len(content.splitlines()) > 600:
            raise ValueError(f"generated Python module exceeds 600 lines: {path}")
    return output
