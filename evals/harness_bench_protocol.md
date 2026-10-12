# Native and Aether harness pilot

This experiment compares the installed native Claude Code and Codex harnesses
with Aether's CLI-engine path, not with a hand-written API tool loop. Aether's
direct API model path is a different treatment and is not measured here: this
host currently has subscription CLI connections, not configured API providers.

## Treatments

The default matrix runs only the four single-agent treatments. The two
coordinator treatments below are opt-in experiments, not Aether default policy.
Single-agent Aether roots use the neutral `assistant` role and omit effort,
so the selected native harness retains its own default.
Matched opt-in lower-tier cells select the same named model and effort in the
native CLI and Aether single-agent paths. They exist for Sonnet, Claude Haiku 5.5
and GPT-6 Luna at low and medium effort; the lower-tier comparison uses Haiku 5.5
and GPT-6 Luna at low effort. Haiku is requested by its pinned id
`claude-haiku-5-5` on both sides (`claude-code/claude-haiku-5-5` in Aether),
never the floating `haiku` alias, so both CLIs resolve one model. Native-default
cells remain unchanged.

Both launch paths are given the same requested effort. Effective effort is
confirmed only where a trial's own session metadata reports it (the Codex
rollout `turn_context`); Claude Code receipts and Aether `engine.session` events
do not report it, so for those cells every report says "requested", not
"confirmed". The resolved model is captured in `resolved_metadata` on both sides.

- Native Claude, installed default model and effort, single-agent task instruction.
- Native Codex, installed default model and effort, single-agent task instruction.
- Aether single agent with `claude-code/default`.
- Aether single agent with `codex/default`.
- Aether default coordinator: create an unassigned, long-lived root and then
  deliver the task; allow it to choose whether delegation helps.
- Aether explicit team: coordinator plus two tracked Aether children. This is
  an intentionally different workflow, **not the default coordinator**.

The earlier pilot observed GPT-6 Astra / medium and Claude Fable 5.1 / high;
these are historical observations, not settings to assume for a new run.
Capture current configuration and runtime-resolved identities separately.
For Codex, read the named session's turn context for both native and
Aether-launched trials, preserving effective effort rather than inferring it
from an omitted or requested value. Unknown effort remains unknown.
Record differences rather than silently treating
different models, efforts, or context windows as matched controls. Team children
use the configured mix preset's Sonnet tier; team results therefore measure a
mixed-model workflow, not pure harness overhead.

## Protocol versions

A manifest records the version it was measured under as `protocol_version`;
one without the field is version 1. `harness_bench_compare.py` and
`harness_bench_report.py` refuse to read manifests of two versions together.

| Version | Change |
| ---: | --- |
| 1 | Every native Claude trial starts with `--strict-mcp-config`, so it loads no MCP server. An Aether Claude trial does not: beside Aether's own `aether` server it loads whatever MCP servers and connectors the machine user's Claude Code has. The two sides of a Claude pair start with unequal MCP sets. |
| 2 | Both sides of a Claude pair start without the machine user's MCP servers. The `claude` shim through which an Aether trial reaches the CLI (`harness_bench_receipts.py`) adds `--strict-mcp-config` beside Aether's own `--mcp-config`, so the session loads the `aether` server and nothing else. Aether itself is not changed. Each receipt records whether the flag was added (`strict_mcp_added`). |

Version 2 applies to every Aether trial that launches Claude Code, single
agent and coordinator alike. The lower-tier comparison published on 2026-10-10
(five runs) was measured under version 1, with unequal MCP sets; its figures
are not comparable with a version 2 run and are not merged with one. The
[second measurement](../docs/harness-lower-tier-results-2026-10-10-protocol-2.md)
of the same day (five runs) was made under version 2.

Codex has the same kind of gap in principle and no measured one. A native
Codex trial is started with `-c mcp_servers={}`; an Aether Codex turn is not
(`crates/aether-engine/src/codex_app.rs` clears the table for summary turns
only), so a machine user's Codex MCP servers would load through Aether only.
The host of the published runs configures none, so both sides loaded none
there, and version 2 changes nothing for Codex.

## Tasks and grading

