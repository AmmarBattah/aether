"""Bounded process-group lifetime for benchmark subprocesses."""
import os
import signal
import subprocess
import threading
import time

# Live process groups this runner started. Parallel blocks need one place to
# stop them all on an interrupt; each trial still stops only its own group.
_OWNED_LOCK = threading.Lock()
_OWNED = set()
_SHUTTING_DOWN = False


def own_process_group(proc):
    """Track a group leader started with start_new_session=True until it is reaped."""
    assert proc.pid > 0, "only a started process can be owned"
    with _OWNED_LOCK:
        if not _SHUTTING_DOWN:
            _OWNED.add(proc)
            return
    _terminate_process_group(proc)
    raise RuntimeError("benchmark runner is shutting down; refused to keep a new process group")


def disown_process_group(proc):
    with _OWNED_LOCK:
        _OWNED.discard(proc)


def terminate_owned_process_groups():
    """Stop every live owned group and refuse new ones; returns how many were stopped."""
    global _SHUTTING_DOWN
    with _OWNED_LOCK:
        _SHUTTING_DOWN = True
        doomed = list(_OWNED)
        _OWNED.clear()
    for proc in doomed:
        _terminate_process_group(proc)
    assert not _OWNED, "no process group may be adopted during shutdown"
    return len(doomed)


def run_subprocess_capturing(command, cwd, timeout, stdout_path, stderr_path, env=None):
    """Run `command` as its own process group, capturing raw stdout/stderr to
    files. Always stops the whole group before returning. `env=None` means
    "inherit as-is"; callers that need isolation must pass a sanitized env
    explicitly rather than relying on ambient inheritance."""
    assert timeout > 0, "a native trial needs a positive timeout"
    assert os.path.isdir(cwd), f"native trial cwd does not exist: {cwd}"
    started = time.monotonic()
    exit_code, timed_out = None, False
    with open(stdout_path, "wb") as out, open(stderr_path, "wb") as err:
        proc = subprocess.Popen(command, cwd=cwd, stdout=out, stderr=err, env=env, start_new_session=True)
        try:
            own_process_group(proc)
            exit_code = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
        finally:
            cleanup_error = _terminate_process_group(proc)
            disown_process_group(proc)
    return {
        "command": command, "exit_code": exit_code, "timed_out": timed_out,
        "cleanup_error": cleanup_error, "seconds": round(time.monotonic() - started, 3),
    }


def _terminate_process_group(proc):
    """Stop the group even when its leader has already exited."""
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        proc.wait(timeout=5)
        return None
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        # Escalation below also covers descendants ignoring SIGTERM.
        pass
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass  # The group exited between signals.
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        return "process group leader still alive after cleanup"
    return None
