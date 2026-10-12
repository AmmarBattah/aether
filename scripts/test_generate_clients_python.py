"""Public Python builder compatibility across generated module boundaries."""
import ast
import inspect
import sys
import types
import typing
import unittest

import generate_clients as generator
from generate_clients_python import files

sys.path.insert(0, str(generator.ROOT / "clients/python"))
from aether import protocol


class PythonBuilderLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = generator.python_library(generator.load(generator.SCHEMA_PATH))
        cls.names = [node.name for node in ast.parse(cls.source).body
                     if isinstance(node, ast.FunctionDef)]
        cls.baseline = types.ModuleType("_aether_protocol_baseline")
        sys.modules[cls.baseline.__name__] = cls.baseline
        exec(compile(cls.source, "<baseline>", "exec"), cls.baseline.__dict__)

    @classmethod
    def tearDownClass(cls):
        del sys.modules[cls.baseline.__name__]

    def test_signatures_docs_and_frames_are_preserved(self):
        for name in self.names:
            with self.subTest(builder=name):
                old = getattr(self.baseline, name)
                new = getattr(protocol, name)
                signature = inspect.signature(old)
                self.assertEqual(signature, inspect.signature(new))
                self.assertEqual(old.__doc__, new.__doc__)
                required = {key: f"value:{key}" for key, parameter in signature.parameters.items()
                            if parameter.default is inspect.Parameter.empty}
                self.assertEqual(old(**required), new(**required))
                for value in (None, False, 0, "", []):
                    args = {key: value for key in signature.parameters}
                    self.assertEqual(old(**args), new(**args))

    def test_annotations_resolve_against_public_protocol_types(self):
        for name in self.names:
            with self.subTest(builder=name):
                old = getattr(self.baseline, name)
                expected = typing.get_type_hints(old, globalns=vars(protocol))
                self.assertEqual(expected, typing.get_type_hints(getattr(protocol, name)))

    def test_builder_modules_are_bounded_deterministic_and_parseable(self):
        output = files(self.source, [generator.BANNER])
        self.assertEqual(output, files(self.source, [generator.BANNER]))
        for path, source in output.items():
            with self.subTest(path=path):
                ast.parse(source)
                self.assertLessEqual(len(source.splitlines()), 600)
        self.assertIn("_protocol_commands/import_.py", output)

    def test_types_preserve_fields_docs_and_resolved_annotations(self):
        def shape(annotation):
            if typing.is_typeddict(annotation):
                return ("record", annotation.__name__)
            origin = typing.get_origin(annotation)
            if origin is not None:
                return (origin, tuple(shape(arg) for arg in typing.get_args(annotation)))
            return annotation

        for node in ast.parse(self.source).body:
            if not isinstance(node, (ast.ClassDef, ast.Assign)):
                continue
            name = node.name if isinstance(node, ast.ClassDef) else node.targets[0].id
            with self.subTest(type=name):
                old, new = getattr(self.baseline, name), getattr(protocol, name)
                self.assertEqual(shape(old), shape(new))
                if not typing.is_typeddict(old):
                    continue
                self.assertEqual(old.__doc__, new.__doc__)
                self.assertEqual(old.__total__, new.__total__)
                self.assertEqual(old.__required_keys__, new.__required_keys__)
                self.assertEqual(old.__optional_keys__, new.__optional_keys__)
                self.assertEqual(
                    {key: shape(value) for key, value in typing.get_type_hints(old).items()},
                    {key: shape(value) for key, value in typing.get_type_hints(new).items()},
                )


if __name__ == "__main__":
    unittest.main()