Use fourteen deterministic, multi-module Python tasks: a durable idempotent ledger,
a dependency scheduler with retries, a five-module line protocol server with a
write-ahead log and compaction, a crash-resumable checkpointed job runner,
serializable transactions, resource leases, concurrent transactions, a frozen
harder version of the concurrent transaction case, a durable consumer-group
event log, a limit order book with a journal (`matchbook`), a spreadsheet
engine with formulas, recalculation and structural edits (`gridcalc`),
Decimal invoicing (`billing`), SemVer ranges with a backtracking resolver
(`semver`), and a tree-walking interpreter (`minilang`).
Each has an explicit public contract, buggy starter implementation, and public
tests. Independent grader
checks are not copied into solver workspaces. Reference implementations validate
the graders and are likewise not given to solvers. This is a procedural holdout,
not a claim of security isolation from a malicious solver on the same host.

Freeze the fixture and grading code before the first live trial. Never give
hidden-test feedback to subsequent solvers, repair a solver's answer manually,
or change a grader merely to make an observed solution pass. Record any genuine
grader correction and regrade every existing result consistently.

The counted grade is `grade` in a trial's record: all hidden checks of a case
run in one process under one 60 second limit, so a solution that hangs in one
check is graded 0. From 2026-10-11 a trial's record also holds
`grade_by_test`, a second field that is **not the counted grade** and enters
no table of the comparison. When the one process did not time out it repeats
the counted result. When it did, every hidden check is run again in a process
of its own under a 5 second limit, all of them under a 120 second limit, and
the field holds how many passed, failed, hung and were not run before the
total limit, with the ids of those that hung. A check can behave differently
alone than after the checks before it, so the two fields are not
interchangeable. `harness_bench_compare.py --trials` prints the field in a
column of its own when a listed trial has it; a manifest written before it
existed loads as before and gets no such column.

`not_run` can be non-zero. A check that hangs uses its whole 5 seconds, and
no check starts with less than 5 seconds of the 120 left, so a solution that
hangs in more than about 23 checks reaches the total limit before every
check has run. Such a record is a **lower bound on `passed`**, not the
solution's test-by-test grade: the checks not run are unknown, not failed.

The counted grade is made and validated first and does not depend on the
second field. If the test-by-test grading raises or its counts do not add up
to its total, `grade_by_test` is `null` and `grade_by_test_error` holds the
reason; `grade`, `grade_error` and the trial's status are what they would
have been without it, and the compare table reads `unknown` in that column.
If the counted grade itself fails, both grades are `null`, `grade_error`
holds the reason and no test-by-test grading is attempted.

### Lower-tier cases: validation, calibration pilot and freeze

`matchbook` and `gridcalc` were added for the Haiku 5.5 and GPT-6 Luna cells.
`test_harness_bench_new_cases.py` enforces every number in this table:

| Case | Hidden checks | Unmodified starter | Reference | Three repeated gradings | Empty and six constant implementations |
| --- | --- | --- | --- | --- | --- |
| `matchbook` | 119 | 23 pass, 96 fail | 119/119 | identical | 0/119 each |
| `gridcalc` | 154 | 5 pass, 149 fail | 154/154 | identical | 0/154 each |

Calibration pilot, 2026-10-10: one native low-effort trial per case and model,
written with `--pilot`. Pilot manifests are refused by both reports and are
never counted.

| Case as piloted | GPT-6 Luna, low | Claude Haiku 5.5, low (effort requested, not confirmed) |
| --- | --- | --- |
| `matchbook`, first version, 80 checks | 80/80, 72 s | 80/80, 240 s |
| `gridcalc`, 154 checks | 123/154 (79.9 %), 135 s | 154/154, 670 s |

What changed because of it:

- `matchbook` was saturated for both models, so the contract was extended with
  iceberg orders (`submit_iceberg`, `iceberg()`, the "Iceberg orders" section)
  and 39 checks were added in `hidden_tests/test_iceberg.py`. The 80 piloted
  checks are byte-for-byte unchanged. The extended case was not piloted again
  (one pilot per case and model is the limit), so its headroom is unknown
  until the counted run.
- `gridcalc` was not changed after its pilot. Each of Luna's failed checks was
  traced to an explicit contract rule (left-to-right argument errors, `ROUND`
  on the decimal representation, callback order on `copy`, `precedents` and
  `dependents` with ranges, `#REF!` in rewritten text, rejected names leaving
  the old content, range shrink on delete, formula text kept as typed). One
  Haiku trial passed every check: on this case the Haiku pair may sit at the
  accuracy ceiling, leaving tokens, cost and wall time to tell the cells apart.

Headroom carried into the counted run. This is a known limit, not a result:

