# Portable helpers shared by scripts/*.sh. Targets bash 3.2 (macOS's
# /bin/bash) and POSIX tools only - no GNU-only flags, no bash 4+ features.
# Source, don't execute: `. "$(dirname "$0")/lib.sh"` after cd'ing to repo root.

# resolve_path PATH - print an absolute path for PATH's existence/failure
# semantics the way `realpath` would, but purely logical: it does not
# resolve symlinks (unlike GNU realpath), it just cd's into the target's
# directory and reads it back from `pwd`. Like `realpath`, it fails (via
# set -e) if that directory does not exist. macOS ships no `realpath` at
# all on older releases, so this stays pure bash.
resolve_path() {
  local target=$1
  if [ -d "$target" ]; then
    (CDPATH='' cd -- "$target" >/dev/null && pwd)
    return
  fi
  local dir base
  dir=$(dirname -- "$target")
  base=$(basename -- "$target")
  (CDPATH='' cd -- "$dir" >/dev/null && printf '%s/%s\n' "$(pwd)" "$base")
}

# tmp_base - print TMPDIR (or /tmp) with any trailing slashes stripped, so
# callers building "$(tmp_base)/name-XXXXXX" never hand mktemp a double
# slash (macOS's default TMPDIR ends in one).
tmp_base() {
  printf '%s\n' "${TMPDIR:-/tmp}" | sed 's:/*$::'
}

# with_timeout SECS CMD... - run CMD; if it is still running after SECS,
# send SIGTERM to its whole process group, then SIGKILL after a short grace
# if it is still alive. Exits 124 on timeout, otherwise CMD's own status.
# Differs from GNU `timeout`: that signals the command alone unless told
# otherwise, and only escalates to SIGKILL when passed `-k`. macOS ships no
# `timeout` at all; this avoids depending on coreutils/perl.
with_timeout() {
  local secs=$1; shift
  local grace=2
  local marker
  marker=$(mktemp "$(tmp_base)/with_timeout.XXXXXX") || {
    echo "with_timeout: mktemp failed; refusing to run without a timeout guard" >&2
    return 1
  }

  # set -m puts the child in its own process group (pgid == its pid), so the
  # deadline below can signal the whole group, not just the direct child.
  set -m
  "$@" <&0 &
  local cmd_pid=$!
  set +m

  ( # Poll instead of a single blocking sleep, so this exits (and stops
    # holding descriptors open) the moment the command finishes early.
    i=0
    while [ "$i" -lt "$secs" ]; do
      sleep 1
      kill -0 "$cmd_pid" 2>/dev/null || exit 0
      i=$((i + 1))
    done
    printf '1' > "$marker"
    kill -TERM -- "-$cmd_pid" 2>/dev/null || kill -TERM "$cmd_pid" 2>/dev/null || true
    sleep "$grace"
    kill -0 "$cmd_pid" 2>/dev/null \
      && { kill -KILL -- "-$cmd_pid" 2>/dev/null || kill -KILL "$cmd_pid" 2>/dev/null || true; }
  ) &
  local watcher_pid=$!

  local status=0
  wait "$cmd_pid" 2>/dev/null || status=$?

  kill "$watcher_pid" 2>/dev/null || true
  wait "$watcher_pid" 2>/dev/null || true

  if [ -s "$marker" ]; then
    rm -f "$marker"
    return 124
  fi
  rm -f "$marker"
  return "$status"
}

# Wait for a daemon launched as our foreground child. The readiness deadline
# accommodates cold debug initialization; release performance has its own gate.
wait_for_test_daemon() {
  local binary=$1 pid=$2 log=$3 timeout=$4
  local deadline=$((SECONDS + timeout))
  while [ "$SECONDS" -lt "$deadline" ] && kill -0 "$pid" 2>/dev/null; do
    if with_timeout 2 "$binary" status >/dev/null 2>&1; then return 0; fi
    sleep 0.1
  done
  if kill -0 "$pid" 2>/dev/null; then
    printf 'test daemon readiness timed out after %ss (pid %s)\n' "$timeout" "$pid" >&2
  else
    printf 'test daemon exited before readiness (pid %s)\n' "$pid" >&2
  fi
  if [ -f "$log" ]; then tail -50 "$log" >&2; else printf 'daemon log was not created\n' >&2; fi
  return 1
}

# Only pass a direct child owned by this shell. Even a daemon that has not
# opened its socket must be stopped and reaped when its fixture exits.
stop_test_child() {
  local pid=$1 attempt
  kill -TERM "$pid" 2>/dev/null || true
  for attempt in $(seq 1 50); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  if kill -0 "$pid" 2>/dev/null; then kill -KILL "$pid" 2>/dev/null || true; fi
  wait "$pid" 2>/dev/null || true
}
