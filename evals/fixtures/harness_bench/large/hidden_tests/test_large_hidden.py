"""Held-out oracle for the `large` case. Not copied into solver workspaces;
grade() imports the solver's `linestore` package from their workspace and runs
these tests against it. Independent of whatever tests the solver writes.

The property-style tests drive seeded random operation sequences through the
server and compare against a plain-dict oracle built straight from the
contract, then check the cross-cutting invariants (journal-before-ack, replay
idempotency, compaction preserving observable state) on the same sequences.
"""
import random
import tempfile
import unittest
from pathlib import Path

from linestore import (FrameDecoder, FramingError, ProtocolError, Server,
                       StorageError, Store, WriteAheadLog, compaction_records,
                       encode_frame, encode_request, encode_response,
                       parse_request, parse_response, replay)


class _FailingLog:
    """Wraps a real log; append() raises exactly once, on demand, to exercise
    the journal-before-acknowledge contract without touching disk."""

    def __init__(self, real):
        self._real = real
        self.fail_next = False

    def append(self, seq, payload):
        if self.fail_next:
            self.fail_next = False
            raise OSError("simulated disk failure")
        self._real.append(seq, payload)

    def records(self):
        return self._real.records()

    def last_seq(self):
        return self._real.last_seq()

    def replace_with(self, records):
        return self._real.replace_with(records)

    def close(self):
        self._real.close()


