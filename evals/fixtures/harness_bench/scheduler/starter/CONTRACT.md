# Scheduler contract

Implement a dependency-aware task scheduler with cycle detection, bounded
retries, deadlines, and deterministic ordering. Fix the bugs in `scheduler/`
so it satisfies this contract exactly. Run
`python3 -m unittest discover -s tests` for the visible smoke tests. All
timing is via the injected virtual `Clock`; nothing here calls `time.sleep`
or the real wall clock, and none of it should.

## `scheduler.errors.SchedulerError(Exception)`

Raised for invalid graphs (duplicate/unknown task ids, cycles).

## `scheduler.clock.Clock`

- `Clock(start=0.0)`
- `now() -> float`
- `advance(seconds) -> float` — moves virtual time **forward by** `seconds`
  (accumulates; does not just set an absolute value) and returns the new
  `now()`. Raises `ValueError` if `seconds < 0`.

## `scheduler.queue.ReadyQueue`

- `push(task_id) -> None`
- `pop_next() -> str` — removes and returns the smallest ready `task_id` in
  ascending lexicographic order, **independent of push order**. Raises
  `IndexError` if empty.
- `is_empty() -> bool`

## `scheduler.graph.DependencyGraph`

- `add_node(node_id) -> None` — raises `SchedulerError` on a duplicate.
- `add_edge(node_id, depends_on) -> None`
- `nodes() -> list[str]` (sorted)
- `dependencies_of(node_id) -> list[str]` (sorted)
- `check_acyclic() -> None` — raises `SchedulerError` for **any** cycle,
  **regardless of the graph's overall shape**:
  - a cycle with no node outside it at all (no root exists anywhere),
  - a cycle in one connected component while an unrelated component elsewhere
    in the same graph *does* have a root (the presence of a root somewhere
    must not mask a rootless cycle elsewhere),
  - a self-loop (a node that depends on itself).

  It must **not** raise for any acyclic graph, including a diamond (a node
  reachable through more than one path, e.g. `d` depends on `b` and `c`, both
  of which depend on `a`).

## `scheduler.runner`

- `Task(task_id, deps=(), action=None, max_attempts=1, retry_backoff=0.0, deadline=None)`
  — `action` is a zero-arg callable that may raise to signal failure. Raises
  `SchedulerError` at construction time if `action is None`, if
  `max_attempts < 1`, or if `retry_backoff < 0`.
- `TaskResult`: `.status` in `{"success", "failed", "skipped"}`, `.attempts`, `.error`.
- `RunReport`: `.order` (attempted task_ids, run order), `.results` (dict with
  exactly one entry per task ever added, whenever `run()` returns normally).
- `Scheduler(clock)`
  - `add_task(task) -> None` — raises `SchedulerError` on a duplicate id.
  - `run() -> RunReport` — validates the graph first: raises `SchedulerError`
    for any dependency on an unregistered task id, and for any cycle (per
    `check_acyclic()` above, including self-loops and disconnected-component
    cycles), **before running anything**.

    A task runs only once every dependency has status `"success"`; if **any**
    dependency ends `"failed"` **or** `"skipped"` — including a task with
    several dependencies (fan-in) where only one of them did not succeed —
    the task itself becomes `"skipped"` (with `attempts == 0`) without ever
    calling `action`, and this propagates transitively through chains of
    skipped tasks.

    Among tasks that become ready at the same point, they run in **ascending
    task_id order**, not push/insertion order.

    For a runnable task: before **every** attempt (the first one and every
    retry), if `deadline` is set and `clock.now() > deadline` (**strictly**
    greater — a deadline exactly equal to the current clock time has *not*
    been exceeded and the attempt proceeds normally), the task becomes
    `"skipped"` with `error="deadline exceeded"` and no further attempts are
    made — including when the deadline is only crossed *during* a retry, not
    just before the first attempt. Otherwise `action()` runs; on success the
    task is `"success"` with the attempt count so far. On an exception, if
    attempts made so far are still below `max_attempts`, the clock advances
    by `retry_backoff` and it retries; once `max_attempts` attempts have all
    failed, the task is `"failed"` with `str(exception)`.
