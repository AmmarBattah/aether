"""Deterministic, hard, multi-module Python benchmark fixtures for comparing
native Claude/Codex against Aether single-agent and coordinator runs.

Eleven cases, each stdlib-only, buggy, with an explicit public contract in its
starter workspace and a held-out oracle grader:

    ledger    - transactional/idempotent ledger with journal persistence,
                replay and failure semantics.
    scheduler - dependency scheduler with cycle detection, bounded retries,
                deadlines and deterministic ordering.
    large     - five-module request/response server: framing, protocol,
                write-ahead log, replay and compaction (a deliberately large
                surface, graded partly by randomised property tests).
    recovery  - crash-resumable checkpointed job runner, graded by injecting
                a fault at every step boundary and demanding exactly-once
                side effects and a self-repairing checkpoint.
    transactions - serializable snapshot transactions, savepoints, phantom/ABA
                conflicts, idempotent commits and strict framed-journal replay.
    leases    - resource-set fairness, fenced ownership, atomic journaling,
                expiry, renewal, replay and callback ownership.
    concurrent - concurrent serializable commits, reader progress during append,
                callback reentrancy, duplicate IDs and recovery.
    concurrent_v2 - a frozen concurrent contract with additional adversarial
                histories and recovery checks.
    eventlog  - durable event publication, fenced consumer-group ownership,
                ordered polling, atomic acknowledgments and strict replay.
    matchbook - limit order book: price-time priority, order types, self-trade
                prevention, iceberg orders, amend/cancel, expiry and a
                replayable journal (119 held-out checks).
    gridcalc  - spreadsheet engine: formula language, error and coercion
                rules, cycles, eager recalculation, structural edits that
                rewrite formulas, and copy (154 held-out checks).

Public API:
    cases() -> list[{"id": str, "objective": str}]
    prepare(case_id: str, workspace: Path) -> None
        Copies the case's starter code (and only the starter code - never
        the reference implementation or hidden tests) into `workspace`.
    grade(case_id: str, workspace: Path) -> dict
        Runs the held-out oracle tests against `workspace`'s package in a
        separate, time-bounded subprocess (so a hanging, os._exit()-ing, or
        globally-mutating candidate can never hang or corrupt this process)
        and returns {"passed": int, "total": int, "errors": [str, ...]}.
    grade_with_tests(case_id: str, workspace: Path) -> (dict, dict)
        The same counted grade, and beside it a test-by-test grade that is
        never the counted one: when the one-process run timed out, every
        hidden test is run again in a process of its own.
    grade_counted(case_id: str, workspace: Path) -> (dict, bool)
    grade_by_test(case_id, workspace, counted, timed_out) -> dict
        The two halves of grade_with_tests(), for a caller that must keep
        the counted grade when the test-by-test half fails.

See evals/fixtures/harness_bench/<case_id>/starter/CONTRACT.md for the full
contract a solution must satisfy, and evals/test_harness_bench_cases.py for
the proof that the starter fails and the (never-copied) reference passes.

This module also serves as its own subprocess grading entry point: it is
invoked as `python3 harness_bench_cases.py --run-hidden <case_id> <workspace>
<hidden_dir>` by grade() below. That subprocess imports the candidate
package fresh and prints one line, `HARNESS_BENCH_REPORT:<json>`, which is
the only thing grade() trusts from its stdout. The test-by-test grade uses
the same entry point with `--list-hidden` and with one test id appended.
"""
import importlib
import json
import shutil
import subprocess
import sys
import time
import unittest
from pathlib import Path

_FIXTURES_ROOT = Path(__file__).parent / "fixtures" / "harness_bench"
_REPORT_MARKER = "HARNESS_BENCH_REPORT:"
_DEFAULT_TIMEOUT_SECONDS = 60
# The test-by-test grade: each hidden test's own limit, and the limit on all of them together.
_PER_TEST_SECONDS = 5
_BY_TEST_TOTAL_SECONDS = 120
_TIMED_OUT = "timed out"
BY_TEST_FROM_COUNTED = "the counted one-process run"
BY_TEST_FROM_PROCESSES = "one hidden test per process"