class _CaseWithLog(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.log_path = self.root / "linestore.wal"

    def new_server(self):
        server = Server(self.log_path)
        self.addCleanup(server.close)
        return server

    def send(self, server, op, *args):
        return server.handle_request(encode_request(op, *args))

    def read_records(self, path=None):
        """Read a log through a fresh, promptly closed handle."""
        log = WriteAheadLog(self.log_path if path is None else path)
        try:
            return log.records()
        finally:
            log.close()


class FramingTests(_CaseWithLog):
    def test_header_counts_utf8_bytes_not_characters(self):
        frame = encode_frame("SET k café-über")
        self.assertEqual(frame.split(b"|", 1)[0], b"17")  # 15 characters, 17 bytes
        decoder = FrameDecoder()
        decoder.feed(frame)
        self.assertEqual(decoder.next_frame(), "SET k café-über")

    def test_frame_split_across_many_chunks_is_reassembled(self):
        frame = encode_frame("PING")
        decoder = FrameDecoder()
        for index in range(len(frame)):
            decoder.feed(frame[index:index + 1])
            expected_done = index == len(frame) - 1
            result = decoder.next_frame()
            self.assertEqual(result is not None, expected_done, f"byte {index}")
        self.assertEqual(decoder.pending_bytes(), 0)

    def test_one_chunk_with_several_frames_yields_them_in_order(self):
        payloads = ["PING", "GET a", "SET b c"]
        decoder = FrameDecoder()
        decoder.feed(b"".join(encode_frame(p) for p in payloads))
        decoded = [decoder.next_frame() for _ in payloads]
        self.assertEqual(decoded, payloads)
        self.assertIsNone(decoder.next_frame())

    def test_malformed_header_poisons_the_decoder_and_clears_the_buffer(self):
        decoder = FrameDecoder()
        decoder.feed(b"not-a-frame\n" + encode_frame("PING"))
        with self.assertRaises(FramingError):
            decoder.next_frame()
        self.assertTrue(decoder.is_poisoned())
        self.assertEqual(decoder.pending_bytes(), 0)

    def test_a_poisoned_decoder_refuses_every_later_call(self):
        decoder = FrameDecoder()
        decoder.feed(b"4|PING?")  # wrong terminator
        with self.assertRaises(FramingError):
            decoder.next_frame()
        with self.assertRaises(FramingError):
            decoder.feed(encode_frame("PING"))
        with self.assertRaises(FramingError):
            decoder.next_frame()

    def test_declared_length_beyond_the_limit_is_rejected(self):
        decoder = FrameDecoder()
        decoder.feed(b"999999|x\n")
        with self.assertRaises(FramingError):
            decoder.next_frame()
        self.assertTrue(decoder.is_poisoned())

    def test_encoder_rejects_a_newline_payload(self):
        with self.assertRaises(FramingError):
            encode_frame("SET a b\nGET a")

    def test_invalid_utf8_payload_poisons_the_decoder(self):
        decoder = FrameDecoder()
        decoder.feed(b"2|" + b"\xff\xfe" + b"\n")
        with self.assertRaises(FramingError):
            decoder.next_frame()
        self.assertTrue(decoder.is_poisoned())


class ProtocolTests(_CaseWithLog):
    def test_value_containing_spaces_round_trips(self):
        payload = encode_request("SET", "k", "a b  c")
        self.assertEqual(parse_request(payload), ("SET", ("k", "a b  c")))

    def test_empty_value_survives_the_round_trip(self):
        payload = encode_request("SET", "k", "")
        request = parse_request(payload)
        self.assertEqual(request.op, "SET")
        self.assertEqual(request.args, ("k", ""))

    def test_unknown_op_and_bad_arity_raise_protocol_error(self):
        with self.assertRaises(ProtocolError):
            parse_request("NOPE a")
        with self.assertRaises(ProtocolError):
            parse_request("GET a b")

    def test_invalid_keys_and_deltas_raise_protocol_error(self):
        with self.assertRaises(ProtocolError):
            parse_request(encode_request("GET", ""))
        with self.assertRaises(ProtocolError):
            parse_request(encode_request("SET", "x" * 65, "v"))
        with self.assertRaises(ProtocolError):
            parse_request(encode_request("INCR", "k", "not-an-int"))

    def test_response_round_trip_and_validation(self):
        self.assertEqual(parse_response(encode_response("KEYS", "a", "b")).args, ("a", "b"))
        self.assertEqual(parse_response(encode_response("OK", 7)).args, ("7",))
        with self.assertRaises(ProtocolError):
            parse_response("ERR nosuchcode detail")


class WriteAheadLogTests(_CaseWithLog):
    def test_append_is_visible_to_a_separate_reader_before_close(self):
        log = WriteAheadLog(self.log_path)
        self.addCleanup(log.close)
        log.append(1, encode_request("SET", "a", "1"))
        reader = WriteAheadLog(self.log_path)
        self.addCleanup(reader.close)
        self.assertEqual(reader.records(), [(1, encode_request("SET", "a", "1"))])

    def test_torn_final_line_is_dropped_silently(self):
        log = WriteAheadLog(self.log_path)
        log.append(1, encode_request("SET", "a", "1"))
        log.close()
        with open(self.log_path, "a", encoding="utf-8") as handle:
            handle.write("2 deadbeef SET b")  # no newline: torn write
        self.assertEqual(len(self.read_records()), 1)

    def test_checksum_mismatch_on_a_complete_line_raises(self):
        log = WriteAheadLog(self.log_path)
        log.append(1, encode_request("SET", "a", "1"))
        log.close()
        with open(self.log_path, "a", encoding="utf-8") as handle:
            handle.write("2 00000000 " + encode_request("SET", "b", "2") + "\n")
        with self.assertRaises(StorageError):
            self.read_records()

    def test_non_increasing_sequence_numbers_raise(self):
        log = WriteAheadLog(self.log_path)
        log.append(2, encode_request("SET", "a", "1"))
        log.append(1, encode_request("SET", "b", "2"))
        log.close()
        with self.assertRaises(StorageError):
            self.read_records()

    def test_replace_with_swaps_the_log_and_leaves_no_temp_file(self):
        log = WriteAheadLog(self.log_path)
        self.addCleanup(log.close)
        for seq in range(1, 6):
            log.append(seq, encode_request("SET", "a", str(seq)))
        log.replace_with([(5, encode_request("SET", "a", "5"))])
        self.assertEqual(log.records(), [(5, encode_request("SET", "a", "5"))])
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), ["linestore.wal"])

    def test_close_is_idempotent(self):
        log = WriteAheadLog(self.log_path)
        log.append(1, encode_request("SET", "a", "1"))
        log.close()
        log.close()


