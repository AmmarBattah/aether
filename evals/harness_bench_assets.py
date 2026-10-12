"""Frozen fixtures, grading, and immutable benchmark provenance."""
import hashlib
import os
import shutil
import subprocess
from pathlib import Path

from aether_client import init_git_workspace
import harness_bench_cases as cases_module
import harness_bench_usage as usage_module
from harness_bench_metadata import native_configured_defaults

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(HERE)

# Fixture files this harness freezes best-effort: harness_bench_cases.py owns
# the fixture layout, but any directory literally named "tests" (its visible
# tests) or one of these filenames (its contract/config) is treated as public
# and must not be modified by a solver. This is advisory, not a security
# boundary: see evals/harness_bench_protocol.md.
FROZEN_DIR_NAMES = {"tests"}
FROZEN_FILE_NAMES = {"conftest.py", "contract.md"}

def snapshot_frozen_files(workspace):
    digest = {}
    for root, dirs, files in os.walk(workspace):
        dirs[:] = [d for d in dirs if d not in (".git", "__pycache__")]
        rel_parts = Path(root).relative_to(workspace).parts
        frozen_dir = any(part in FROZEN_DIR_NAMES for part in rel_parts)
        for name in files:
            if not frozen_dir and name.lower() not in FROZEN_FILE_NAMES:
                continue
            path = Path(root, name)
            digest[str(path.relative_to(workspace))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return digest


def frozen_violations(before, after):
    changed = sorted(p for p in before if before[p] != after.get(p))
    removed = sorted(p for p in before if p not in after)
    return {"changed": changed, "removed": removed} if (changed or removed) else None


# ----- fixture prepare / grade wrappers -------------------------------------


def prepare_workspace(case_id, workspace):
    workspace.mkdir(parents=True, exist_ok=True)
    cases_module.prepare(case_id, workspace)
    assert any(workspace.iterdir()), f"prepare() left an empty workspace for case {case_id!r}"
    init_git_workspace(str(workspace))
    return snapshot_frozen_files(workspace)


BY_TEST_NEEDS_COUNTED = "not graded test by test: the counted grade failed"
BY_TEST_PARTS = ("passed", "failed", "hung", "not_run")


def run_grade(case_id, workspace):
    """(counted grade, error): the counted half of run_grade_with_tests()."""
    result, _timed_out, error = _run_counted_grade(case_id, workspace)
    return result, error


def run_grade_with_tests(case_id, workspace):
    """(counted grade, test-by-test grade, counted error, test-by-test error).

    The second grade is recorded beside the first and never counted; its failure leaves the first as it is.
    """
    result, timed_out, error = _run_counted_grade(case_id, workspace)
    if error is not None:
        return None, None, error, BY_TEST_NEEDS_COUNTED
    by_test, by_test_error = _run_grade_by_test(case_id, workspace, result, timed_out)
    assert (by_test is None) != (by_test_error is None), "a test-by-test grade or its error, never both or neither"
    return result, by_test, None, by_test_error


def _run_counted_grade(case_id, workspace):
    """(counted grade, whether its one process timed out, error)."""
    try:
        result, timed_out = cases_module.grade_counted(case_id, workspace)
        if not isinstance(result, dict) or not {"passed", "total", "errors"} <= result.keys():
            raise ValueError("grade() contract violated")
        if (type(result["passed"]) is not int or type(result["total"]) is not int
                or not 0 <= result["passed"] <= result["total"]):
            raise ValueError("grade() returned impossible counters")
        return result, timed_out, None
    except Exception as error:  # A fixture/grader bug must not crash the matrix.
        return None, False, f"grade() raised {type(error).__name__}: {error}"


def _run_grade_by_test(case_id, workspace, counted, timed_out):
    """(test-by-test grade, error): when it fails it is unknown, with the reason, and never a made-up grade."""
    try:
        by_test = cases_module.grade_by_test(case_id, workspace, counted, timed_out)
        parts = [by_test[key] for key in BY_TEST_PARTS]
        if type(by_test["total"]) is not int or any(type(part) is not int for part in parts):
            raise ValueError("the test-by-test grade holds a count that is not an integer")
        if not all(0 <= part <= by_test["total"] for part in parts):
            raise ValueError("the test-by-test grade holds a count outside 0 to its total")
        if sum(parts) != by_test["total"]:
            raise ValueError("the test-by-test grade does not add up to its total")
        return by_test, None
    except Exception as error:  # The counted grade is already made and must stand.
        return None, f"grade_by_test() raised {type(error).__name__}: {error}"


# ----- provenance / binary pinning ------------------------------------------


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pin_binary(binary_path, output_dir):
    """Copy the resolved aether binary into --output-dir once per harness
    invocation, so a rebuild/reinstall mid-matrix cannot change trials."""
    assert os.path.isfile(binary_path), f"aether binary not found: {binary_path}"
    pinned_dir = os.path.join(output_dir, "bin")
    os.makedirs(pinned_dir, exist_ok=True)
    pinned_path = os.path.join(pinned_dir, "aether")
    shutil.copy2(binary_path, pinned_path)
    os.chmod(pinned_path, 0o755)
    return pinned_path, sha256_file(pinned_path)


def native_cli_provenance(name):
    path = shutil.which(name)
    if not path:
        return {"path": None, "sha256": None, "version": None}
    version = subprocess.run([path, "--version"], capture_output=True, text=True, timeout=10, check=False)
    return {"path": path, "sha256": sha256_file(path), "version": (version.stdout or version.stderr).strip(),
            "configured_defaults": native_configured_defaults(name)}




def benchmark_hashes():
    paths = [Path(HERE) / "harness_bench.py", Path(cases_module.__file__), Path(usage_module.__file__),
             Path(HERE) / "aether_client.py", Path(HERE) / "test_harness_bench.py",
             Path(HERE) / "harness_bench_pricing.py", Path(HERE) / "harness_bench_reprice.py",
             Path(HERE) / "harness_bench_metadata.py", Path(HERE) / "harness_bench_receipts.py",
             Path(HERE) / "test_harness_bench_receipts.py", Path(HERE) / "test_harness_bench_metadata.py",
             Path(HERE) / "harness_bench_treatments.py", Path(HERE) / "test_harness_bench_treatments.py"]
    paths.extend(Path(HERE).glob("harness_bench*.py"))
    paths.extend(Path(HERE).glob("test_harness_bench*.py"))
    paths.extend((Path(REPO_ROOT) / "clients/python/aether").glob("*.py"))
    fixtures = Path(HERE) / "fixtures/harness_bench"
    paths.extend(p for p in fixtures.rglob("*") if p.is_file()
                 and {"starter", "hidden_tests"}.intersection(p.relative_to(fixtures).parts)
                 and "__pycache__" not in p.parts and p.suffix != ".pyc")
    return {str(p.relative_to(REPO_ROOT)): sha256_file(p) for p in sorted(set(paths)) if p.is_file()}


def fixture_digest(case_id):
    """One sha256 over a case's frozen files (starter and hidden checks) as benchmark_hashes() sees them."""
    prefix = f"evals/fixtures/harness_bench/{case_id}/"
    entries = sorted((path, digest) for path, digest in benchmark_hashes().items() if path.startswith(prefix))
    assert any("/hidden_tests/" in path for path, _ in entries), f"{case_id}: no hidden checks to freeze"
    assert any("/starter/" in path for path, _ in entries), f"{case_id}: no starter to freeze"
    listing = "".join(f"{digest}  {path}\n" for path, digest in entries)
    return hashlib.sha256(listing.encode()).hexdigest()


def verify_benchmark_hashes(expected):
    actual = benchmark_hashes()
    return sorted(path for path in expected.keys() | actual.keys()
                  if expected.get(path) != actual.get(path))




def git_provenance(repo_root):
    """HEAD and dirty/untracked *paths* only - never diff content, so a
    committed manifest cannot leak a secret sitting in a working-tree edit."""
    def git(*args):
        return subprocess.run(["git", *args], cwd=repo_root, capture_output=True, text=True,
                               timeout=10, check=True).stdout
    # Status columns begin with meaningful spaces. NUL records also preserve
    # whitespace/newlines in paths without Git's display quoting.
    records = iter(git("status", "--porcelain=v1", "-z").split("\0"))
    changed = []
    for record in records:
        if not record:
            continue
        if len(record) < 4 or record[2] != " ":
            raise ValueError("malformed git status record")
        changed.append(record[3:])
        if "R" in record[:2] or "C" in record[:2]:
            source = next(records, "")
            if not source:
                raise ValueError("git rename/copy record is missing its source")
            changed.append(source)
    return {"head": git("rev-parse", "HEAD").strip(), "dirty": bool(changed), "changed_paths": changed}