_CASES = {
    "ledger": {
        "package": "ledger",
        "hidden_test_count": 22,
        "objective": (
            "This workspace contains a Python package `ledger/` implementing "
            "a transactional, idempotent, journaled account ledger, plus "
            "visible tests in tests/. Read CONTRACT.md: it is the complete "
            "public API and behavior the package must satisfy, including "
            "idempotency, transfer atomicity, insufficient-funds handling, "
            "journal durability and replay, append-failure rollback, "
            "malformed/truncated journal recovery, defensive-copy return "
            "values, and same-account transfers. The starter has real bugs "
            "against that contract. Fix ledger/ so it fully satisfies "
            "CONTRACT.md. Do not change the public API names/signatures in "
            "CONTRACT.md. You may add tests, but the visible tests in "
            "tests/ must still pass and must not be weakened to hide a bug."
        ),
    },
    "scheduler": {
        "package": "scheduler",
        "hidden_test_count": 22,
        "objective": (
            "This workspace contains a Python package `scheduler/` "
            "implementing a dependency-aware task scheduler with cycle "
            "detection, retries, deadlines and deterministic ordering, plus "
            "visible tests in tests/. Read CONTRACT.md: it is the complete "
            "public API and behavior the package must satisfy, including "
            "cycle detection (self-loops and disconnected-component cycles "
            "too), ready-task ordering, retry backoff, deadline "
            "re-checking on every retry, fan-in skip propagation, and "
            "input validation. The starter has real bugs against that "
            "contract. Fix scheduler/ so it fully satisfies CONTRACT.md. Do "
            "not change the public API names/signatures in CONTRACT.md. "
            "You may add tests, but the visible tests in tests/ must still "
            "pass and must not be weakened to hide a bug."
        ),
    },
    "large": {
        "package": "linestore",
        "hidden_test_count": 40,
        "objective": (
            "This workspace contains a Python package `linestore/` "
            "implementing a small line-oriented key/value server - byte "
            "framing, a request/response protocol, a checksummed write-ahead "
            "log, replay and log compaction - plus visible tests in tests/. "
            "Read CONTRACT.md: it is the complete public API and behavior "
            "the package must satisfy, including length-prefixed framing of "
            "non-ASCII payloads, rejecting malformed frames without "
            "corrupting server state, escaping and empty arguments, "
            "journaling every mutation before acknowledging it, dropping a "
            "torn or checksum-invalid log tail, idempotent replay by "
            "sequence number, and compaction that preserves both the "
            "observable state and the sequence numbering. The starter has "
            "real bugs against that contract. Fix linestore/ so it fully "
            "satisfies CONTRACT.md. Do not change the public API "
            "names/signatures in CONTRACT.md. You may add tests, but the "
            "visible tests in tests/ must still pass and must not be "
            "weakened to hide a bug."
        ),
    },
    "recovery": {
        "package": "jobrunner",
        "hidden_test_count": 41,
        "objective": (
            "This workspace contains a Python package `jobrunner/` "
            "implementing a checkpointed multi-step job that can be "
            "interrupted at any step boundary and resumed by a later "
            "process, plus visible tests in tests/. Read CONTRACT.md: it is "
            "the complete public API and behavior the package must satisfy, "
            "including firing every documented fault-injection boundary, "
            "making a step's intent durable before its side effect runs, "
            "using each step's probe to resolve a step that was interrupted "
            "in flight so no side effect is ever repeated, keeping the "
            "accumulated context across crashes, and an atomically written, "
            "self-repairing checkpoint file. The grading oracle injects a "
            "fault at every boundary of every step and demands exactly-once "
            "execution and a recoverable state. The starter has real bugs "
            "against that contract. Fix jobrunner/ so it fully satisfies "
            "CONTRACT.md. Do not change the public API names/signatures in "
            "CONTRACT.md. You may add tests, but the visible tests in "
            "tests/ must still pass and must not be weakened to hide a bug."
        ),
    },
    "transactions": {
        "package": "txstore",
        "hidden_test_count": 40,
        "objective": (
            "This workspace contains a buggy Python package `txstore/` implementing "
            "a serializable snapshot transactional store and a framed recovery journal. "
            "Read CONTRACT.md for the complete public API and behavior. Repair txstore/ "
            "to satisfy the contract, including point and prefix dependencies, phantom "
            "and ABA detection, savepoint rollback without forgetting reads, defensive "
            "copies, type-sensitive idempotency, atomic append-failure rollback, and "
            "strict canonical framing and recovery. Interleaved transactions are "
            "single-threaded and deterministic. Do not change public API signatures, "
            "the contract, or existing visible tests. You may add tests. Use only "
            "the standard library."
        ),
    },
}