- GPT-6 Luna has shown headroom on `gridcalc` only (123/154). On the extended
  `matchbook` its headroom is unknown.
- Claude Haiku 5.5 has shown no headroom on either case: it passed 154/154 on
  `gridcalc` and 80/80 on the first `matchbook`, and the iceberg extension was
  not piloted. The Haiku cells may sit at the accuracy ceiling on both cases.
- `gridcalc` could have been extended after its saturated Haiku pilot and was
  not: its one pilot per model was spent, so an extension could not have been
  checked for headroom before the freeze, and the Luna pair already had it.
- The counted run therefore reports every run's check count beside the mean.
  Where every run of both sides of a pair passes every check of a case,
  `harness_bench_compare.py` names the case as at the accuracy ceiling: it can
  show a loss but not a gain, so "no difference" means "no loss seen at the
  ceiling", and that pair is told apart by tokens, cost and wall time alone.
- A harder case added later is a new case version: validated, piloted once
  and frozen before any trial counts on it, and never pooled with these.

Counted run, 2026-10-10: five runs of the matched set, 40 trials, reported in
the [dated report](../docs/harness-lower-tier-results-2026-10-10.md) with the
counting rule that was written down before the first counted trial. Run 5 was
added after runs 1 to 4 had been seen, on a rule written down before it
started, and is counted whole. What the runs showed about headroom: no GPT-6
Luna trial passed every check on either case (best 144/154 and 118/119).
Claude Haiku 5.5 passed every check in 17 of its 20 trials, 9 of 10 native
and 8 of 10 through Aether, so the Haiku pair sat next to the ceiling on both
cases, as the pilot had suggested, and accuracy could hardly separate the
harnesses for it. The fixtures were not changed for or after those runs; the
digests below still hold.

Counted run under protocol version 2, 2026-10-10: five runs of the same
matched set, 40 trials, reported in the [version 2
report](../docs/harness-lower-tier-results-2026-10-10-protocol-2.md) with its
counting rule committed before the first run. The five runs were fixed in
advance; none was added, dropped or repeated. What the runs showed about
headroom: one GPT-6 Luna trial of 20 passed every check (Aether on
`matchbook`, 119/119); the best on `gridcalc` was 140/154. Claude Haiku 5.5
passed every check in 18 of its 20 trials, 10 of 10 native and 8 of 10
through Aether, so the Haiku pair sat at the ceiling again. Two Luna trials
on `matchbook`, one on each side, were graded 0 because a solution hung in
the grader's one process; while the grader ran each of them, available memory
on the host fell to about 1.1 GB. The fixtures are byte for byte those of the
first counted run.

Counted run under protocol version 2, second sitting, 2026-10-11: five more
runs of the same matched set, seeds 6 to 10, 40 trials, on the same Aether
binary, reported in the [report of
2026-10-11](../docs/harness-lower-tier-results-2026-10-11.md) with its
counting rule, the rule for the README's table and its statistics committed
before the first run. None was added, dropped or repeated. What the runs
showed about headroom: no GPT-6 Luna trial of 20 passed every check; the best
were 117/119 on `matchbook` and 140/154 on `gridcalc`. Claude Haiku 5.5 passed
every check in 14 of its 20 trials, 7 of 10 native and 7 of 10 through Aether,
and the other six passed 117 of 119 or 153 of 154, so the Haiku pair sat next
to the ceiling again. Three Luna trials on `matchbook`, 2 native and 1 through
Aether, were graded 0 because a solution hung in the grader's one process; the
runner now also grades such a trial test by test, as a second figure that is
never counted. The memory guard ended two processes that a native Luna solver
had started in run 9, and that trial is counted as it stands. Available memory
on the host did not fall below 5,015 MB. The fixtures are byte for byte those
of the first counted run.

No grader was changed to fail an observed correct solution or to pass an
observed wrong one. Frozen fixtures, as `fixture_digest()` computes them (one
sha256 over the starter and hidden-check hashes `benchmark_hashes()` records in
every manifest), asserted by `test_fixtures_are_frozen`:

| Case | Frozen digest |
| --- | --- |
| `matchbook` | `7f719f2d3e0b7c98228f9bdf9a922aea93b605cbec9cb3b46535fe66ab8d630a` |
| `gridcalc` | `df35116f6492541fbdc8078c26355647483ff321332b91bb7ab3c9c975dd4758` |

A change to either fixture is a new case version: revalidate it, update the
digest and discard results from the old version.

