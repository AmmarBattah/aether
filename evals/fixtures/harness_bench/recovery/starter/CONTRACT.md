# Jobrunner contract

`jobrunner/` runs a multi-step job that can be interrupted at any step
boundary and resumed in another process. Fix the bugs in `jobrunner/` so it
satisfies this contract exactly. Run `python3 -m unittest discover -s tests`
for the visible smoke tests. Do not weaken or delete any check described here,
and do not change any public name/signature listed below. Standard library
only; no threads, no wall-clock or random behaviour.

The graded oracle injects a fault at **every** boundary of **every** step and
then resumes the job in a fresh `JobRunner`, so these three invariants matter
more than anything else:

1. **Exactly once.** Across any number of crash/resume cycles, each step's
   `action` is performed exactly once, provided its `probe` answers honestly,
   and the finished job's context is the same as an uninterrupted run's — so
   an action's result must be durable before the action can be lost.
2. **Intent is durable before the side effect.** The checkpoint recording that
   a step is in flight is on disk *before* that step's `action` runs, so a
   crash can always be diagnosed afterwards.
3. **A damaged checkpoint is repairable.** A crash can damage the live
   checkpoint file but never both it and its backup; `load()` repairs itself
   from whichever copy survived and never adopts a half-written temp file.

## `jobrunner.errors`

- `JobError(Exception)` — invalid job/step/hook definition, or a checkpoint
  that belongs to a different job.
- `CheckpointError(JobError)` — a checkpoint exists but no readable copy of
  it does, or a payload that cannot be stored.
- `InjectedFault(Exception)` — raised by a fault hook to simulate the process
  dying. **The runner must never catch it**; only durable state survives it.

## `jobrunner.faults`

Boundary names, in the order the runner reaches them:

- `JOB_START = "job_start"` — once, before any step.
- `BEFORE_INTENT = "before_intent"` — before the in-flight checkpoint.
- `AFTER_INTENT = "after_intent"` — after the in-flight checkpoint is durable
  and before the action (or the probe, when resuming) runs.
- `AFTER_ACTION = "after_action"` — after an action ran *and its result was
  checkpointed*, before its completion checkpoint. **Skipped when a probe
  resolved the step instead.**
- `AFTER_COMMIT = "after_commit"` — after the completion checkpoint is durable.
- `JOB_END = "job_end"` — once, just before `run()` returns (for a completed
  *and* for a failed job).
- `BOUNDARIES` — all six, in that order.

A fault hook is `hook(boundary: str, info: dict) -> None`. `info` carries
`{"step": name, "index": position}` for per-step boundaries and
`{"step": None, "index": None, "resumed": bool}` / `{..., "status": str}` for
`JOB_START` / `JOB_END`. Steps that are skipped because they already completed
produce **no** boundaries.

- `crash_at(boundary, step=None, times=1)` — returns a hook that raises
  `InjectedFault` on its first `times` matching calls and returns `None`
  otherwise (including every call after the budget is spent). Matching means
  the boundary is `boundary` and, when `step` is given, `info["step"] == step`.
  Raises `JobError` for an unknown boundary or `times < 1`.
- `record_boundaries(log)` — returns a hook that appends
  `(boundary, info["step"])` to the list `log` and never raises.

## `jobrunner.steps`

- `Step(name, action, probe)` — raises `JobError` unless `name` is a non-empty
  string and `action`/`probe` are callables.
  - `action(context: dict) -> dict` — performs the side effect and returns the
    updates to merge into the job context. It may raise to signal an ordinary
    step failure; when it does, it must not have applied its side effect.
  - `probe(context: dict) -> bool` — True when this step's side effect is
    already present. It is consulted **only** when resuming a step that was
    in flight, and it is what makes a crash between the action and its
    completion checkpoint safe.
- `validate_steps(steps) -> tuple` — raises `JobError` for an empty job, a
  non-`Step` entry, or a duplicate step name.

## `jobrunner.checkpoint`

The checkpoint lives in three files: `<path>` (live), `<path>.bak` (the
previous version) and `<path>.tmp` (a write in progress). A stored record is
`{"version": int, "payload": dict, "checksum": str}`; the checksum covers the
version and the payload, so a truncated or edited file is detectable.

- `CheckpointStore(path)` — `path` and `backup_path` properties.
- `save(payload) -> dict` — writes `<path>.tmp`, demotes the current `<path>`
  to `<path>.bak`, then renames the temp over `<path>`; the live file is
  never written in place and the temp never survives. The stored version is
  one higher than the newest readable version. Raises `CheckpointError` if
  `payload` is not a dict or is not JSON serializable — without disturbing
  the checkpoint already on disk.
- `load() -> dict | None` — returns a **copy** of the newest readable
  payload, or `None` when nothing was ever saved. A damaged live file is
  repaired from the backup (afterwards `<path>` itself is readable again). A
  leftover `<path>.tmp` is always discarded, never adopted. Raises
  `CheckpointError` when a checkpoint file exists but none is readable.

## `jobrunner.runner`

Durable state payload: `{"job_id", "completed": [names], "context": {},
"in_flight": name|None, "failure": {"step", "error"}|None}`.

- `JobReport` — `.status` in `{"completed", "failed"}`, `.completed`
  (cumulative names), `.executed` (actions run in *this* call), `.repaired`
  (in-flight steps this call resolved with a probe instead of re-running),
  `.skipped` (already complete when this call started), `.context`,
  `.failed_step`, `.error`.
- `JobRunner(job_id, steps, checkpoint_path, *, checkpoints=None)` — raises
  `JobError` for an empty `job_id` or invalid steps. `checkpoints`, when
  given, replaces the `CheckpointStore` opened on `checkpoint_path`.
- `state() -> dict` — the durable state, freshly loaded. Raises `JobError` if
  the checkpoint belongs to another `job_id` or names unknown steps, and
  `CheckpointError` if its shape is wrong.
- `run(fault_hook=None) -> JobReport` — for each step not already completed,
  in order:
  1. `BEFORE_INTENT`, then save the checkpoint with `in_flight` set to this
     step (**before** the action), then `AFTER_INTENT`.
  2. If this step was already in flight when the run started, call
     `probe(context)`: when it returns True the action is **not** run again,
     the step counts as `repaired`, and `AFTER_ACTION` does **not** fire.
     Otherwise run `action(context)`, merge its dict into the context, save
     the checkpoint again (still in flight) so the action's result is durable
     before anything can go wrong, then fire `AFTER_ACTION`.
  3. Save the checkpoint with the step appended to `completed`, `in_flight`
     back to `None`, and the accumulated **context**, then `AFTER_COMMIT`.

  An `InjectedFault` from any hook propagates out of `run()` untouched. Any
  other exception from an `action` is an ordinary step failure: record it in
  the checkpoint (`in_flight` cleared, `failure` set), fire `JOB_END`, and
  return a `"failed"` report naming the step; later steps do not run, and a
  later `run()` retries that step from the start. A step whose `action`
  returns a non-dict, or whose `probe` returns a non-bool, raises `JobError`.
  Running an already-completed job returns a `"completed"` report that
  executed nothing.