_CASES["leases"] = {
    "package": "leasepool",
    "hidden_test_count": 28,
    "objective": (
        "Repair the Python package leasepool/ to satisfy CONTRACT.md. It implements "
        "durable fenced resource leases with deterministic interleavings, priority "
        "and conflicting-resource fairness, expiry, renewal, terminal idempotency, "
        "atomic append-failure rollback, defensive copies, and strict journal replay. "
        "Use only the standard library. Preserve public signatures and do not modify "
        "CONTRACT.md or existing visible tests. You may add tests."
    ),
}


_CASES["concurrent"] = {
    "package": "txstore",
    "hidden_test_count": 49,
    "objective": (
        "Repair txstore/ to satisfy CONTRACT.md: a thread-safe serializable "
        "snapshot transactional store with savepoints, phantom/ABA detection, "
        "type-sensitive idempotency and strict framed-journal recovery. Different "
        "transactions may commit concurrently. Readers must remain available "
        "during a blocked journal append, and callback reentrant commits must "
        "fail explicitly without deadlock. Preserve atomic publication, retry "
        "semantics and journal ordering. Use only the standard library. Do not "
        "change public signatures, CONTRACT.md or existing visible tests. "
        "You may add tests."
    ),
}

_CASES["eventlog"] = {
    "package": "eventlog",
    "hidden_test_count": 49,
    "objective": (
        "Repair eventlog/ to satisfy CONTRACT.md: a durable append-only event "
        "stream with idempotent publication, consumer groups, expiring "
        "generation-fenced ownership, ordered polling, contiguous acknowledgments, "
        "atomic append-failure behavior and strict full-journal replay. Preserve "
        "public signatures and visible tests. Use only the standard library."
    ),
}


_CASES["concurrent_v2"] = {
    "package": "txstore",
    "hidden_test_count": 67,
    "objective": (
        "Repair txstore/ to satisfy CONTRACT.md: a thread-safe serializable "
        "snapshot store with savepoints, mutation-history conflict checks, "
        "idempotent commits, and strict journal recovery. This version adds "
        "adversarial interleavings and recovery histories, including queued "
        "commit retries, callback reentrancy, read-only validation, ABA and "
        "phantom mutations, defensive copies, and every valid final-frame "
        "truncation. Use only the standard library. Preserve public signatures "
        "and visible tests."
    ),
}


_CASES["matchbook"] = {
    "package": "matchbook",
    "hidden_test_count": 119,
    "objective": (
        "Repair matchbook/ to satisfy CONTRACT.md: a single-instrument limit "
        "order book with price-time priority, market, immediate-or-cancel, "
        "fill-or-kill and good-till-date orders, post-only, three self-trade "
        "prevention modes, iceberg orders with a hidden reserve that refills, "
        "amend and cancel with ownership, expiry on a logical "
        "clock, defensive copies, and a journal with atomic append-failure "
        "behavior and strict whole-journal replay. The starter is a naive first "
        "cut with real bugs and missing behavior. Use only the standard library. "
        "Preserve public signatures; do not modify CONTRACT.md or the existing "
        "visible tests. You may add tests."
    ),
}

_CASES["gridcalc"] = {
    "package": "gridcalc",
    "hidden_test_count": 154,
    "objective": (
        "Repair gridcalc/ to satisfy CONTRACT.md: a spreadsheet engine with a "
        "formula language (operators with stated precedence, coercion and error "
        "rules, twenty-four functions), static dependency cycles, eager "
        "recalculation reported through a callback, row and column inserts and "
        "deletes that rewrite references inside formula text, and copy with "
        "relative and absolute references. The starter is a naive first cut "
        "with real bugs and missing behavior. Use only the standard library. "
        "Preserve public signatures; do not modify CONTRACT.md or the existing "
        "visible tests. You may add tests."
    ),
}