class StoreTests(_CaseWithLog):
    def test_apply_is_idempotent_by_sequence_number(self):
        store = Store()
        record = encode_request("INCR", "n", "5")
        store.apply(1, encode_request("SET", "n", "0"))
        self.assertTrue(store.apply(2, record))
        self.assertFalse(store.apply(2, record))
        self.assertFalse(store.apply(1, record))
        self.assertEqual(store.get("n"), "5")

    def test_replaying_the_same_records_twice_changes_nothing(self):
        records = [(1, encode_request("SET", "n", "1")),
                   (2, encode_request("INCR", "n", "4")),
                   (3, encode_request("SET", "s", "x y"))]
        store = replay(records)
        first = store.snapshot()
        for seq, payload in records:
            store.apply(seq, payload)
        self.assertEqual(store.snapshot(), first)
        self.assertEqual(first, {"n": "5", "s": "x y"})

    def test_records_whose_precondition_is_gone_are_no_ops(self):
        store = Store()
        store.apply(1, encode_request("DEL", "ghost"))
        store.apply(2, encode_request("INCR", "ghost", "3"))
        store.apply(3, encode_request("SET", "t", "text"))
        store.apply(4, encode_request("INCR", "t", "1"))
        self.assertEqual(store.snapshot(), {"t": "text"})
        self.assertEqual(store.applied_seq(), 4)

    def test_a_read_op_in_the_log_is_corruption(self):
        with self.assertRaises(StorageError):
            Store().apply(1, encode_request("GET", "a"))


class ServerRequestTests(_CaseWithLog):
    def test_reads_are_answered_and_never_journaled(self):
        server = self.new_server()
        self.send(server, "SET", "a", "1")
        self.assertEqual(self.send(server, "PING"), encode_response("PONG"))
        self.assertEqual(self.send(server, "GET", "a"), encode_response("VAL", "1"))
        self.assertEqual(self.send(server, "GET", "b"), encode_response("ERR", "notfound", "b"))
        self.assertEqual(self.send(server, "LIST", ""), encode_response("KEYS", "a"))
        self.assertEqual(server.next_seq, 2)
        self.assertEqual(len(self.read_records()), 1)

    def test_failed_preconditions_are_not_journaled(self):
        server = self.new_server()
        self.send(server, "SET", "text", "abc")
        self.assertEqual(self.send(server, "DEL", "ghost"),
                         encode_response("ERR", "notfound", "ghost"))
        self.assertEqual(self.send(server, "INCR", "ghost", "1"),
                         encode_response("ERR", "notfound", "ghost"))
        self.assertEqual(self.send(server, "INCR", "text", "1"),
                         encode_response("ERR", "badvalue", "text"))
        self.assertEqual(server.next_seq, 2)
        self.assertEqual(server.snapshot(), {"text": "abc"})

    def test_a_protocol_error_becomes_an_error_response_not_an_exception(self):
        server = self.new_server()
        response = parse_response(server.handle_request("BOGUS x"))
        self.assertEqual(response.kind, "ERR")
        self.assertEqual(response.args[0], "protocol")
        self.assertEqual(server.next_seq, 1)

    def test_a_failed_journal_append_blocks_the_acknowledgement(self):
        real = WriteAheadLog(self.log_path)
        failing = _FailingLog(real)
        server = Server(self.log_path, wal=failing)
        self.addCleanup(server.close)
        self.send(server, "SET", "a", "1")

        failing.fail_next = True
        with self.assertRaises(OSError):
            self.send(server, "SET", "b", "2")
        self.assertEqual(server.snapshot(), {"a": "1"})
        self.assertEqual(server.applied_seq(), 1)

        self.assertEqual(self.send(server, "SET", "b", "2"), encode_response("OK", "2"))
        self.assertEqual(real.records()[-1], (2, encode_request("SET", "b", "2")))

    def test_the_journal_holds_the_canonical_encoding_of_each_mutation(self):
        server = self.new_server()
        server.handle_request("SET k a%2Db")  # non-canonical escape of "a-b"
        server.close()
        self.assertEqual(self.read_records(), [(1, encode_request("SET", "k", "a-b"))])


