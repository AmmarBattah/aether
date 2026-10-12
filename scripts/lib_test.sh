#!/usr/bin/env bash
# Tests for scripts/lib.sh's with_timeout and resolve_path. Targets bash 3.2
# like lib.sh itself; run directly or from scripts/ci.sh.
#   scripts/lib_test.sh
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
. scripts/lib.sh

fail=0
ok() { printf 'ok - %s\n' "$1"; }
bad() { printf 'FAIL - %s: %s\n' "$1" "$2"; fail=1; }
check() { # desc got want
  if [ "$2" = "$3" ]; then ok "$1"; else bad "$1" "got [$2] want [$3]"; fi
}

WORK=$(mktemp -d "$(tmp_base)/lib_test-XXXXXX") || { echo "mktemp -d failed"; exit 1; }
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

# --- with_timeout: stdin passthrough (bash 3.2 drops it without `<&0`) ---
out=$( ( printf 'hello\n'; sleep 1 ) | with_timeout 20 cat )
check "stdin passthrough" "$out" "hello"

# --- with_timeout: exit code passthrough ---
with_timeout 20 true
check "exit 0 passthrough" "$?" 0

with_timeout 20 bash -c 'exit 7'
check "exit 7 passthrough" "$?" 7

with_timeout 1 sleep 5
check "timeout returns 124" "$?" 124

# --- with_timeout: early finish returns immediately, no orphaned sleep ---
start=$(date +%s)
out=$(with_timeout 15 printf 'done\n' | cat)
end=$(date +%s)
elapsed=$((end - start))
if [ "$out" = "done" ] && [ "$elapsed" -lt 5 ]; then
  ok "early finish returns immediately (${elapsed}s)"
else
  bad "early finish returns immediately" "out=[$out] elapsed=${elapsed}s"
fi

# --- with_timeout: deadline kills a grandchild via the process group ---
gc_marker="$WORK/grandchild.pid"
with_timeout 2 bash -c "sleep 20 & echo \$! > '$gc_marker'; wait" >/dev/null 2>&1 || true
sleep 1
gpid=$(cat "$gc_marker" 2>/dev/null || true)
if [ -n "$gpid" ] && kill -0 "$gpid" 2>/dev/null; then
  bad "deadline kills a grandchild" "pid $gpid still alive"
else
  ok "deadline kills a grandchild"
fi

# --- resolve_path: existing file ---
mkdir -p "$WORK/subject/adir"
: > "$WORK/subject/adir/afile.txt"
got=$(cd "$WORK/subject" && resolve_path adir/afile.txt)
check "resolve_path on file" "$got" "$WORK/subject/adir/afile.txt"

# --- resolve_path: existing dir ---
got=$(cd "$WORK/subject" && resolve_path adir)
check "resolve_path on dir" "$got" "$WORK/subject/adir"

# --- resolve_path: missing final component, parent exists (should succeed) ---
got=$(cd "$WORK/subject" && resolve_path adir/missing.txt)
check "resolve_path with missing final component" "$got" "$WORK/subject/adir/missing.txt"

# --- resolve_path: missing parent directory (should fail) ---
if (cd "$WORK/subject" && resolve_path nosuchdir/file.txt) >/dev/null 2>&1; then
  bad "resolve_path with missing parent" "unexpectedly succeeded"
else
  ok "resolve_path with missing parent fails"
fi

# --- resolve_path: CDPATH must not leak into the cd it does internally ---
mkdir -p "$WORK/cdpath_root/decoy"
if (cd "$WORK/subject" && CDPATH="$WORK/cdpath_root" resolve_path decoy/f) >/dev/null 2>&1; then
  bad "resolve_path ignores CDPATH" "resolved decoy/f via CDPATH instead of failing"
else
  ok "resolve_path ignores CDPATH"
fi

# --- a TMPDIR with a trailing slash (macOS's default shape) must not break
# the resolve_path expectations above; re-run the suite under one. The guard
# var stops this sub-invocation from recursing into the same check. ---
if [ -z "${LIB_TEST_TRAILING_SLASH_CHECK:-}" ]; then
  slash_tmpdir="$WORK/trailing_slash_tmpdir/"
  mkdir -p "$slash_tmpdir"
  if LIB_TEST_TRAILING_SLASH_CHECK=1 TMPDIR="$slash_tmpdir" "$0" >/dev/null 2>&1; then
    ok "passes with a trailing-slash TMPDIR"
  else
    bad "passes with a trailing-slash TMPDIR" "sub-invocation failed"
  fi
fi

if [ "$fail" -ne 0 ]; then
  printf '\nlib_test: FAILED\n'
  exit 1
fi
printf '\nlib_test: all checks passed\n'