_CASES["billing"] = {
    "package": "invoicing",
    "hidden_test_count": 79,
    "objective": (
        "Repair invoicing/ to satisfy CONTRACT.md: Decimal money with "
        "per-currency minor units and half-even rounding, largest-remainder "
        "allocation, graduated and volume tiered pricing, month-end billing "
        "anchors, half-open proration, and invoices with ordered compounding "
        "discounts, line-level tax and capped credit. Validate inputs exactly "
        "as specified and never mutate arguments. Use only the standard "
        "library. Preserve public signatures and visible tests."
    ),
}

_CASES["semver"] = {
    "package": "semverkit",
    "hidden_test_count": 80,
    "objective": (
        "Repair semverkit/ to satisfy CONTRACT.md: strict SemVer 2.0.0 parsing "
        "and precedence, npm-style ranges with partial, tilde, caret, hyphen "
        "and wildcard forms, the prerelease inclusion rule, min/max selection, "
        "and a deterministic backtracking dependency resolver that validates "
        "the whole registry first. Use only the standard library. Preserve "
        "public signatures and visible tests."
    ),
}

_CASES["minilang"] = {
    "package": "minilang",
    "hidden_test_count": 102,
    "objective": (
        "Repair minilang/ to satisfy CONTRACT.md: a tree-walking interpreter "
        "with exact lexing, operator precedence and associativity, parse-time "
        "checks, strict value semantics, lexical scopes with by-reference "
        "closures, a call-depth limit, display forms, and precisely positioned "
        "error types. Use only the standard library. Preserve public "
        "signatures and visible tests."
    ),
}


def cases():
    """Return the benchmark case list as [{"id", "objective"}, ...]."""
    return [{"id": case_id, "objective": spec["objective"]} for case_id, spec in _CASES.items()]


def prepare(case_id, workspace):
    """Copy case_id's starter code (only) into workspace. Idempotent."""
    spec = _require_case(case_id)
    starter = _FIXTURES_ROOT / case_id / "starter"
    assert starter.is_dir(), f"missing starter fixture for {case_id!r}: {starter}"
    workspace = Path(workspace)
    workspace.mkdir(parents=True, exist_ok=True)
    ignore = shutil.ignore_patterns("__pycache__", "*.pyc")
    for item in starter.iterdir():
        dest = workspace / item.name
        if item.is_dir():
            shutil.copytree(item, dest, dirs_exist_ok=True, ignore=ignore)
        else:
            shutil.copy2(item, dest)
    assert (workspace / spec["package"]).is_dir(), (
        f"prepare({case_id!r}) did not produce a {spec['package']!r} package in {workspace}"
    )


def grade(case_id, workspace, timeout_seconds=_DEFAULT_TIMEOUT_SECONDS):
    """Run the held-out oracle against workspace's package in a subprocess.

    Returns {"passed": int, "total": int, "errors": [str, ...]}. Never
    raises for an ordinary grading failure (bad/missing/hanging/os._exit()-
    ing solution code): that shows up as a low passed count and populated
    errors instead, because the candidate code never runs in this process.
    """
    counted, _timed_out = grade_counted(case_id, workspace, timeout_seconds)
    return counted


def grade_counted(case_id, workspace, timeout_seconds=_DEFAULT_TIMEOUT_SECONDS):
    """Return (counted, timed_out): grade()'s report, and whether its one process reached the limit."""
    spec = _require_case(case_id)
    workspace = Path(workspace)
    hidden_dir = _FIXTURES_ROOT / case_id / "hidden_tests"
    assert hidden_dir.is_dir(), f"missing hidden tests for {case_id!r}: {hidden_dir}"
    assert timeout_seconds > 0, f"timeout_seconds must be positive, got {timeout_seconds!r}"
    return _run_hidden_tests_in_subprocess(case_id, spec, workspace, hidden_dir, timeout_seconds)