Code correctness and workflow completion are distinct: correct files left by
a timed-out or failed process do not count as a successfully finished workflow.
Preserve raw artifacts and record partial scores, failures, and timeouts.

### Six cases at medium effort: counted sittings

`billing`, `semver`, `minilang`, `concurrent_v2`, `eventlog` and `leases`, three
runs of the matched Haiku 5.5 and GPT-6 Luna medium-effort cells with seed 393
and a 900 s timeout: 72 trials a sitting. The three new cases (`billing`,
`semver`, `minilang`) were frozen before any counted trial and none of them
was piloted.

First sitting, 2026-10-10, under protocol version 1: reported in the [report
of 2026-10-10](../docs/harness-bench-2026-10-10.md).

Second sitting, 2026-10-11, under protocol version 2 with `--jobs 2`: reported
in the [report of 2026-10-11](../docs/harness-bench-2026-10-11.md) with its
counting rule and its statistics committed before the first trial. No trial
was added, dropped or repeated. What it showed about headroom: Claude Haiku
5.5 passed every check in 25 of its 36 trials, 13 of 18 native and 12 of 18
through Aether, and the other 11 each missed one check, so the Haiku pair sat
next to the ceiling. GPT-6 Luna passed every check in 3 of its 36 trials, 2
native and 1 through Aether. No grader hung and no trial was graded 0. One
Luna trial through Aether ended at the timeout, and the memory guard ended one
process that a Haiku solver had started through Aether; both trials are
counted as they stand. The fixtures of the six cases are byte for byte those
of the first sitting. The two sittings are reported side by side and never
pooled.

## Execution and accounting

Run a bounded pilot, initially one observation per task/treatment. Trials run
one at a time by default (`--jobs 1`); see "Concurrency" for bounded parallel
blocks. Use fresh fixture Git repositories
and isolated daemon stores per Aether trial. Pin the executable and record its
hash; keep workspace copies and scratch data under the repository's `target/`.
Measure daemon setup separately from solve time. Preserve the live daemon and
existing user files. A timeout must stop the trial's owned process tree.

### Concurrency

`--jobs N` (1 to 8) runs up to N trials at once in *blocks*. The seeded trial
order is cut into paired units: a case's `native-X` trial and its
`aether-single-X` trial of the same run are one unit and always share a block,
so both sides of a pair run under the same concurrency and the same host load.
A block is filled with whole units up to N trials and finishes completely
before the next block starts. Within a unit the launch order alternates with
the seed, so across cases and runs each side goes first half the time.
`--jobs 1` is exactly the sequential order described under "Interpretation".

Every trial still gets its own fresh workspace and its own daemon store, config
and socket; nothing is shared between concurrent trials and none of them can
reach the operator's daemon (the live routing variables are stripped from every
child environment). The daemon listens only on its Unix socket. When a trial
directory is too deep for a Unix socket path, the socket goes into a private
`0700` directory under `XDG_RUNTIME_DIR` that is removed with the trial. A
timeout stops only that trial's process group; an interrupted runner stops
every process group it owns and starts no new one.

The manifest records `jobs` and the block scheme, and each trial records its
block, the number of trials in it, its launch position, and the host load
average at its start and end (unknown stays `null`).

Wall time under `--jobs` above 1 is time under shared load. It is comparable
within a pair, because both sides ran in the same block, but it is not an
absolute latency and must not be compared with sequential results or across
different `--jobs` values. Token and cost measures do not depend on
concurrency. Keep `--jobs` at 4 or below on a shared 15 GB host, unless at
least 4 GB of memory is available just before the run and an earlier block
showed no memory pressure. The counted runs of 2026-10-10, under both
protocol versions, had that much available before every run and ran each
matched set of 8 trials at once, so a whole run shared one block. In runs 4
and 5 under version 2, available memory fell to about 1.1 GB for under a
minute while the grader ran a solution that hung; count that as memory
pressure before choosing `--jobs 8` again.

### Usage capture

Capture raw native receipts and Aether durable events for the entire run tree.
Count every descendant once, including failed or retried work. Do not sum
cumulative snapshots, double-count cached input or reasoning output, or convert
missing telemetry to zero. Prefer lifecycle-validated terminal usage. Report
coverage and attribution limits explicitly.

Claude `modelUsage` includes auxiliary models that its primary `usage` may omit.
The Aether adapter may expose only the latter in durable statistics. Reconcile
native receipts against durable statistics separately; never add the two views
together. Tool calls and turns have different native/runtime meanings and need
labelled definitions, not an unqualified cross-harness ratio.

