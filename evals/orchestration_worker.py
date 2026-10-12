"""Track the isolated Linux fixture worker without PID-reuse ambiguity."""
import json
import os
import select
from contextlib import contextmanager
from pathlib import Path


@contextmanager
def track_worker(workspace):
    if not hasattr(os, "pidfd_open"):
        raise RuntimeError("worker-exit verification requires Linux pidfd support")
    marker = json.loads((workspace / "worker.started").read_text())
    pid = marker.get("pid")
    if type(pid) is not int or pid <= 1:
        raise ValueError("invalid fixture worker PID")
    namespace = marker.get("pid_namespace")
    if type(namespace) is not int or namespace <= 0:
        raise ValueError("invalid fixture PID namespace")
    matches = []
    for process in Path("/proc").iterdir():
        if not process.name.isdecimal():
            continue
        try:
            if (process / "ns/pid").stat().st_ino != namespace:
                continue
            identities = [line.split()[1:] for line in (process / "status").read_text().splitlines()
                          if line.startswith("NSpid:")]
            if identities and int(identities[0][-1]) == pid:
                matches.append(process)
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            # Unrelated host processes can exit or be inaccessible during this scan.
            continue
    if len(matches) != 1:
        raise RuntimeError(f"expected one host process for fixture identity, found {len(matches)}")
    process = matches[0]
    # This is observation only: never signal a process named by a model-written
    # file. The pidfd remains tied to this process even if its PID is reused.
    descriptor = os.pidfd_open(int(process.name))
    try:
        if (process / "ns/pid").stat().st_ino != namespace:
            raise RuntimeError("fixture process identity changed while opening pidfd")
        if (process / "cwd").resolve() != workspace.resolve():
            raise ValueError("fixture worker is not in the isolated workspace")
        if worker_exited(descriptor):
            raise RuntimeError("fixture worker exited before cancellation")
        yield descriptor
    finally:
        os.close(descriptor)


def worker_exited(descriptor):
    ready, _, _ = select.select([descriptor], [], [], 0)
    return bool(ready)
