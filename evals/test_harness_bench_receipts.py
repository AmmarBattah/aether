"""Deterministic ordering tests for the independent native receipt recorder."""
import io
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import harness_bench as hb


class ReceiptOrderingTests(unittest.TestCase):
    def test_terminal_receipt_is_saved_before_the_adapter_can_observe_it(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / f"{os.getpid()}.json"
            output = [b"not-json\n", b"null\n", b'{"type":"system"}\n',
                      b'{"type":"result","origin":{"kind":"task-notification"}}\n',
                      b'{"type":"result","usage":{"input_tokens":3},"result":"SECRET"}\n']
            child = Mock(stdout=io.BytesIO(b"".join(output)))
            child.wait.return_value = 0
            child.poll.return_value = 0
            forwarded = []
            test = self

            class AdapterPipe:
                def write(self, line):
                    # Inspect at the visibility boundary, not after the proxy
                    # exits: the adapter may kill its process on this write.
                    capture = json.loads(path.read_text())
                    test.assertIs(type(capture["started_ns"]), int)
                    test.assertGreater(capture["started_ns"], 0)
                    test.assertFalse(capture["resumed"])
                    receipt = capture["receipt"]
                    if line == output[-1]:
                        test.assertEqual(receipt["usage"]["input_tokens"], 3)
                        test.assertNotIn("SECRET", json.dumps(receipt))
                    else:
                        test.assertIsNone(receipt)
                    forwarded.append(line)

                def flush(self):
                    pass

            script = hb._CLAUDE_PROXY.format(real="/fake-claude", receipts=directory, strict_mcp=False)
            with patch("subprocess.Popen", return_value=child), \
                 patch.object(sys, "argv", ["proxy", "-p"]), \
                 patch.object(sys, "stdout", SimpleNamespace(buffer=AdapterPipe())):
                with self.assertRaises(SystemExit) as exit_result:
                    exec(compile(script, "receipt-proxy", "exec"), {})
            self.assertEqual(exit_result.exception.code, 0)
            self.assertEqual(forwarded, output, "all output is forwarded verbatim in order")
            child.terminate.assert_not_called()


class StrictMcpTests(unittest.TestCase):
    def _launched(self, argv, strict_mcp):
        """The real CLI's arguments for one shim invocation, and the receipt it left."""
        with tempfile.TemporaryDirectory() as directory:
            child = Mock(stdout=io.BytesIO(b""))
            child.wait.return_value = 0
            child.poll.return_value = 0
            script = hb._CLAUDE_PROXY.format(real="/fake-claude", receipts=directory, strict_mcp=strict_mcp)
            with patch("subprocess.Popen", return_value=child) as popen, \
                 patch("os.execv", side_effect=SystemExit(0)) as execv, \
                 patch.object(sys, "argv", ["proxy", *argv]), \
                 patch.object(sys, "stdout", SimpleNamespace(buffer=io.BytesIO())), \
                 self.assertRaises(SystemExit):
                exec(compile(script, "receipt-proxy", "exec"), {})
            if execv.called:
                return execv.call_args.args[1][1:], None
            return popen.call_args.args[0][1:], json.loads((Path(directory) / f"{os.getpid()}.json").read_text())

    def test_a_strict_shim_adds_the_flag_ahead_of_aethers_own_arguments(self):
        aether = ["-p", "--disallowedTools", "Agent,Workflow", "--mcp-config", '{"mcpServers":{"aether":{}}}']
        launched, capture = self._launched(aether, strict_mcp=True)
        self.assertEqual(launched, ["--strict-mcp-config", *aether])
        self.assertIs(capture["strict_mcp_added"], True)

    def test_a_shim_that_is_not_strict_changes_no_argument(self):
        aether = ["-p", "--mcp-config", "{}"]
        launched, capture = self._launched(aether, strict_mcp=False)
        self.assertEqual(launched, aether)
        self.assertIs(capture["strict_mcp_added"], False)

    def test_the_flag_is_never_doubled_and_never_added_outside_print_mode(self):
        already = ["-p", "--strict-mcp-config", "--mcp-config", "{}"]
        launched, capture = self._launched(already, strict_mcp=True)
        self.assertEqual(launched, already)
        self.assertIs(capture["strict_mcp_added"], False)
        self.assertEqual(self._launched(["--version"], strict_mcp=True), (["--version"], None))

    def test_the_installed_shim_is_transparent_unless_asked(self):
        with tempfile.TemporaryDirectory() as directory:
            for strict_mcp, expected in ((False, "strict_mcp = False and"), (True, "strict_mcp = True and")):
                env = {"PATH": "/usr/bin"}
                arguments = () if not strict_mcp else (True,)
                hb.install_claude_receipt_proxy(env, directory, "/fake-claude", *arguments)
                self.assertIn(expected, (Path(directory) / "bin" / "claude").read_text())
                self.assertTrue(env["PATH"].startswith(str(Path(directory) / "bin")))


if __name__ == "__main__":
    unittest.main()