Uncached input, output, and API-equivalent USD lead the cost-first report;
cache reads/writes remain separate columns. Dollar values are not subscription
invoices or measured quota decrements. Each row shows billed / estimated / mixed
basis and billed / estimated / unpriced unit counts. “Billed” means reported
dollar metadata. Partial observed dollar floors remain visible but cost means
include only fully priced terminal trials, with the priced/total denominator.

Runtime `estimated_cost_usd` is retained alongside any billed component when
`cost_complete` is false. That estimate is itself a floor: the runtime has no
API twin for every model and does not report whether its estimate is complete,
so an estimated turn is a lower bound, not a settled receipt. Otherwise-unpriced turns use their `engine.session`
model and the pinned binary's captured `list_models` inventory. `price_table`
records that binary's SHA-256 and model prices: input, output, cache read, and
cache write USD per million tokens, following `ModelCost`. Capture uses an owned
isolated daemon, never the operator's socket. Engine providers map from
`claude-code` to `anthropic` and `codex` to `openai`; exact API IDs take precedence,
then explicit inventory aliases or unique engine/API display-name matches.
Unresolved aliases and turns with multiple session models stay unpriced rather
than guessing an allocation. Current inventories do not expose every alias.
When a list-price estimate replaces a partial billed floor (`cost_complete`
false, no runtime estimate), the floor stays visible as `billed_floor_usd`.