def grade_by_test(case_id, workspace, counted, timed_out,
                  per_test_seconds=_PER_TEST_SECONDS, by_test_total_seconds=_BY_TEST_TOTAL_SECONDS):
    """The test-by-test grade beside a report of grade_counted(); it is never the counted grade.

    Returns {"passed", "failed", "hung", "not_run", "total", "hung_tests", "errors", "source"}.
    It repeats the counted report unless the one-process run timed out; then each hidden test
    runs in a process of its own under per_test_seconds, all of them under by_test_total_seconds.
    """
    _require_case(case_id)
    hidden_dir = _FIXTURES_ROOT / case_id / "hidden_tests"
    assert hidden_dir.is_dir(), f"missing hidden tests for {case_id!r}: {hidden_dir}"
    assert 0 < per_test_seconds <= by_test_total_seconds, "a test's limit lies within the total limit"
    if not timed_out:
        return _by_test_report(counted["total"], {"passed": counted["passed"],
                                                  "failed": counted["total"] - counted["passed"]},
                               [], [], BY_TEST_FROM_COUNTED)
    return _grade_test_by_test(case_id, Path(workspace), hidden_dir, per_test_seconds, by_test_total_seconds)


def grade_with_tests(case_id, workspace, timeout_seconds=_DEFAULT_TIMEOUT_SECONDS,
                     per_test_seconds=_PER_TEST_SECONDS, by_test_total_seconds=_BY_TEST_TOTAL_SECONDS):
    """Return (counted, by_test): the report of grade_counted() and the one of grade_by_test()."""
    assert 0 < per_test_seconds <= by_test_total_seconds, "a test's limit lies within the total limit"
    counted, timed_out = grade_counted(case_id, workspace, timeout_seconds)
    return counted, grade_by_test(case_id, workspace, counted, timed_out, per_test_seconds, by_test_total_seconds)


def _require_case(case_id):
    if case_id not in _CASES:
        raise KeyError(f"unknown harness_bench case: {case_id!r}; known: {sorted(_CASES)}")
    return _CASES[case_id]