class ServerFramingTests(_CaseWithLog):
    def test_valid_frames_before_a_malformed_one_are_still_served(self):
        server = self.new_server()
        chunk = encode_frame(encode_request("SET", "a", "1")) + b"bad-frame\n"
        out = server.handle_bytes(chunk)
        decoder = FrameDecoder()
        decoder.feed(out)
        self.assertEqual(parse_response(decoder.next_frame()).kind, "OK")
        framing_error = parse_response(decoder.next_frame())
        self.assertEqual(framing_error.args[0], "framing")
        self.assertEqual(server.snapshot(), {"a": "1"})

    def test_a_poisoned_connection_keeps_answering_framing_errors(self):
        server = self.new_server()
        server.handle_bytes(b"bad-frame\n")
        out = server.handle_bytes(encode_frame(encode_request("SET", "a", "1")))
        decoder = FrameDecoder()
        decoder.feed(out)
        self.assertEqual(parse_response(decoder.next_frame()).args[0], "framing")
        self.assertIsNone(decoder.next_frame())
        self.assertEqual(server.snapshot(), {})
        self.assertEqual(self.read_records(), [])

    def test_requests_split_across_chunk_boundaries_are_served_once(self):
        server = self.new_server()
        stream = b"".join(encode_frame(encode_request("SET", f"k{i}", str(i))) for i in range(5))
        out = b""
        for index in range(0, len(stream), 7):
            out += server.handle_bytes(stream[index:index + 7])
        decoder = FrameDecoder()
        decoder.feed(out)
        kinds = []
        payload = decoder.next_frame()
        while payload is not None:
            kinds.append(parse_response(payload).kind)
            payload = decoder.next_frame()
        self.assertEqual(kinds, ["OK"] * 5)
        self.assertEqual(len(server.snapshot()), 5)


class RecoveryAndCompactionTests(_CaseWithLog):
    def _seed_server(self):
        server = self.new_server()
        self.send(server, "SET", "a", "1")
        self.send(server, "SET", "b", "2")
        self.send(server, "INCR", "a", "4")
        self.send(server, "SET", "gone", "x")
        self.send(server, "DEL", "gone")
        return server

    def test_recover_rebuilds_state_without_growing_the_log(self):
        server = self._seed_server()
        server.close()
        before = self.log_path.read_text()

        recovered = Server.recover(self.log_path)
        self.addCleanup(recovered.close)
        self.assertEqual(recovered.snapshot(), {"a": "5", "b": "2"})
        self.assertEqual(recovered.next_seq, 6)
        self.assertEqual(self.log_path.read_text(), before)

    def test_recover_after_a_torn_tail_continues_the_sequence(self):
        server = self._seed_server()
        server.close()
        with open(self.log_path, "a", encoding="utf-8") as handle:
            handle.write("6 00000000 SET c")  # torn write
        recovered = Server.recover(self.log_path)
        self.addCleanup(recovered.close)
        self.assertEqual(recovered.next_seq, 6)
        self.assertEqual(recovered.handle_request(encode_request("SET", "c", "3")),
                         encode_response("OK", "6"))

    def test_compaction_preserves_state_and_the_highest_sequence(self):
        server = self._seed_server()
        before = server.snapshot()
        count = server.compact()
        self.assertEqual(server.snapshot(), before)
        self.assertLess(count, 5)
        server.close()

        recovered = Server.recover(self.log_path)
        self.addCleanup(recovered.close)
        self.assertEqual(recovered.snapshot(), before)
        self.assertEqual(recovered.next_seq, 6)

    def test_compaction_is_stable_when_repeated(self):
        server = self._seed_server()
        first = server.compact()
        records = self.read_records()
        second = server.compact()
        self.assertEqual(first, second)
        self.assertEqual(self.read_records(), records)

    def test_mutations_after_compaction_keep_increasing_sequences(self):
        server = self._seed_server()
        server.compact()
        self.assertEqual(self.send(server, "SET", "d", "4"), encode_response("OK", "6"))
        server.close()
        recovered = Server.recover(self.log_path)
        self.addCleanup(recovered.close)
        self.assertEqual(recovered.snapshot(), {"a": "5", "b": "2", "d": "4"})

    def test_compaction_records_keep_a_trailing_delete_for_the_highest_seq(self):
        records = [(1, encode_request("SET", "a", "1")),
                   (2, encode_request("SET", "b", "2")),
                   (3, encode_request("DEL", "b"))]
        compacted = compaction_records(records)
        self.assertEqual([seq for seq, _ in compacted], [1, 3])
        self.assertEqual(replay(compacted).snapshot(), {"a": "1"})

    def test_compaction_of_an_empty_log_is_empty(self):
        self.assertEqual(compaction_records([]), [])