Native Codex prefers the model the trial itself resolved, recorded in the row's
`resolved_metadata` (`native rollout turn_context`), with basis `estimated
(resolved model)`. Only when no resolved model was captured does it fall back to
the manifest's configured default, with basis `estimated (configured default
model)`; that fallback is not proof of the runtime model. Several distinct
resolved models in one trial cannot be allocated across turns and stay unpriced.
Its inclusive input includes cache reads/writes; subtract these before pricing
uncached input. This subtraction assumes `cached_input_tokens` and
`cache_write_input_tokens` are disjoint subsets of `input_tokens`; observed
captures so far all report zero writes, so the assumption is untested against a
real nonzero write. Missing price-relevant counters stay unknown (a zero-rate
cache field needs no counter). Unknown models remain unknown. Native Claude
retains its reported all-model receipt; overlapping durable/native views are not
added.

`harness_bench_reprice.py MANIFEST` rebuilds all trial usage reports from saved
`events.json` or native JSONL and the captured price table, writing a new
`MANIFEST.repriced.json` (replacing the `.json` suffix). `--price-table` supplies
a saved inventory for old manifests; `--binary` plus `--scratch-dir` fetches one
from the matching pinned executable in isolation and removes that scratch
directory once the capture succeeds, keeping it only to diagnose a failure.
`--scratch-dir` without `--binary` is rejected rather than ignored.
`--output-dir` redirects the new manifest away from read-only evidence.
Originals and trial files are never modified, and an existing output is never
overwritten. Missing captures produce explicit usage errors and a nonzero CLI
exit, not stale successful accounting. Receipt reconciliation is reported
separately as `claude_reconciliation_error`, and stale accounting keys from the
source manifest are always cleared before the rebuild writes its own.

Benchmark construction and this interactive coordinator's own usage are study
overhead, not solver-trial cost.

## Interpretation and improvement work

One run per task is diagnostic, not a reliable accuracy or latency distribution.
Report per-task pairs and sample counts; do not infer a universal winner from
aggregate tokens. Cache warmth, provider load, native user settings, model
selection, and extra coordination instructions remain possible confounds.
Run order is counterbalanced: `--runs N` schedules every case×condition cell
once per run before any cell repeats, and the condition order rotates per
case and per run so no condition always follows a warm cache or a loaded host.
`--seed` selects a reproducible initial permutation of cases and conditions
(the default zero preserves the listed order). For a two-condition, one-case
comparison, each run is a paired block and the first condition alternates;
three runs give one condition two first positions and the other one. The
manifest records the `ordering` scheme, seed, and each trial's `sequence`
number. Aether rows also include counts by durable event type, while the raw
event log remains the source for trace inspection.
Historical `.lavish/` experiments are separate evidence, never pooled controls.

`harness_bench_compare.py MANIFEST [MANIFEST ...]` prints one Markdown table per
matched model pair, native and Aether side by side: trials, workflows finished,
hidden checks passed (mean fraction and every run's value), uncached input,
output and cache tokens, API-equivalent USD with its basis and priced/total
count, and wall seconds (mean and range), each with the paired difference
(Aether minus native) and the number of pairs it rests on. A pair is the two
sides of one case and run in one manifest: trials from different manifests are
never paired, so a native block and an Aether block run separately give "0
pairs". Run both sides of a pair in one block. A measure a trial did not report
is left out of that mean and shown in the measured/total count; an unpriced
cell reads "unpriced", never zero. A token or cost figure counts as measured
only when every unit of its accounting scope reported it at a terminal point
(the field's `coverage.complete`); a timed-out trial, or an Aether Claude trial
without its process receipts, yields a lower bound that enters no mean and no
paired difference, is counted in "Partial-receipt trials" and is listed
with its floor under the table. Each pair names the accounting scope of both
sides. Blocked trials are excluded and counted. The
report makes no significance claim. It refuses a `--pilot` manifest, a
diagnostic manifest (below) and overlapping manifests. `--trials` appends
one line per trial, naming each
manifest by its file and directory only, for a dated report whose manifests
stay local: a lower bound reads "at least", a trial with tokens but no
price reads "unpriced", and an ungraded trial reads "unknown".

### Overhead diagnosis arms

When a counted pair differs, the cause is located with diagnostic arms, each
one change away from its neighbour. They are conditions named `diag-...`:

| Condition | Differs from | In |
| --- | --- | --- |
| `diag-native-haiku-low-envelope` | `native-haiku-low` | the task is sent inside the first-turn envelope an Aether root agent receives (same words; a fixed nonce and task id; the trial's own workspace path), with "Effort low." |
| `diag-native-haiku-low-envelope-noeffort` | `diag-native-haiku-low-envelope` | the words "Effort low." are left out of the envelope; the `--effort low` flag stays |
| `diag-aether-haiku-low-minimal` | `aether-single-haiku-low` | the agent's Aether tools are limited to `complete_task` and `fail_task` |
| `diag-native-haiku-low-nocoord` | `native-haiku-low` | the CLI is started with `--disallowedTools` and the list of its own coordination tools that Aether passes (`NATIVE_COORDINATION_TOOLS`) |
| `diag-native-haiku-low-nocoord-connectors` | `diag-native-haiku-low-nocoord` | `--strict-mcp-config` is left out, so the machine user's MCP servers and connectors load |
| `diag-aether-haiku-low-connectors` | `aether-single-haiku-low` | the shim does not add `--strict-mcp-config`, so the machine user's MCP servers and connectors load: the counted Aether cell as it started under protocol version 1 |
| `diag-aether-haiku-low-complete-task` | `aether-single-haiku-low` | the task ends with one more sentence asking the solver to finish by calling the Aether tool `complete_task`: a check that Aether's own MCP server answers a call under `--strict-mcp-config` |

Both envelope arms also pass `--` before the prompt, because the envelope
opens with dashes; this changes the command line and not what the model is
sent. `aether-solo-minimal-luna-low` is the same tool limit for Luna. The two
`connectors` arms depend on the machine they run on: what they load is the
machine user's own configuration, which is why no counted cell loads it.

`diag-native-luna-low-repeat` differs from `native-luna-low` in nothing: it
is the counted native Luna cell a second time in the same block, and its
command line is held equal to that cell's by a test. The difference between
the two is the yardstick an Aether difference has to exceed before it is
read as one.

Diagnostic trials are never counted. A `diag-` condition runs only with
`--diagnostic`, which marks the manifest; the flag is also given when counted
cells are run as the reference arms of a diagnosis. `harness_bench_compare.py`
and `harness_bench_report.py` refuse a manifest that carries the mark or any
`diag-` trial, as they refuse a pilot. No `diag-` condition is in the default
set or among the lower-tier cells. The figures a diagnosis needs beyond the
manifest are in the solver's own session transcript:
`harness_bench_transcript.py` reads them per trial (first-request input,
thinking or reasoning before the first edit and in the largest single
response, requests, tool calls, test runs, the tools Claude Code loaded and
the context Codex was given). It reads the session logs and changes nothing.

Use failures and event traces to identify candidate improvements: repeated reads
and tests, duplicated coordinator/worker implementation, post-wait tool calls,
stale background jobs, context growth, and incomplete usage attribution. Separate
observed waste from hypothetical savings. Any production policy change requires
its own regression tests and matched rerun; this pilot does not silently alter
the harness under measurement.