def _run_hidden_tests_in_subprocess(case_id, spec, workspace, hidden_dir, timeout_seconds):
    command = [sys.executable, str(Path(__file__).resolve()), "--run-hidden",
               case_id, str(workspace), str(hidden_dir)]
    try:
        completed = subprocess.run(command, capture_output=True, text=True,
                                    timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        return _failure_report(spec, [f"hidden tests timed out after {timeout_seconds}s"]), True
    report = _parse_subprocess_report(completed.stdout)
    if report is None:
        return _failure_report(spec, [_summarize_subprocess_failure(completed)]), False
    return report, False


def _grade_test_by_test(case_id, workspace, hidden_dir, per_test_seconds, total_seconds):
    """Each hidden test in its own process, so a hanging test costs itself and no other."""
    deadline = time.monotonic() + total_seconds
    base = [sys.executable, str(Path(__file__).resolve())]
    where = [case_id, str(workspace), str(hidden_dir)]
    listed, error = _run_bounded([*base, "--list-hidden", *where], total_seconds)
    test_ids = (listed or {}).get("tests")
    if not isinstance(test_ids, list) or not test_ids:
        total = _require_case(case_id)["hidden_test_count"]
        return _by_test_report(total, {}, [], [f"hidden tests could not be listed: {error or 'no test ids'}"],
                               BY_TEST_FROM_PROCESSES)
    counts, hung = {"passed": 0, "failed": 0}, []
    for test_id in test_ids:
        # A test that cannot have its full limit is left not run rather than called hung.
        if deadline - time.monotonic() < per_test_seconds:
            break
        report, error = _run_bounded([*base, "--run-hidden", *where, test_id], per_test_seconds)
        if error == _TIMED_OUT:
            hung.append(test_id)
            continue
        passed = report is not None and report.get("passed") == 1 and report.get("total") == 1
        counts["passed" if passed else "failed"] += 1
    return _by_test_report(len(test_ids), counts, hung, [], BY_TEST_FROM_PROCESSES)


def _run_bounded(command, timeout_seconds):
    """(report, error) of one grading subprocess; a report is None when the process gave none."""
    assert command and timeout_seconds > 0, "a grading subprocess needs a command and a limit"
    try:
        completed = subprocess.run(command, capture_output=True, text=True, timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        return None, _TIMED_OUT
    report = _parse_subprocess_report(completed.stdout)
    if report is None:
        return None, _summarize_subprocess_failure(completed)
    assert isinstance(report, dict), f"a grading report is a mapping, got {type(report).__name__}"
    return report, None


def _by_test_report(total, counts, hung_tests, errors, source):
    passed, failed = counts.get("passed", 0), counts.get("failed", 0)
    not_run = total - passed - failed - len(hung_tests)
    assert min(passed, failed, not_run) >= 0, f"impossible test-by-test grade: {counts} of {total}"
    assert source in (BY_TEST_FROM_COUNTED, BY_TEST_FROM_PROCESSES), f"unknown source {source!r}"
    return {"passed": passed, "failed": failed, "hung": len(hung_tests), "not_run": not_run, "total": total,
            "hung_tests": list(hung_tests), "errors": list(errors), "source": source}


def _summarize_subprocess_failure(completed):
    tail = completed.stderr.strip().splitlines()
    detail = tail[-1] if tail else f"exit code {completed.returncode}, no stderr output"
    return f"hidden test subprocess produced no report: {detail}"


def _failure_report(spec, errors):
    return {"passed": 0, "total": spec["hidden_test_count"], "errors": errors}


def _parse_subprocess_report(stdout):
    for line in stdout.splitlines():
        if line.startswith(_REPORT_MARKER):
            try:
                return json.loads(line[len(_REPORT_MARKER):])
            except json.JSONDecodeError:
                return None
    return None


class _GradingResult(unittest.TestResult):
    def __init__(self):
        super().__init__()
        self.passed = 0

    def addSuccess(self, test):
        super().addSuccess(test)
        self.passed += 1


def _discover(hidden_dir):
    return unittest.TestLoader().discover(str(hidden_dir), pattern="test_*.py",
                                          top_level_dir=str(hidden_dir))


def _test_ids(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from _test_ids(item)
        else:
            yield item.id()


def _discover_and_run(hidden_dir, test_id=None):
    suite = _discover(hidden_dir) if test_id is None else unittest.TestLoader().loadTestsFromName(test_id)
    result = _GradingResult()
    suite.run(result)
    errors = [_format_case_error(case, trace) for case, trace in result.errors + result.failures]
    total = result.testsRun
    # Several failing subtests still represent one failed top-level test.
    # Count reported successes, not failures subtracted from testsRun.
    passed = result.passed
    assert 0 <= passed <= total, f"impossible grade: passed={passed} total={total}"
    return {"passed": passed, "total": total, "errors": errors}


def _format_case_error(case, trace):
    return f"{case.id()}: {trace.strip().splitlines()[-1]}"


def _format_import_error(exc):
    return f"could not import solution package: {exc!r}"


def _main_run_hidden(case_id, workspace, hidden_dir, test_id=None):
    """Subprocess entry point (see module docstring). Runs entirely inside a
    fresh interpreter, so a candidate that hangs, calls sys.exit(), or
    mutates global/builtin state can never affect the grading process."""
    spec = _require_case(case_id)
    sys.path.insert(0, str(hidden_dir))
    sys.path.insert(0, str(workspace))
    try:
        importlib.import_module(spec["package"])
    except BaseException as exc:  # noqa: BLE001 - report any import failure, incl. SystemExit
        report = {"passed": 0, "total": spec["hidden_test_count"],
                  "errors": [_format_import_error(exc)]}
    else:
        report = _discover_and_run(hidden_dir, test_id)
    print(_REPORT_MARKER + json.dumps(report))


def _main_list_hidden(case_id, workspace, hidden_dir):
    """Subprocess entry point: the ids of the hidden tests, in the order the one-process run takes them."""
    _require_case(case_id)
    sys.path.insert(0, str(hidden_dir))
    sys.path.insert(0, str(workspace))
    tests = list(_test_ids(_discover(hidden_dir)))
    assert len(set(tests)) == len(tests), "hidden test ids must be unique"
    print(_REPORT_MARKER + json.dumps({"tests": tests}))


if __name__ == "__main__":
    if len(sys.argv) in (5, 6) and sys.argv[1] == "--run-hidden":
        _main_run_hidden(sys.argv[2], Path(sys.argv[3]), Path(sys.argv[4]), *sys.argv[5:])
    elif len(sys.argv) == 5 and sys.argv[1] == "--list-hidden":
        _main_list_hidden(sys.argv[2], Path(sys.argv[3]), Path(sys.argv[4]))
    else:
        raise SystemExit(
            "usage: harness_bench_cases.py --run-hidden|--list-hidden <case_id> <workspace> <hidden_dir> [test_id]"
        )