class PropertyTests(_CaseWithLog):
    """Seeded random operation sequences checked against a dict oracle.

    Plain loops rather than subTest(): the grader counts one failure per test
    method, so a failing seed must stop its test instead of reporting many.
    """

    def test_random_sequences_match_an_independent_oracle(self):
        for seed in range(8):
            self._run_sequence(seed, compact_midway=False)

    def test_random_sequences_survive_compaction_and_recovery(self):
        for seed in range(8):
            self._run_sequence(seed, compact_midway=True)

    def _run_sequence(self, seed, compact_midway):
        path = self.root / f"seeded-{seed}-{int(compact_midway)}.wal"
        operations = _random_operations(seed)
        server = Server(path)
        expected = {}
        for index, (op, args) in enumerate(operations):
            _oracle_apply(expected, op, args)
            server.handle_request(encode_request(op, *args))
            if compact_midway and index == len(operations) // 2:
                server.compact()
                self.assertEqual(server.snapshot(), expected, f"seed={seed} after compact")
        self.assertEqual(server.snapshot(), expected, f"seed={seed}")
        highest = server.next_seq
        server.close()

        recovered = Server.recover(path)
        self.assertEqual(recovered.snapshot(), expected, f"seed={seed} recovered")
        self.assertEqual(recovered.next_seq, highest, f"seed={seed} next_seq")
        recovered.compact()
        recovered.close()
        self.assertEqual(replay(self.read_records(path)).snapshot(), expected)


def _oracle_apply(state, op, args):
    """The contract's mutation rules over a plain dict, written independently
    of the package under test (reads and rejected mutations change nothing)."""
    if op == "SET":
        state[args[0]] = args[1]
    elif op == "DEL":
        state.pop(args[0], None)
    elif op == "INCR":
        current = state.get(args[0])
        if current is not None and _looks_like_int(current):
            state[args[0]] = str(int(current) + int(args[1]))


def _looks_like_int(text):
    body = text[1:] if text.startswith("-") else text
    return bool(body) and body.isdigit()


def _random_operations(seed, count=40):
    """A deterministic operation sequence mixing reads, mutations, rejected
    mutations, spaced values and non-ASCII keys."""
    rng = random.Random(seed)
    keys = ["a", "b", "kéy", "long-key", "n"]
    operations = []
    for _ in range(count):
        key = rng.choice(keys)
        choice = rng.random()
        if choice < 0.35:
            operations.append(("SET", (key, rng.choice(["1", "2", "a b", "", "x"]))))
        elif choice < 0.55:
            operations.append(("INCR", (key, str(rng.randint(-5, 5)))))
        elif choice < 0.7:
            operations.append(("DEL", (key,)))
        elif choice < 0.85:
            operations.append(("GET", (key,)))
        else:
            operations.append(("LIST", ("",)))
    return operations


if __name__ == "__main__":
    unittest.main()
