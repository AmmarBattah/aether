"""Visible smoke tests. These only cover the easy happy path; the graded
oracle (outside this workspace) checks the harder contract requirements,
especially the four cross-cutting invariants listed in CONTRACT.md."""
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from linestore import (FrameDecoder, Server, WriteAheadLog, encode_frame,
                       encode_request, encode_response, parse_response)


class LinestoreBasicTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.path = Path(self._tmp.name) / "linestore.wal"
        self.server = Server(self.path)

    def tearDown(self):
        self.server.close()
        self._tmp.cleanup()

    def send(self, op, *args):
        return self.server.handle_request(encode_request(op, *args))

    def test_set_then_get(self):
        self.assertEqual(self.send("SET", "a", "1"), encode_response("OK", "1"))
        self.assertEqual(self.send("GET", "a"), encode_response("VAL", "1"))

    def test_delete_removes_the_key(self):
        self.send("SET", "a", "1")
        self.assertEqual(self.send("DEL", "a"), encode_response("OK", "2"))
        self.assertEqual(self.send("GET", "a"), encode_response("ERR", "notfound", "a"))

    def test_incr_adds_to_the_stored_number(self):
        self.send("SET", "n", "5")
        self.send("INCR", "n", "3")
        self.assertEqual(self.send("GET", "n"), encode_response("VAL", "8"))

    def test_list_returns_sorted_keys_for_a_prefix(self):
        self.send("SET", "kb", "1")
        self.send("SET", "ka", "2")
        self.send("SET", "zz", "3")
        self.assertEqual(self.send("LIST", "k"), encode_response("KEYS", "ka", "kb"))

    def test_unknown_op_answers_a_protocol_error(self):
        response = parse_response(self.server.handle_request("BOGUS x"))
        self.assertEqual(response.kind, "ERR")
        self.assertEqual(response.args[0], "protocol")

    def test_handle_bytes_answers_one_framed_request(self):
        out = self.server.handle_bytes(encode_frame(encode_request("SET", "a", "1")))
        decoder = FrameDecoder()
        decoder.feed(out)
        self.assertEqual(parse_response(decoder.next_frame()).kind, "OK")

    def test_recover_rebuilds_the_store_from_the_log(self):
        self.send("SET", "a", "1")
        self.send("SET", "b", "2")
        self.server.close()

        recovered = Server.recover(self.path)
        self.assertEqual(recovered.snapshot(), {"a": "1", "b": "2"})
        self.assertEqual(recovered.next_seq, 3)
        recovered.close()

    def test_log_records_survive_a_close_and_reopen(self):
        log = WriteAheadLog(self.path)
        log.append(1, encode_request("SET", "a", "1"))
        log.close()

        reader = WriteAheadLog(self.path)
        self.assertEqual(reader.records(), [(1, encode_request("SET", "a", "1"))])
        reader.close()


if __name__ == "__main__":
    unittest.main()
