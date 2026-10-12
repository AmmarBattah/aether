# Harness overhead diagnosis: Claude Haiku 5.5 and GPT-6 Luna, 2026-10-10

The [lower-tier comparison](harness-lower-tier-results-2026-10-10.md) found
that Claude Haiku 5.5 spends less through Aether than in Claude Code, and that
GPT-6 Luna spends more through Aether than in Codex. This report looks for the
cause of each with diagnostic arms, every arm one change away from its
neighbour, on one case (`matchbook`) in 6 blocks per model. It changes nothing
in Aether. Its 48 trials are diagnostic: all of them are reported here, none
is counted, and the comparison and report tools refuse their manifests.

A second iteration followed on the same day and is reported in
[Iteration 2](#iteration-2): it removes the repeated effort words from
Aether, makes the benchmark start both sides of a Claude pair with the same
MCP set (protocol version 2), and tests again with 60 more diagnostic trials.
The sections between the summary and that one are iteration 1 as it was
written.

Two more followed. [Iteration 3](#iteration-3) is the counted test made
again under protocol version 2, reported in its own document.
[Iteration 4](#iteration-4) looks for the cause of Luna's overhead by
experiment, with six arms on `matchbook` and `gridcalc` and 72 more
diagnostic trials, one arm on a diagnostic binary that is not committed.

[Iteration 5](#iteration-5) asks whether Luna through Aether differs from
Luna in Codex by more than Codex differs from itself, with three arms, one
of them the native cell a second time, and a rule for a claim written
before its first trial.

## Summary

### Where the loop stands after five iterations

| | Haiku | Luna |
| --- | --- | --- |
| Located in iteration 1 | the difference sits in one response; the words "Effort low." in Aether's first message likely account for part of it, about half in the mean of 6 blocks | no overhead per trial; a fixed 939 input tokens per request; the counted gap is one long trial |
| Fixed | Aether no longer repeats the effort to an engine that is already given it (`crates/aether-context/src/render.rs`); the benchmark starts both sides with the same MCP set (protocol version 2) | nothing to fix |
| Retest, matched pair | output tokens 83,656 through Aether against 75,794 natively (+10.4 %), lower in 3 of 6 pairs and higher in 3; hidden checks 119.0 against 118.3 | output tokens 10,972 against 12,048 (-8.9 %), lower in 6 of 10 pairs; hidden checks 79.0 against 88.0, lower in 7 of 10 |
| Stop rule | **not met**: not one-sided on any measure, but the mean difference is outside 5 % on output tokens and cost | **not met**: not one-sided on any measure, but the mean difference is outside 5 % on all three |
| Still open after iteration 2 | whether the 10 % is real: 6 pairs cannot place a mean within 5 % | whether full-tool Aether passes fewer hidden checks than Codex: 12 of 16 blocks, with trials at 0 in every arm |
| Iteration 3, the counted test again | stop rule **not met**: output tokens lower through Aether in 5 of 10 pairs and higher in 5, the mean differences outside 5 % | stop rule **not met**: no measure one-sided, the mean differences outside 5 % against Aether (output +32.3 %, uncached input +43.9 %, USD +51.9 %) |
| Iteration 4, the cause by experiment | not run | **not located**: in 12 new pairs Aether is lower in 6 and higher in 6 on uncached input, output tokens and cost, and a native cell run twice differs by as much |
| Iteration 4, the approval policy | does not apply | no approval or escalation request in any of 38 Aether sessions; taking the policy away through Aether did not lower spend (more requests in 10 of 12 pairs, higher cost in 9) |
| Still open after iteration 4 | unchanged by iteration 4 | whether the `on-request` permission text shortens Luna's run and costs hidden checks: a hypothesis from one count on 12 pairs |
| Iteration 5, a larger matched sample against a yardstick | not run | **no effect claimed**: in 24 new pairs no measure meets the rule written before the first trial; over both cases every interval of Aether minus Codex includes zero, and so does every interval of Codex against itself |
| Still open after iteration 5 | whether the 10 % is real | an effect inside the intervals: on cost from 26 % below native to 172 % above, on hidden checks test by test from 19 % below to 2 % above; and whether Aether passes fewer hidden checks on `gridcalc` (lower in 15 of 18 pairs over two sittings, not claimed) |

**Haiku.** The saving the comparison reported had two located parts, and
both are now gone from the matched pair.

- **The words.** With the connectors loading as before, Aether without the
  words is 8,617 thinking tokens below native, where it was 17,077 below
  with them. That is read across two sittings an hour apart, in which the
  native cell itself moved by 4,300.
- **The connectors.** This is likely the half iteration 1 could not place:
  the sizes agree, and 6 blocks do not pin them down. Loading the operator's
  MCP connectors lowers Haiku's output by 18,586 tokens through Aether (6 of
  6 blocks) and by 16,966 natively (5 of 6). Under protocol version 1 only
  the Aether side loaded them.
- **Not the coordination tools.** Removing the CLI's own coordination tools
  natively, as Aether does, did not lower thinking (+7,331 tokens, 3 blocks
  each way).
- **What is left** is a matched pair at 3 blocks each way, with Aether
  7,863 output tokens above native in the mean and one block carrying that
  mean. Hidden checks are at the ceiling in every arm.

**Luna.** Tokens, cost and time again show nothing consistent: the pair is
not one-sided on any measure, and its means are 9 % to 25 % apart in both
directions, following how long a run goes.

- **Trials at 0 are in every arm**: 2 of 10 each for Codex, full-tool
  Aether and two-tool Aether. Five of the six are one failure, a loop in
  iceberg matching that hangs a few hidden tests; the grader runs all 119
  in one process and gives 0. Two of the five are native.
- **Iteration 1's open question is not supported**: the two-tool cell no
  longer beats the full-tool one (5 blocks each way).
- **A smaller one is left**: full-tool Aether is below Codex in 12 of 16
  blocks over both iterations, and the two-tool cell is not (7 of 16).
- **No Aether tool was called in any Luna trial**, so none ended early
  through `complete_task`.

**Next, for iteration 3.** No further change to Aether is supported. The
concrete step is in the benchmark: at least 12 blocks of the Haiku matched
pair with the removed coordination tools split into the four the CLI
always loads and the six it defers (`evals/harness_bench_treatments.py`,
`diagnostic_conditions`, lines 65 to 72), and for Luna a per-test count
beside the counted grade (`evals/harness_bench_cases.py`, `grade`, lines
287 to 316). Details are under
[What iteration 2 points to](#what-iteration-2-points-to).

**Luna, after iteration 4.** The cause of Luna spending more through
Aether is not located, and the sitting that looked for it did not show the
overhead.

- **The matched pair is level**: lower through Aether in 6 of 12 pairs and
  higher in 6 on uncached input, output tokens and cost. Two native cells
  that the model cannot tell apart differ by as much.
- **The approval policy is the largest thing Aether adds to the first
  turn**, 5,782 characters, and it is never acted on: no request for
  approval in 38 Aether sessions. Without it the solver made more requests
  in 10 of 12 pairs and cost more in 9. It is not a saving, and it points
  the other way from what was expected.
- **`codex exec` cannot run under Aether's policy**, so the arm built to
  test it natively measures a second native cell.
- **The multi-agent setting and the envelope are not one-sided** natively.
- **No change to Aether is proposed.** The options for the approval
  policy, with the file, the lines and what each costs in function, are
  under [What iteration 4 points to](#what-iteration-4-points-to).

**Luna, after iteration 5.** The loop found and fixed the Haiku cause. For
Luna it found none: with 24 matched pairs and the native cell run twice in
every block, Luna through Aether is not distinguishable from Luna in Codex
at this sample size, on any of eight measures.

- **The rule, written before the first trial, claims nothing.** An effect
  needs an interval of Aether minus Codex that excludes zero, a yardstick
  interval that includes it, and the same direction in both cases. Every
  interval of Aether minus Codex includes zero: cost +0.0140 USD (-0.0069
  to +0.0458), uncached input +15,176 tokens (-9,096 to +48,425), hidden
  checks as counted -10.6 (-33.6 to +12.3) and test by test -8.6 (-22.0 to
  +2.3).
- **Codex differs from itself by as much.** The identical native command
  run twice in one block differs by a median of 48 % of the native mean
  cost, and by 0 to 119 hidden checks as counted.
- **What remains possible** is an effect inside those intervals: on cost
  from 26 % below native to 172 % above, and from 35 % below to 32 % above
  once the block of one stopped Aether trial is left out; on hidden checks
  test by test from 19 % below native to 2 % above.
- **Against Aether, and not claimed:** hidden checks lean below native in
  both cases, and on `gridcalc` they are lower in 15 of 18 pairs over
  iterations 4 and 5 together. The native repeat leans below the native
  cell as well. Aether's first request carries 923 more input tokens in 24
  of 24 pairs; that part is measured and fixed in size.
- **A trial also records a test-by-test grade now**, beside the counted
  one, which is unchanged. The test-by-test figures here are the counts the
  trials recorded, as planned. One of the 72 grades was cut by its time
  limit, so its count is a lower bound; what it reads when finished, and
  which way that moves the result, is under
  [The cut grade, as planned and finished](#the-cut-grade-as-planned-and-finished).

The published five-run figures were measured under protocol version 1 and
are unchanged. All 108 trials of the first two iterations, the 72 of the
fourth and the 72 of the fifth are diagnostic and none is counted; the 40
of the third are counted and are reported in their own document.

### Iteration 1

**Haiku: the difference is located in one response, and two words likely
account for part of it.** In all 30 trials the solver read the workspace in
two requests and then did between 86 % and 100 % of its thinking in a
single response. That response is smaller through Aether: 41,484 thinking
tokens in the mean against 55,853 natively, lower in 6 of 6 blocks. Output
tokens follow (59,442 against 81,209, 6 of 6) and hidden checks do not move
(118.7 against 119.0 of 119).

- Aether's first message ends with "Turn 1. Effort low." Sent natively in
  the same envelope, with and without those two words and with the same
  `--effort low` flag, the words alone take 10,072 thinking tokens off a
  trial in the mean (6 of 6 blocks) and 12,983 output tokens (6 of 6). The
  gap between the counted cells is 17,077 and 21,767. That the words lower
  Haiku's thinking is likely; that they account for half the gap rather
  than a third or two thirds is not shown by 6 blocks.
- The envelope without the words changes nothing that shows.
- With the first message equal on both sides, Aether is still lower by 9,093
  thinking tokens (6 of 6). That half is not Aether's tool list: limiting the
  agent to two tools changes thinking by +950 tokens (3 blocks lower, 3
  higher). What is left is the four orchestration tools the native CLI loads
  and Aether removes, and the operator's connectors, which load through
  Aether only. No arm separates those two; that part is a hypothesis.

**Luna: there is no overhead per trial to locate.** Through Aether a Luna
request carries 939 more input tokens (6 of 6 blocks, about 6 %), from a
longer approval text and skill listing. Beyond that the Aether cell used
fewer input tokens than the native one in 3 blocks and more in 3, and cost
less in 3 and more in 3. Limiting Aether's tools changes the first request by
3 tokens. Read again from the session logs, the counted Luna difference is
one trial: a single `matchbook` pair in run 4, where the Aether trial made
145 requests and the native one 16, is larger than the whole published gap in
cache read tokens, cost and wall time. Whether such long runs are more likely
through Aether cannot be told from 6 blocks, or from the 16 counted trials
that could be read again.

One Luna result is left open. The counted Aether cell passed fewer hidden
checks than the native one in 5 of 6 blocks (62.8 against 104.8 of 119 in the
mean, with two trials at 0 whose solutions hang), while the Aether arm with
two tools passed 102.3. In the five counted `matchbook` pairs the same Aether
cell passed more checks than native in 3. Six blocks do not settle whether
Aether's full tool list costs Luna checks.

**No repair in Aether was proposed on this evidence.** Iteration 1 judged
the words "Effort low." worth keeping; iteration 2 removed them for engine
turns, because they made an agent work below the effort it was asked for.
The benchmark started the two sides unequal in one way that is not the
treatment: the native Claude trial was kept from the operator's MCP
connectors and the Aether trial was not. One line in the benchmark would
equalise it, under a new protocol version. Iteration 2 added it.

## The arms

All arms run the frozen case `matchbook` at requested effort low.

| Arm | Condition | Differs from | In |
| --- | --- | --- | --- |
| H-N0 | `native-haiku-low` | | the counted native cell: `claude -p --model claude-haiku-5-5 --effort low` |
| H-N1 | `diag-native-haiku-low-envelope` | H-N0 | the task is sent inside the first-turn envelope an Aether root agent receives, with the words "Effort low." |
| H-N2 | `diag-native-haiku-low-envelope-noeffort` | H-N1 | the words "Effort low." are left out of the envelope; the `--effort low` flag stays |
| H-A0 | `aether-single-haiku-low` | | the counted Aether cell: one agent on `claude-code/claude-haiku-5-5` |
| H-A1 | `diag-aether-haiku-low-minimal` | H-A0 | the agent's Aether tools are limited to `complete_task` and `fail_task` |
| L-N0 | `native-luna-low` | | the counted native cell: `codex exec --model gpt-6-luna -c model_reasoning_effort="low"` |
| L-A0 | `aether-single-luna-low` | | the counted Aether cell: one agent on `codex/gpt-6-luna` |
| L-A1 | `aether-solo-minimal-luna-low` | L-A0 | the agent's Aether tools are limited to `complete_task` and `fail_task` |

Where an arm is not exactly its one change:

- **H-A1 limits the tools and nothing else. The operator's own MCP connectors
  still load.** Aether keeps them out of a Claude session only for an agent
  without network access, and that setting changes other things at once (the
  CLI's sandbox is switched on, its setting sources are emptied, hooks are
  off, the web tools are removed and denied, and the envelope says "network
  denied"). So
  of the two things the minimal arm could have removed, it removes the tools.
  The transcripts confirm it: H-A1 has the same 67 deferred tools and the same
  3,719 characters of MCP server instructions as H-A0.
- **The envelope is a fixture, not Aether's output.** It has the words of a
  real first turn, checked byte for byte against one from an Aether trial of
  the same cell, with two substitutions: the nonce in the two marker lines is
  `00000000` and the task id is `tsk_00000000000000000000000000`, where
  Aether draws both at random. The workspace path is the trial's own, as
  Aether prints it. The envelope adds 704 characters to the 789 of the task.
- **Both envelope arms pass `--` before the prompt.** The envelope opens with
  dashes, and without `--` the CLI reads it as an option. This changes the
  command line and not what the model is sent.
- **The Aether arms are not the native arm plus a wrapper.** What else
  differs between them is measured under each model below.

## How it was run

| | |
| --- | --- |
| Aether binary | `aether 0.1.1`, `cargo build --release`, SHA-256 `d687547b7c7b55a770927af54ef434a1fe25e004e488a31c3dd762b771608512`, not installed; every trial ran on its own isolated daemon |
| Built from | commit `fe661a7e`, the head of the comparison's branch before this work. The commits of this work change no Rust source, and the SHA-256 equals that of the binary of the counted runs |
| Benchmark code | Luna: commit `ee33f7d5`. Haiku: commit `a9e825a7`, which adds only the `--` before an enveloped prompt. Each batch ran from a frozen copy of that commit, so later commits could not change a running batch; the manifests record the commit with a clean tree |
| Native CLIs | `claude` 2.1.294 (Claude Code), `codex-cli` 0.162.1, as in the counted runs |
| Case | `matchbook`, 119 hidden checks, digest `7f719f2d3e0b7c98228f9bdf9a922aea93b605cbec9cb3b46535fe66ab8d630a`, recomputed before the first trial and equal to the frozen value (`gridcalc` likewise: `df35116f6492541fbdc8078c26355647483ff321332b91bb7ab3c9c975dd4758`) |
| Blocks | 6 per model, fixed before the first trial and all run. A block is one runner invocation with `--runs 1`: every arm of the model once, all started together |
| Seeds | 1 to 6, one per block; the seed sets the launch order inside the block |
| Trials at once | Haiku `--jobs 5`, Luna `--jobs 3`: a block's arms ran under the same load |
| Timeout | `--timeout 1200` seconds; no trial reached it (longest: 436.7 seconds, H-N0 in block 4) |
| Flag | `--diagnostic` on every invocation, recorded in every manifest |
| Trials | Haiku 30 (5 arms, 6 blocks), Luna 18 (3 arms, 6 blocks) |

Before every block the available memory and the free space on the home
volume were read and required to be at least 3 GB and 15 GB:

| Block | Luna start (UTC) | Luna end | Available memory (MB) | Free disk (GB) | Haiku start (UTC) | Haiku end | Available memory (MB) | Free disk (GB) |
| ---: | --- | --- | ---: | ---: | --- | --- | ---: | ---: |
| 1 | 18:46:27 | 18:49:37 | 6,810 | 39 | 18:50:26 | 18:57:28 | 8,206 | 40 |
| 2 | 18:49:37 | 18:52:24 | 8,850 | 39 | 18:57:28 | 19:02:51 | 7,996 | 40 |
| 3 | 18:52:24 | 18:56:46 | 7,282 | 40 | 19:02:51 | 19:09:14 | 7,958 | 39 |
| 4 | 18:56:46 | 18:58:35 | 8,340 | 40 | 19:09:14 | 19:16:32 | 7,861 | 39 |
| 5 | 18:58:35 | 19:00:37 | 7,579 | 40 | 19:16:32 | 19:23:24 | 7,665 | 39 |
| 6 | 19:00:37 | 19:04:23 | 7,373 | 39 | 19:23:24 | 19:29:21 | 7,479 | 39 |

Three things about the runs that a reader should know:

- **The two batches overlapped.** The Luna blocks ran from 18:46 to 19:04 and
  the Haiku blocks from 18:50 to 19:29, so up to 8 trials ran at once. The arms of
  one block still shared their load, and blocks are only compared within
  themselves.
- **A first Haiku launch was aborted and is not in any figure.** Its two
  envelope trials failed at launch in 0.2 seconds, before any model call,
  because the CLI read the envelope's leading dashes as an option. The block
  was stopped, the `--` was added (commit `a9e825a7`) and all 6 Haiku blocks
  were run again from the start. The aborted manifest is kept with the raw
  data.
- **The Haiku manifests were re-priced offline.** The runner filed the two
  native envelope arms under Aether accounting because their names do not
  start with `native-`, and so left their usage empty. The fix (commit
  `7f238f53`) classifies a trial by its treatment. The 6 Haiku manifests were
  then rebuilt from the raw captures each trial had saved, with
  `harness_bench_reprice.py`, into separate files. In them only the 12
  envelope trials changed; the other 18 are identical field for field. Both
  sets are kept. The figures below are from the re-priced files.

Token, cost, time and hidden-check figures are from the manifests. The
figures a manifest does not hold (first-request input, thinking, requests,
tools, test runs) are read from each solver's own session transcript by
`evals/harness_bench_transcript.py`. For every trial the reader's sums of
output, cache read and cache write tokens equal the manifest's.

## Haiku

### What the first request carried

Claude Code records in the session transcript the system prompt and the
tools it loaded with their schemas, and names the tools it only listed for
later loading ("deferred").

**What the first request carried (as the CLI recorded it; one value when every trial agrees)**

| Arm | Always-loaded tools | Their schema, characters | System prompt, characters | Deferred tools named | MCP server instructions, characters | First user message, characters |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 11 | 43,600 | 9,851 | 15 | unknown | 789 |
| H-N1 | 11 | 43,600 | 9,851 | 15 | unknown | 1,493 |
| H-N2 | 11 | 43,600 | 9,851 | 15 | unknown | 1,481 |
| H-A0 | 27 | 43,321 | 9,851 | 67 | 3,719 | 1,493 |
| H-A1 | 10 | 21,307 | 9,851 | 67 | 3,719 | 1,493 |

- H-N1 against H-N0: has nothing more; lacks nothing.
- H-N2 against H-N0: has nothing more; lacks nothing.
- H-A0 against H-N0: has `mcp__aether__ask_user`, `mcp__aether__cancel_agent`, `mcp__aether__complete_task`, `mcp__aether__create_schedule`, `mcp__aether__delegate`, `mcp__aether__fail_task`, `mcp__aether__inspect_state`, `mcp__aether__integrate_workspace`, `mcp__aether__list_agents`, `mcp__aether__list_models`, `mcp__aether__list_schedules`, `mcp__aether__list_tasks`, `mcp__aether__permission`, `mcp__aether__read_artifact`, `mcp__aether__read_task`, `mcp__aether__remember`, `mcp__aether__request_checkpoint`, `mcp__aether__send_message`, `mcp__aether__update_schedule`, `mcp__aether__wait_for`; lacks `Agent`, `ListAgents`, `ScheduleWakeup`, `Workflow`.
- H-A1 against H-N0: has `mcp__aether__complete_task`, `mcp__aether__fail_task`, `mcp__aether__permission`; lacks `Agent`, `ListAgents`, `ScheduleWakeup`, `Workflow`.
- H-N0 always loads `Agent`, `Bash`, `Edit`, `ListAgents`, `Read`, `ReportFindings`, `ScheduleWakeup`, `Skill`, `ToolSearch`, `Workflow`, `Write`.

"unknown" for the native arms means the transcript holds no such record: a
native trial runs with `--strict-mcp-config` and loads no MCP server.

Read against the Aether source:

- **The system prompt is the same in all five arms.** Aether appends nothing
  to it for an agent on the Claude Code engine. Its 9,851 characters are the
  same in all 30 trials but for the path of the per-directory memory folder.
- **Four native tools are gone and Aether's are there instead.** Aether
  starts the CLI with `--disallowedTools` and a list of the CLI's own
  coordination tools (`crates/aether-engine/src/claude_code.rs`,
  `NATIVE_COORDINATION_TOOLS`, line 21, passed at line 122). Of that list the
  native CLI loads four in this setup: `Agent` (5,492 characters of schema),
  `ListAgents` (2,081), `ScheduleWakeup` (8,096) and `Workflow` (9,138),
  24,807 characters together. The native transcript also lists 5 agent types
  for `Agent`; the Aether one lists none. In their place H-A0 loads 20
  `mcp__aether__*` tools (about 24,500 characters), so the two schemas are
  within 279 characters of each other. H-A1 loads 3 of them (about 2,500).
- **The operator's connectors load through Aether only.** The 67 deferred
  tools of an Aether trial are 13 of the CLI's own and 54 from two MCP
  servers of the operator's account, with 3,719 characters of their
  instructions. A native trial names 15 deferred tools, all the CLI's own.
  Aether passes `--strict-mcp-config` only for an agent without network
  access (lines 145 to 151); the benchmark passes it on every native trial.
- **The first user message** is the task (789 characters) natively and the
  task in the envelope (1,493) through Aether. H-N1 sends the same 1,493
  characters and H-N2 the same less the 12 of " Effort low.".

In tokens, on the first request: the envelope is +321 (H-N1 against H-N0,
+316 to +324 in 6 of 6 blocks); the words "Effort low." are 4 of those; the
Aether session is +3,322 on top of the envelope (H-A0 against H-N1), which is
the connectors' names and instructions net of the agent listing, not split
further by any arm; and Aether's 17 other tools are 5,232 (H-A0 against
H-A1). With them limited, the Aether agent's first request is 1,589 tokens
smaller than the native one.

### Per arm, every trial

"unknown" under thinking before the first edit means the solver made no
file-editing call in that trial: it wrote its files from shell commands.

**First-request input tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 23,106 | 23,106 | 23,100 | 23,106 | 23,108 | 23,102 | 23,104.7 (6) |
| H-N1 | 23,426 | 23,430 | 23,418 | 23,422 | 23,430 | 23,426 | 23,425.3 (6) |
| H-N2 | 23,425 | 23,429 | 23,417 | 23,417 | 23,421 | 23,421 | 23,421.7 (6) |
| H-A0 | 26,749 | 26,747 | 26,756 | 26,744 | 26,750 | 26,739 | 26,747.5 (6) |
| H-A1 | 21,512 | 21,517 | 21,510 | 21,531 | 21,510 | 21,513 | 21,515.5 (6) |

**Thinking tokens before the first edit**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 70,374 | 43,543 | 51,055 | 74,421 | 50,155 | 45,809 | 55,892.8 (6) |
| H-N1 | 53,042 | 52,294 | 38,369 | 50,687 | 45,811 | 49,533 | 48,289.3 (6) |
| H-N2 | 52,042 | 54,232 | 65,244 | 52,911 | 64,700 | 54,141 | 57,211.7 (6) |
| H-A0 | 40,358 | 40,737 | 34,858 | 43,459 | 45,588 | unknown | 41,000.0 (5) |
| H-A1 | 38,004 | 36,761 | unknown | 43,496 | 48,419 | 44,805 | 42,297.0 (5) |

**Thinking tokens, total**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 74,728 | 45,879 | 53,080 | 78,783 | 52,901 | 51,084 | 59,409.2 (6) |
| H-N1 | 54,676 | 55,306 | 40,910 | 51,571 | 49,571 | 56,515 | 51,424.8 (6) |
| H-N2 | 54,814 | 57,059 | 68,941 | 55,797 | 69,735 | 62,635 | 61,496.8 (6) |
| H-A0 | 41,234 | 41,502 | 36,078 | 44,095 | 45,835 | 45,247 | 42,331.8 (6) |
| H-A1 | 38,158 | 38,979 | 43,439 | 45,212 | 48,781 | 45,120 | 43,281.5 (6) |

**Thinking tokens of the largest single response**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 70,336 | 43,471 | 51,028 | 74,380 | 50,125 | 45,780 | 55,853.3 (6) |
| H-N1 | 53,005 | 52,280 | 38,329 | 50,673 | 45,797 | 49,493 | 48,262.8 (6) |
| H-N2 | 52,014 | 54,208 | 65,202 | 52,872 | 64,658 | 54,102 | 57,176.0 (6) |
| H-A0 | 40,281 | 40,702 | 34,858 | 43,445 | 45,588 | 44,029 | 41,483.8 (6) |
| H-A1 | 38,004 | 36,761 | 41,383 | 43,484 | 48,419 | 44,769 | 42,136.7 (6) |

**Request that held the largest Thinking response**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 4 | 3 | 3 | 3 | 3 | 3 | 3.2 (6) |
| H-N1 | 3 | 3 | 3 | 3 | 3 | 3 | 3.0 (6) |
| H-N2 | 3 | 3 | 3 | 3 | 3 | 3 | 3.0 (6) |
| H-A0 | 3 | 3 | 3 | 3 | 4 | 3 | 3.2 (6) |
| H-A1 | 3 | 3 | 3 | 3 | 3 | 3 | 3.0 (6) |

**Output tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 97,499 | 65,884 | 70,209 | 103,936 | 77,054 | 72,673 | 81,209.2 (6) |
| H-N1 | 74,008 | 76,138 | 61,606 | 68,697 | 68,254 | 74,224 | 70,487.8 (6) |
| H-N2 | 76,529 | 77,814 | 90,298 | 74,496 | 95,901 | 85,788 | 83,471.0 (6) |
| H-A0 | 58,830 | 60,449 | 51,230 | 61,797 | 64,019 | 60,328 | 59,442.2 (6) |
| H-A1 | 53,452 | 56,338 | 60,147 | 61,813 | 64,327 | 61,590 | 59,611.2 (6) |

**Uncached input tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 42 | 30 | 26 | 34 | 40 | 24 | 32.7 (6) |
| H-N1 | 20 | 28 | 26 | 28 | 26 | 30 | 26.3 (6) |
| H-N2 | 32 | 24 | 30 | 28 | 36 | 26 | 29.3 (6) |
| H-A0 | 22 | 22 | 20 | 30 | 18 | 22 | 22.3 (6) |
| H-A1 | 20 | 24 | 20 | 20 | 18 | 18 | 20.0 (6) |

**Cache read tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 1,993,364 | 1,067,361 | 948,788 | 1,738,708 | 1,704,599 | 881,119 | 1,388,989.8 (6) |
| H-N1 | 715,574 | 1,088,615 | 894,670 | 1,072,977 | 952,440 | 1,129,928 | 975,700.7 (6) |
| H-N2 | 1,315,376 | 918,943 | 1,318,713 | 1,057,146 | 1,764,705 | 1,077,731 | 1,242,102.3 (6) |
| H-A0 | 689,064 | 754,114 | 585,731 | 1,160,836 | 508,516 | 744,933 | 740,532.3 (6) |
| H-A1 | 571,009 | 736,127 | 581,653 | 617,682 | 536,725 | 515,511 | 593,117.8 (6) |

**Cache write tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 121,258 | 90,987 | 93,058 | 128,692 | 107,481 | 98,892 | 106,728.0 (6) |
| H-N1 | 98,513 | 100,634 | 86,617 | 94,259 | 91,553 | 96,590 | 94,694.3 (6) |
| H-N2 | 102,178 | 103,731 | 113,652 | 97,586 | 124,833 | 109,324 | 108,550.7 (6) |
| H-A0 | 83,992 | 89,130 | 78,480 | 89,208 | 90,720 | 85,758 | 86,214.7 (6) |
| H-A1 | 78,805 | 84,919 | 85,434 | 86,747 | 90,519 | 86,820 | 85,540.7 (6) |

**API-equivalent USD**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 0.2968 | 0.0788 | 0.0960 | 0.3055 | 0.1550 | 0.1098 | 0.1736 (6) |
| H-N1 | 0.1081 | 0.1183 | 0.0571 | 0.1005 | 0.0804 | 0.1144 | 0.0965 (6) |
| H-N2 | 0.1337 | 0.1204 | 0.1902 | 0.1145 | 0.2352 | 0.1413 | 0.1559 (6) |
| H-A0 | 0.0531 | 0.0617 | 0.0472 | 0.0661 | 0.0724 | 0.0548 | 0.0592 (6) |
| H-A1 | 0.0482 | 0.0525 | 0.0530 | 0.0544 | 0.0556 | 0.0533 | 0.0528 (6) |

**Wall seconds**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 420.1 | 280.1 | 289.6 | 436.7 | 321.0 | 292.6 | 340.0 (6) |
| H-N1 | 307.5 | 312.6 | 255.1 | 276.7 | 276.7 | 315.6 | 290.7 (6) |
| H-N2 | 322.5 | 321.4 | 382.4 | 311.5 | 410.5 | 355.9 | 350.7 (6) |
| H-A0 | 242.1 | 245.1 | 218.6 | 251.9 | 260.9 | 251.8 | 245.1 (6) |
| H-A1 | 218.3 | 237.2 | 246.4 | 257.6 | 260.6 | 253.7 | 245.6 (6) |

**Hidden checks passed (of 119)**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 119 | 119 | 119 | 119 | 119 | 119 | 119.0 (6) |
| H-N1 | 119 | 117 | 119 | 119 | 119 | 119 | 118.7 (6) |
| H-N2 | 119 | 119 | 117 | 112 | 117 | 119 | 117.2 (6) |
| H-A0 | 117 | 119 | 119 | 119 | 119 | 119 | 118.7 (6) |
| H-A1 | 119 | 119 | 119 | 119 | 119 | 119 | 119.0 (6) |

**API requests**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 21 | 15 | 13 | 17 | 20 | 12 | 16.3 (6) |
| H-N1 | 10 | 14 | 13 | 14 | 13 | 15 | 13.2 (6) |
| H-N2 | 16 | 12 | 15 | 14 | 18 | 13 | 14.7 (6) |
| H-A0 | 11 | 11 | 10 | 15 | 9 | 11 | 11.2 (6) |
| H-A1 | 10 | 12 | 10 | 10 | 9 | 9 | 10.0 (6) |

**Test runs**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 5 | 4 | 3 | 4 | 5 | 3 | 4.0 (6) |
| H-N1 | 4 | 3 | 4 | 3 | 2 | 2 | 3.0 (6) |
| H-N2 | 5 | 3 | 4 | 4 | 8 | 4 | 4.7 (6) |
| H-A0 | 4 | 3 | 3 | 2 | 4 | 4 | 3.3 (6) |
| H-A1 | 3 | 3 | 5 | 2 | 2 | 4 | 3.2 (6) |

**Tests in the solver's last test run**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 39 | 39 | 32 | 45 | 55 | 51 | 43.5 (6) |
| H-N1 | 46 | 58 | 49 | 29 | 25 | 29 | 39.3 (6) |
| H-N2 | 45 | 47 | 46 | 34 | 38 | 41 | 41.8 (6) |
| H-A0 | 38 | 41 | 32 | 2 | 39 | 25 | 29.5 (6) |
| H-A1 | 31 | 32 | 33 | 28 | 29 | 28 | 30.2 (6) |

**Seconds from the last change to the end**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 17.5 | 27.4 | 8.3 | 27.0 | 7.6 | 13.0 | 16.8 (6) |
| H-N1 | 8.9 | 11.9 | 10.6 | 13.9 | 22.1 | 19.7 | 14.5 (6) |
| H-N2 | 13.5 | 9.0 | 14.6 | 19.9 | 71.2 | 17.5 | 24.3 (6) |
| H-A0 | 8.6 | 8.8 | 7.2 | 10.0 | 5.2 | unknown | 8.0 (5) |
| H-A1 | 11.4 | 10.8 | unknown | 22.3 | 9.5 | 9.2 | 12.6 (5) |

Hidden checks do not separate the arms: 22 of the 24 trials outside H-N2
passed all 119 and two passed 117; H-N2 passed 119, 119, 117, 112, 117 and
119.

### Contrasts, per block

Each cell is the first arm minus the second in that block. "Lower / higher /
equal" counts the blocks.

**H-N1 minus H-N0** (the envelope, natively)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to H-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +320 | +324 | +318 | +316 | +322 | +324 | +320.7 | +1.4 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | -17,332 | +8,751 | -12,686 | -23,734 | -4,344 | +3,724 | -7,603.5 | -13.6 % | 4 / 2 / 0 |
| Thinking tokens, total | -20,052 | +9,427 | -12,170 | -27,212 | -3,330 | +5,431 | -7,984.3 | -13.4 % | 4 / 2 / 0 |
| Thinking tokens of the largest single response | -17,331 | +8,809 | -12,699 | -23,707 | -4,328 | +3,713 | -7,590.5 | -13.6 % | 4 / 2 / 0 |
| Output tokens | -23,491 | +10,254 | -8,603 | -35,239 | -8,800 | +1,551 | -10,721.3 | -13.2 % | 4 / 2 / 0 |
| API requests | -11 | -1 | +0 | -3 | -7 | +3 | -3.2 | -19.4 % | 4 / 1 / 1 |
| Test runs | -1 | -1 | +1 | -1 | -3 | -1 | -1.0 | -25.0 % | 5 / 1 / 0 |
| Tests in the solver's last test run | +7 | +19 | +17 | -16 | -30 | -22 | -4.2 | -9.6 % | 3 / 3 / 0 |
| Cache read tokens | -1,277,790 | +21,254 | -54,118 | -665,731 | -752,159 | +248,809 | -413,289.2 | -29.8 % | 4 / 2 / 0 |
| Cache write tokens | -22,745 | +9,647 | -6,441 | -34,433 | -15,928 | -2,302 | -12,033.7 | -11.3 % | 5 / 1 / 0 |
| API-equivalent USD | -0.1887 | +0.0396 | -0.0389 | -0.2050 | -0.0745 | +0.0046 | -0.0772 | -44.4 % | 4 / 2 / 0 |
| Wall seconds | -112.6 | +32.5 | -34.5 | -160.0 | -44.3 | +23.1 | -49.3 | -14.5 % | 4 / 2 / 0 |
| Hidden checks passed (of 119) | +0 | -2 | +0 | +0 | +0 | +0 | -0.3 | -0.3 % | 1 / 0 / 5 |

**H-N2 minus H-N1** (dropping the words "Effort low.")

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to H-N1 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | -1 | -1 | -1 | -5 | -9 | -5 | -3.7 | -0.0 % | 6 / 0 / 0 |
| Thinking tokens before the first edit | -1,000 | +1,938 | +26,875 | +2,224 | +18,889 | +4,608 | +8,922.3 | +18.5 % | 1 / 5 / 0 |
| Thinking tokens, total | +138 | +1,753 | +28,031 | +4,226 | +20,164 | +6,120 | +10,072.0 | +19.6 % | 0 / 6 / 0 |
| Thinking tokens of the largest single response | -991 | +1,928 | +26,873 | +2,199 | +18,861 | +4,609 | +8,913.2 | +18.5 % | 1 / 5 / 0 |
| Output tokens | +2,521 | +1,676 | +28,692 | +5,799 | +27,647 | +11,564 | +12,983.2 | +18.4 % | 0 / 6 / 0 |
| API requests | +6 | -2 | +2 | +0 | +5 | -2 | +1.5 | +11.4 % | 2 / 3 / 1 |
| Test runs | +1 | +0 | +0 | +1 | +6 | +2 | +1.7 | +55.6 % | 0 / 4 / 2 |
| Tests in the solver's last test run | -1 | -11 | -3 | +5 | +13 | +12 | +2.5 | +6.4 % | 3 / 3 / 0 |
| Cache read tokens | +599,802 | -169,672 | +424,043 | -15,831 | +812,265 | -52,197 | +266,401.7 | +27.3 % | 3 / 3 / 0 |
| Cache write tokens | +3,665 | +3,097 | +27,035 | +3,327 | +33,280 | +12,734 | +13,856.3 | +14.6 % | 0 / 6 / 0 |
| API-equivalent USD | +0.0256 | +0.0020 | +0.1331 | +0.0139 | +0.1548 | +0.0270 | +0.0594 | +61.6 % | 0 / 6 / 0 |
| Wall seconds | +15.0 | +8.8 | +127.3 | +34.8 | +133.8 | +40.3 | +60.0 | +20.6 % | 0 / 6 / 0 |
| Hidden checks passed (of 119) | +0 | +2 | -2 | -7 | -2 | +0 | -1.5 | -1.3 % | 3 / 1 / 2 |

**H-A1 minus H-A0** (limiting Aether's tools)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to H-A0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | -5,237 | -5,230 | -5,246 | -5,213 | -5,240 | -5,226 | -5,232.0 | -19.6 % | 6 / 0 / 0 |
| Thinking tokens before the first edit | -2,354 | -3,976 | unknown | +37 | +2,831 | unknown | -865.5 | -2.0 % | 2 / 2 / 0 |
| Thinking tokens, total | -3,076 | -2,523 | +7,361 | +1,117 | +2,946 | -127 | +949.7 | +2.2 % | 3 / 3 / 0 |
| Thinking tokens of the largest single response | -2,277 | -3,941 | +6,525 | +39 | +2,831 | +740 | +652.8 | +1.6 % | 2 / 4 / 0 |
| Output tokens | -5,378 | -4,111 | +8,917 | +16 | +308 | +1,262 | +169.0 | +0.3 % | 2 / 4 / 0 |
| API requests | -1 | +1 | +0 | -5 | +0 | -2 | -1.2 | -10.4 % | 3 / 1 / 2 |
| Test runs | -1 | +0 | +2 | +0 | -2 | +0 | -0.2 | -5.0 % | 2 / 1 / 3 |
| Tests in the solver's last test run | -7 | -9 | +1 | +26 | -10 | +3 | +0.7 | +2.3 % | 3 / 3 / 0 |
| Cache read tokens | -118,055 | -17,987 | -4,078 | -543,154 | +28,209 | -229,422 | -147,414.5 | -19.9 % | 5 / 1 / 0 |
| Cache write tokens | -5,187 | -4,211 | +6,954 | -2,461 | -201 | +1,062 | -674.0 | -0.8 % | 4 / 2 / 0 |
| API-equivalent USD | -0.0049 | -0.0092 | +0.0058 | -0.0117 | -0.0167 | -0.0015 | -0.0064 | -10.7 % | 5 / 1 / 0 |
| Wall seconds | -23.8 | -7.9 | +27.8 | +5.7 | -0.4 | +2.0 | +0.5 | +0.2 % | 3 / 3 / 0 |
| Hidden checks passed (of 119) | +2 | +0 | +0 | +0 | +0 | +0 | +0.3 | +0.3 % | 0 / 1 / 5 |

**H-A0 minus H-N0** (the counted gap)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to H-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +3,643 | +3,641 | +3,656 | +3,638 | +3,642 | +3,637 | +3,642.8 | +15.8 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | -30,016 | -2,806 | -16,197 | -30,962 | -4,567 | unknown | -16,909.6 | -29.2 % | 5 / 0 / 0 |
| Thinking tokens, total | -33,494 | -4,377 | -17,002 | -34,688 | -7,066 | -5,837 | -17,077.3 | -28.7 % | 6 / 0 / 0 |
| Thinking tokens of the largest single response | -30,055 | -2,769 | -16,170 | -30,935 | -4,537 | -1,751 | -14,369.5 | -25.7 % | 6 / 0 / 0 |
| Output tokens | -38,669 | -5,435 | -18,979 | -42,139 | -13,035 | -12,345 | -21,767.0 | -26.8 % | 6 / 0 / 0 |
| API requests | -10 | -4 | -3 | -2 | -11 | -1 | -5.2 | -31.6 % | 6 / 0 / 0 |
| Test runs | -1 | -1 | +0 | -2 | -1 | +1 | -0.7 | -16.7 % | 4 / 1 / 1 |
| Tests in the solver's last test run | -1 | +2 | +0 | -43 | -16 | -26 | -14.0 | -32.2 % | 4 / 1 / 1 |
| Cache read tokens | -1,304,300 | -313,247 | -363,057 | -577,872 | -1,196,083 | -136,186 | -648,457.5 | -46.7 % | 6 / 0 / 0 |
| Cache write tokens | -37,266 | -1,857 | -14,578 | -39,484 | -16,761 | -13,134 | -20,513.3 | -19.2 % | 6 / 0 / 0 |
| API-equivalent USD | -0.2437 | -0.0170 | -0.0488 | -0.2394 | -0.0826 | -0.0550 | -0.1144 | -65.9 % | 6 / 0 / 0 |
| Wall seconds | -177.9 | -34.9 | -71.0 | -184.9 | -60.1 | -40.8 | -94.9 | -27.9 % | 6 / 0 / 0 |
| Hidden checks passed (of 119) | -2 | +0 | +0 | +0 | +0 | +0 | -0.3 | -0.3 % | 1 / 0 / 5 |

**H-A0 minus H-N1** (Aether against native with the envelope)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to H-N1 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +3,323 | +3,317 | +3,338 | +3,322 | +3,320 | +3,313 | +3,322.2 | +14.2 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | -12,684 | -11,557 | -3,511 | -7,228 | -223 | unknown | -7,040.6 | -14.7 % | 5 / 0 / 0 |
| Thinking tokens, total | -13,442 | -13,804 | -4,832 | -7,476 | -3,736 | -11,268 | -9,093.0 | -17.7 % | 6 / 0 / 0 |
| Thinking tokens of the largest single response | -12,724 | -11,578 | -3,471 | -7,228 | -209 | -5,464 | -6,779.0 | -14.0 % | 6 / 0 / 0 |
| Output tokens | -15,178 | -15,689 | -10,376 | -6,900 | -4,235 | -13,896 | -11,045.7 | -15.7 % | 6 / 0 / 0 |
| API requests | +1 | -3 | -3 | +1 | -4 | -4 | -2.0 | -15.2 % | 4 / 2 / 0 |
| Test runs | +0 | +0 | -1 | -1 | +2 | +2 | +0.3 | +11.1 % | 2 / 2 / 2 |
| Tests in the solver's last test run | -8 | -17 | -17 | -27 | +14 | -4 | -9.8 | -25.0 % | 5 / 1 / 0 |
| Cache read tokens | -26,510 | -334,501 | -308,939 | +87,859 | -443,924 | -384,995 | -235,168.3 | -24.1 % | 5 / 1 / 0 |
| Cache write tokens | -14,521 | -11,504 | -8,137 | -5,051 | -833 | -10,832 | -8,479.7 | -9.0 % | 6 / 0 / 0 |
| API-equivalent USD | -0.0550 | -0.0566 | -0.0099 | -0.0344 | -0.0081 | -0.0596 | -0.0373 | -38.6 % | 6 / 0 / 0 |
| Wall seconds | -65.4 | -67.4 | -36.5 | -24.8 | -15.8 | -63.9 | -45.6 | -15.7 % | 6 / 0 / 0 |
| Hidden checks passed (of 119) | -2 | +2 | +0 | +0 | +0 | +0 | +0.0 | +0.0 % | 1 / 1 / 4 |

**H-A1 minus H-N0** (Aether with minimal tools against native)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to H-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | -1,594 | -1,589 | -1,590 | -1,575 | -1,598 | -1,589 | -1,589.2 | -6.9 % | 6 / 0 / 0 |
| Thinking tokens before the first edit | -32,370 | -6,782 | unknown | -30,925 | -1,736 | -1,004 | -14,563.4 | -25.6 % | 5 / 0 / 0 |
| Thinking tokens, total | -36,570 | -6,900 | -9,641 | -33,571 | -4,120 | -5,964 | -16,127.7 | -27.1 % | 6 / 0 / 0 |
| Thinking tokens of the largest single response | -32,332 | -6,710 | -9,645 | -30,896 | -1,706 | -1,011 | -13,716.7 | -24.6 % | 6 / 0 / 0 |
| Output tokens | -44,047 | -9,546 | -10,062 | -42,123 | -12,727 | -11,083 | -21,598.0 | -26.6 % | 6 / 0 / 0 |
| API requests | -11 | -3 | -3 | -7 | -11 | -3 | -6.3 | -38.8 % | 6 / 0 / 0 |
| Test runs | -2 | -1 | +2 | -2 | -3 | +1 | -0.8 | -20.8 % | 4 / 2 / 0 |
| Tests in the solver's last test run | -8 | -7 | +1 | -17 | -26 | -23 | -13.3 | -30.7 % | 5 / 1 / 0 |
| Cache read tokens | -1,422,355 | -331,234 | -367,135 | -1,121,026 | -1,167,874 | -365,608 | -795,872.0 | -57.3 % | 6 / 0 / 0 |
| Cache write tokens | -42,453 | -6,068 | -7,624 | -41,945 | -16,962 | -12,072 | -21,187.3 | -19.9 % | 6 / 0 / 0 |
| API-equivalent USD | -0.2486 | -0.0262 | -0.0430 | -0.2511 | -0.0993 | -0.0565 | -0.1208 | -69.6 % | 6 / 0 / 0 |
| Wall seconds | -201.8 | -42.9 | -43.2 | -179.1 | -60.4 | -38.8 | -94.4 | -27.8 % | 6 / 0 / 0 |
| Hidden checks passed (of 119) | +0 | +0 | +0 | +0 | +0 | +0 | +0.0 | +0.0 % | 0 / 0 / 6 |

### Where the difference sits: one response

All 30 trials went the same way. Two requests read the workspace. Then one
response, the third (the fourth in 2 trials), held between 86 % and 100 % of
all the thinking of the trial: the solution is designed there, and the first
file is written in or after it. The arms differ in the size of that one
response:

| Arm | Smallest | Largest | Mean |
| --- | ---: | ---: | ---: |
| H-N0 | 43,471 | 74,380 | 55,853 |
| H-N1 | 38,329 | 53,005 | 48,263 |
| H-N2 | 52,014 | 65,202 | 57,176 |
| H-A0 | 34,858 | 45,588 | 41,484 |
| H-A1 | 36,761 | 48,419 | 42,137 |

No Aether trial reached 50,000 thinking tokens in it; every H-N2 trial
passed 52,000. Everything after it follows: fewer output tokens, fewer
requests, a shorter conversation read from cache on each of them, less time.

The gap between the counted cells, split along the arms. Each row is a
difference per block; the share is the row's mean over the gap's mean:

**Thinking tokens of the largest single response**

| Difference | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean | Share of the gap's mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The gap: H-A0 minus H-N0 | -30,055 | -2,769 | -16,170 | -30,935 | -4,537 | -1,751 | -14,370 | 100 % | 6 / 0 / 0 |
| The envelope, natively: H-N1 minus H-N0 | -17,331 | +8,809 | -12,699 | -23,707 | -4,328 | +3,713 | -7,590 | 53 % | 4 / 2 / 0 |
| Left after the envelope: H-A0 minus H-N1 | -12,724 | -11,578 | -3,471 | -7,228 | -209 | -5,464 | -6,779 | 47 % | 6 / 0 / 0 |
| The words "Effort low.": H-N1 minus H-N2 | +991 | -1,928 | -26,873 | -2,199 | -18,861 | -4,609 | -8,913 | 62 % | 5 / 1 / 0 |
| The envelope without those words: H-N2 minus H-N0 | -18,322 | +10,737 | +14,174 | -21,508 | +14,533 | +8,322 | +1,323 | -9 % | 2 / 4 / 0 |
| Aether's full tool list: H-A0 minus H-A1 | +2,277 | +3,941 | -6,525 | -39 | -2,831 | -740 | -653 | 5 % | 4 / 2 / 0 |

**Thinking tokens, total**

| Difference | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean | Share of the gap's mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The gap: H-A0 minus H-N0 | -33,494 | -4,377 | -17,002 | -34,688 | -7,066 | -5,837 | -17,077 | 100 % | 6 / 0 / 0 |
| The envelope, natively: H-N1 minus H-N0 | -20,052 | +9,427 | -12,170 | -27,212 | -3,330 | +5,431 | -7,984 | 47 % | 4 / 2 / 0 |
| Left after the envelope: H-A0 minus H-N1 | -13,442 | -13,804 | -4,832 | -7,476 | -3,736 | -11,268 | -9,093 | 53 % | 6 / 0 / 0 |
| The words "Effort low.": H-N1 minus H-N2 | -138 | -1,753 | -28,031 | -4,226 | -20,164 | -6,120 | -10,072 | 59 % | 6 / 0 / 0 |
| The envelope without those words: H-N2 minus H-N0 | -19,914 | +11,180 | +15,861 | -22,986 | +16,834 | +11,551 | +2,088 | -12 % | 2 / 4 / 0 |
| Aether's full tool list: H-A0 minus H-A1 | +3,076 | +2,523 | -7,361 | -1,117 | -2,946 | +127 | -950 | 6 % | 3 / 3 / 0 |

**Output tokens**

| Difference | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean | Share of the gap's mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The gap: H-A0 minus H-N0 | -38,669 | -5,435 | -18,979 | -42,139 | -13,035 | -12,345 | -21,767 | 100 % | 6 / 0 / 0 |
| The envelope, natively: H-N1 minus H-N0 | -23,491 | +10,254 | -8,603 | -35,239 | -8,800 | +1,551 | -10,721 | 49 % | 4 / 2 / 0 |
| Left after the envelope: H-A0 minus H-N1 | -15,178 | -15,689 | -10,376 | -6,900 | -4,235 | -13,896 | -11,046 | 51 % | 6 / 0 / 0 |
| The words "Effort low.": H-N1 minus H-N2 | -2,521 | -1,676 | -28,692 | -5,799 | -27,647 | -11,564 | -12,983 | 60 % | 6 / 0 / 0 |
| The envelope without those words: H-N2 minus H-N0 | -20,970 | +11,930 | +20,089 | -29,440 | +18,847 | +13,115 | +2,262 | -10 % | 2 / 4 / 0 |
| Aether's full tool list: H-A0 minus H-A1 | +5,378 | +4,111 | -8,917 | -16 | -308 | -1,262 | -169 | 1 % | 4 / 2 / 0 |

Reading the rows:

- **The gap** is there in 6 of 6 blocks on all three measures, from -1,751
  to -30,935 thinking tokens in the one response.
- **The words "Effort low."** With the envelope held fixed, the native
  solver thought less with the words than without in 6 of 6 blocks on total
  thinking and on output tokens, and in 5 of 6 on the one response. The size
  is not steady: from 138 to 28,031 thinking tokens.
- **The envelope without those words** does nothing that shows: 2 blocks
  lower, 4 higher, mean +2,088 thinking tokens against a native range of
  45,879 to 78,783.
- **What is left after the envelope** (H-A0 against H-N1, the same first
  message on both sides) is lower through Aether in 6 of 6 blocks on all
  three measures, by 3,736 to 13,804 thinking tokens.
- **Aether's tool list** is not it: 3 blocks lower and 3 higher on thinking,
  mean within 1,000 tokens, although it changes the first request by 5,232
  tokens.

The two parts do not add up exactly to a cause each, because the first two
rows share the noisy H-N0: its 6 trials span 45,879 to 78,783 thinking
tokens, the widest range of any arm.

### Where the input tokens differ

The same split as for Luna below (R requests, F the first request's input
tokens):

**H-A0 minus H-N0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | 11 vs 21 | 26,749 vs 23,106 | 773,078 vs 2,114,664 | -1,341,586 | +76,503 | -267,490 | -1,150,599 |
| 2 | 11 vs 15 | 26,747 vs 23,106 | 843,266 vs 1,158,378 | -315,112 | +54,615 | -106,988 | -262,739 |
| 3 | 10 vs 13 | 26,756 vs 23,100 | 664,231 vs 1,041,872 | -377,641 | +47,528 | -80,268 | -344,901 |
| 4 | 15 vs 17 | 26,744 vs 23,106 | 1,250,074 vs 1,867,434 | -617,360 | +61,846 | -53,488 | -625,718 |
| 5 | 9 vs 20 | 26,750 vs 23,108 | 599,254 vs 1,812,120 | -1,212,866 | +72,840 | -294,250 | -991,456 |
| 6 | 11 vs 12 | 26,739 vs 23,102 | 830,713 vs 980,035 | -149,322 | +43,644 | -26,739 | -166,227 |
| Mean (6) | | | | -668,981 | +59,496 | -138,204 | -590,273 |

**H-A1 minus H-A0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | 10 vs 11 | 21,512 vs 26,749 | 649,834 vs 773,078 | -123,244 | -57,607 | -21,512 | -44,125 |
| 2 | 12 vs 11 | 21,517 vs 26,747 | 821,070 vs 843,266 | -22,196 | -57,530 | +21,517 | +13,817 |
| 3 | 10 vs 10 | 21,510 vs 26,756 | 667,107 vs 664,231 | +2,876 | -52,460 | +0 | +55,336 |
| 4 | 10 vs 15 | 21,531 vs 26,744 | 704,449 vs 1,250,074 | -545,625 | -78,195 | -107,655 | -359,775 |
| 5 | 9 vs 9 | 21,510 vs 26,750 | 627,262 vs 599,254 | +28,008 | -47,160 | +0 | +75,168 |
| 6 | 9 vs 11 | 21,513 vs 26,739 | 602,349 vs 830,713 | -228,364 | -57,486 | -43,026 | -127,852 |
| Mean (6) | | | | -148,091 | -58,406 | -25,113 | -64,572 |

**H-N1 minus H-N0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | 10 vs 21 | 23,426 vs 23,106 | 814,107 vs 2,114,664 | -1,300,557 | +6,720 | -257,686 | -1,049,591 |
| 2 | 14 vs 15 | 23,430 vs 23,106 | 1,189,277 vs 1,158,378 | +30,899 | +4,860 | -23,430 | +49,469 |
| 3 | 13 vs 13 | 23,418 vs 23,100 | 981,313 vs 1,041,872 | -60,559 | +4,134 | +0 | -64,693 |
| 4 | 14 vs 17 | 23,422 vs 23,106 | 1,167,264 vs 1,867,434 | -700,170 | +5,372 | -70,266 | -635,276 |
| 5 | 13 vs 20 | 23,430 vs 23,108 | 1,044,019 vs 1,812,120 | -768,101 | +6,440 | -164,010 | -610,531 |
| 6 | 15 vs 12 | 23,426 vs 23,102 | 1,226,548 vs 980,035 | +246,513 | +3,888 | +70,278 | +172,347 |
| Mean (6) | | | | -425,329 | +5,236 | -74,186 | -356,379 |

Aether's larger first request costs it 43,644 to 76,503 input tokens a
trial, and it still reads fewer in 6 of 6 blocks, because it makes fewer
requests (6 of 6) over a shorter conversation.

### The counted runs, read again

The sessions of counted runs 1 to 4 are still in Claude Code's own session
log, and the reader's figures for them equal the published per-trial rows in
all 16. Run 5 ran elsewhere and its sessions were not found. These are
counted trials read a second time, not new ones:

| Run | Case | Largest thinking response, native | Largest thinking response, Aether | Thinking, total, native | Thinking, total, Aether | Output tokens, native | Output tokens, Aether |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | matchbook | 69,033 | 45,826 | 83,613 | 48,209 | 107,357 | 70,121 |
| 1 | gridcalc | 103,678 | 89,435 | 112,868 | 93,676 | 147,699 | 120,791 |
| 2 | matchbook | 59,250 | 38,366 | 60,297 | 39,092 | 80,415 | 52,969 |
| 2 | gridcalc | 82,230 | 100,294 | 110,554 | 103,700 | 158,627 | 126,267 |
| 3 | matchbook | 47,104 | 39,300 | 49,593 | 39,999 | 68,495 | 56,799 |
| 3 | gridcalc | 111,378 | 97,404 | 122,244 | 102,657 | 155,300 | 132,596 |
| 4 | matchbook | 56,292 | 44,099 | 62,634 | 45,945 | 83,889 | 66,848 |
| 4 | gridcalc | 96,545 | 89,780 | 114,173 | 102,313 | 149,316 | 138,285 |

The same response is smaller through Aether in 4 of 4 `matchbook` pairs and
in 3 of 4 `gridcalc` pairs, and total thinking and output tokens are lower in
8 of 8. So the pattern of these blocks is the pattern of the counted runs,
and it is not particular to `matchbook`.

### Verdict for Haiku

**Through Aether, Haiku thinks less in the one response in which it designs
the solution, and in these blocks passes the same checks.** Mean total
thinking is 42,332 tokens against 59,409 (lower in 6 of 6 blocks), mean
output 59,442 against 81,209 (6 of 6), with 118.7 against 119.0 hidden
checks. This is not an overhead; it is the saving the comparison reported,
located.

What is measured:

1. **Where**: the third response, in every trial of every arm.
2. **About half of it is two words.** Aether's first message ends with
   "Turn 1. Effort low." (`crates/aether-context/src/render.rs`,
   `runtime_state`, line 92). Natively, with the same `--effort low` flag,
   the envelope with those words against the envelope without them is
   -10,072 thinking tokens in the mean (lower in 6 of 6 blocks) and -12,983
   output tokens (6 of 6); the gap between the counted cells is -17,077 and
   -21,767. The wrapper around the task, without the words, shows no effect.
3. **The other half is in the Aether session itself.** With the first
   message the same on both sides, Aether is still lower by 9,093 thinking
   tokens (6 of 6) and 11,046 output tokens (6 of 6).
4. **It is not Aether's tool list.** Limiting the agent to two tools changes
   thinking by +950 tokens (3 blocks lower, 3 higher).

What is hypothesis, for the half in point 3. Between H-A1 and H-N1 the
solver sees three differences, ranked by how plausible each is as a cause,
none of them tested:

1. the four orchestration tools and the agent listing that the native CLI
   loads and Aether removes: 24,807 characters that describe delegating,
   scheduling and workflows, on a task that says not to delegate;
2. the operator's connectors, loaded through Aether only;
3. the two completion tools.

How sure. That the gap is real: sure, 6 of 6 here and 8 of 8 in the counted
runs. That the words "Effort low." lower Haiku's thinking: likely. Six of six
one way would come up by chance about once in 32 such comparisons, and more
than one comparison was looked at here; the size (-138 to -28,031) is not
pinned down. That the words account for half the gap rather than a third or
two thirds: not shown by 6 blocks. That the rest lies in the tools: not
tested. Whether the words act through Aether as they do natively was not
tested either: an Aether arm without them needs a change to Aether.

What 6 blocks do not show:

- **Whether thinking less costs accuracy.** Here it did not: the 12 Aether
  trials passed 117 to 119 checks. The comparison's accuracy difference for
  Haiku is one counted Aether trial, 95 of 119 on `matchbook` in run 3. Its
  design response was 39,300 thinking tokens, ordinary for the Aether cell.
  With the 12 trials here that is one low score in 17 Aether trials on
  `matchbook`, against none in 11 of the native cell. Six blocks cannot tell
  a rate of 1 in 17 from none, and Haiku is otherwise at the ceiling on this
  case in every arm.
- **Anything about another effort.** All arms ran at low.

## Luna

### What the first turn was given

A Codex rollout records each block of context sent before the first request,
and not the tool definitions. Characters per block, the same in all 6 trials
of an arm:

| Block of context | L-N0 | L-A0 | L-A1 |
| --- | ---: | ---: | ---: |
| `developer: <collaboration_mode># Collab` | 920 | 920 | 920 |
| `developer: <multi_agent_mode>Any earlie` | 271 | 0 | 0 |
| `developer: <multi_agent_role>You are `/` | 2,400 | 0 | 0 |
| `developer: <permissions instructions>` | 362 | 6,144 | 6,144 |
| `developer: <recommended_plugins>` | 901 | 901 | 901 |
| `developer: <skills_instructions>` | 4,751 | 8,030 | 8,030 |
| `user: # AGENTS.md instructions` | 5,884 | 5,884 | 5,884 |
| `user: <environment_context>` | 575 | 575 | 575 |
| `user: the task` | 789 | 0 | 0 |
| `user: the task (in the Aether envelope)` | 0 | 1,479 | 1,479 |
| Total | 16,853 | 23,933 | 23,933 |

Read against the Aether source:

- **Permission instructions, +5,782 characters.** Aether starts the thread
  with approval policy `on-request` (`crates/aether-engine/src/codex_app.rs`,
  `thread_params`, lines 189 to 190); the native cell runs with `never`. With
  `on-request`, Codex adds its instructions for requesting escalation.
- **Skill listing, +3,279 characters.** The Aether session lists nine more
  skill entries (`pages` four times, `plugin-management`, `sites`,
  `work-pets` three times), by their names plugin skills. `codex exec` does
  not list them.
- **Multi-agent text, -2,671 characters.** Native Codex adds its own
  multi-agent role and mode text; Aether switches that feature off.
- **The task, +690 characters.** The Aether envelope around the same 789
  characters.

The net is +7,080 characters. Both sides also carry the operator's own
instruction file (5,884 characters) and the same environment block.

### Per arm, every trial

"unknown" under reasoning before the first edit means the solver made no
file-change call in that trial: it wrote its files from shell commands, which
the reader does not see as an edit.

**First-request input tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 15,632 | 15,628 | 15,628 | 15,626 | 15,628 | 15,634 | 15,629.3 (6) |
| L-A0 | 16,568 | 16,569 | 16,561 | 16,564 | 16,577 | 16,572 | 16,568.5 (6) |
| L-A1 | 16,570 | 16,568 | 16,570 | 16,577 | 16,574 | 16,569 | 16,571.3 (6) |

**Reasoning tokens before the first edit**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0 | unknown | 0 | unknown | unknown | 0 | 0.0 (3) |
| L-A0 | unknown | unknown | unknown | 0 | 0 | 0 | 0.0 (3) |
| L-A1 | 0 | 0 | 0 | unknown | unknown | 0 | 0.0 (4) |

**Reasoning tokens, total**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 4,084 | 968 | 0 | 2,659 | 0 | 2,065 | 1,629.3 (6) |
| L-A0 | 0 | 0 | 0 | 0 | 0 | 21 | 3.5 (6) |
| L-A1 | 0 | 0 | 0 | 0 | 0 | 0 | 0.0 (6) |

**Output tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 16,748 | 14,018 | 14,350 | 12,387 | 7,130 | 11,570 | 12,700.5 (6) |
| L-A0 | 16,354 | 7,433 | 8,532 | 11,356 | 14,341 | 9,630 | 11,274.3 (6) |
| L-A1 | 13,184 | 7,964 | 10,224 | 7,566 | 7,892 | 16,397 | 10,537.8 (6) |

**Uncached input tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 55,912 | 40,228 | 58,284 | 42,394 | 30,257 | 33,969 | 43,507.3 (6) |
| L-A0 | 47,731 | 40,695 | 29,815 | 40,785 | 41,176 | 63,059 | 43,876.8 (6) |
| L-A1 | 53,730 | 30,733 | 39,180 | 26,814 | 32,233 | 55,705 | 39,732.5 (6) |

**Cache read tokens**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 823,808 | 537,856 | 1,337,344 | 484,864 | 347,392 | 423,680 | 659,157.3 (6) |
| L-A0 | 459,008 | 258,560 | 166,144 | 761,600 | 771,584 | 981,248 | 566,357.3 (6) |
| L-A1 | 714,240 | 301,824 | 640,256 | 280,576 | 319,232 | 1,209,600 | 577,621.3 (6) |

**Input tokens over all requests (uncached + cache read + cache write)**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 879,720 | 578,084 | 1,395,628 | 527,258 | 377,649 | 457,649 | 702,664.7 (6) |
| L-A0 | 506,739 | 299,255 | 195,959 | 802,385 | 812,760 | 1,044,307 | 610,234.2 (6) |
| L-A1 | 767,970 | 332,557 | 679,436 | 307,390 | 351,465 | 1,265,305 | 617,353.8 (6) |

**API-equivalent USD**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0.0222 | 0.0164 | 0.0264 | 0.0153 | 0.0101 | 0.0134 | 0.0173 (6) |
| L-A0 | 0.0175 | 0.0104 | 0.0089 | 0.0174 | 0.0190 | 0.0209 | 0.0157 (6) |
| L-A1 | 0.0191 | 0.0101 | 0.0154 | 0.0093 | 0.0104 | 0.0259 | 0.0150 (6) |

**Wall seconds**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 149.2 | 130.8 | 261.3 | 107.9 | 61.4 | 100.4 | 135.2 (6) |
| L-A0 | 127.6 | 56.5 | 178.0 | 90.4 | 120.1 | 223.9 | 132.7 (6) |
| L-A1 | 123.6 | 163.8 | 95.8 | 60.2 | 62.5 | 118.1 | 104.0 (6) |

**Hidden checks passed (of 119)**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 104 | 116 | 102 | 105 | 84 | 118 | 104.8 (6) |
| L-A0 | 0 | 0 | 99 | 86 | 102 | 90 | 62.8 (6) |
| L-A1 | 76 | 117 | 103 | 111 | 100 | 107 | 102.3 (6) |

**API requests**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 30 | 19 | 44 | 18 | 15 | 17 | 23.8 (6) |
| L-A0 | 15 | 11 | 8 | 25 | 26 | 31 | 19.3 (6) |
| L-A1 | 26 | 13 | 23 | 12 | 13 | 35 | 20.3 (6) |

**Test runs**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 5 | 6 | 4 | 7 | 4 | 3 | 4.8 (6) |
| L-A0 | 1 | 4 | 1 | 7 | 9 | 8 | 5.0 (6) |
| L-A1 | 7 | 3 | 4 | 4 | 3 | 5 | 4.3 (6) |

**Tests in the solver's last test run**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 4 | 4 | 4 | 11 | 4 | 4 | 5.2 (6) |
| L-A0 | unknown | 4 | 4 | 9 | 4 | 4 | 5.0 (5) |
| L-A1 | 4 | 4 | 4 | 4 | 4 | 4 | 4.0 (6) |

**Seconds from the last change to the end**

| Arm | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 8.5 | unknown | 1.6 | unknown | unknown | 20.0 | 10.1 (3) |
| L-A0 | unknown | unknown | unknown | 15.7 | 26.0 | 16.9 | 19.5 (3) |
| L-A1 | 6.1 | 17.6 | 12.5 | unknown | unknown | 6.5 | 10.7 (4) |

The two L-A0 trials with 0 of 119 (blocks 1 and 2) are graded "hidden tests
timed out after 60s". Grading a copy of one of the two workspaces again, with
nothing else running, timed out the same way: the solution hangs, the host
did not. Both trials ended early, after 15 and 11 requests.

### Contrasts, per block

Each cell is the first arm minus the second in that block. "Lower / higher /
equal" counts the blocks.

**L-A0 minus L-N0** (the counted gap)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to L-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +936 | +941 | +933 | +938 | +949 | +938 | +939.2 | +6.0 % | 0 / 6 / 0 |
| API requests | -15 | -8 | -36 | +7 | +11 | +14 | -4.5 | -18.9 % | 3 / 3 / 0 |
| Uncached input tokens | -8,181 | +467 | -28,469 | -1,609 | +10,919 | +29,090 | +369.5 | +0.8 % | 3 / 3 / 0 |
| Cache read tokens | -364,800 | -279,296 | -1,171,200 | +276,736 | +424,192 | +557,568 | -92,800.0 | -14.1 % | 3 / 3 / 0 |
| Input tokens over all requests (uncached + cache read + cache write) | -372,981 | -278,829 | -1,199,669 | +275,127 | +435,111 | +586,658 | -92,430.5 | -13.2 % | 3 / 3 / 0 |
| Output tokens | -394 | -6,585 | -5,818 | -1,031 | +7,211 | -1,940 | -1,426.2 | -11.2 % | 5 / 1 / 0 |
| Reasoning tokens, total | -4,084 | -968 | +0 | -2,659 | +0 | -2,044 | -1,625.8 | -99.8 % | 4 / 0 / 2 |
| API-equivalent USD | -0.0047 | -0.0060 | -0.0175 | +0.0021 | +0.0089 | +0.0075 | -0.0016 | -9.3 % | 3 / 3 / 0 |
| Wall seconds | -21.6 | -74.3 | -83.3 | -17.5 | +58.7 | +123.5 | -2.4 | -1.8 % | 4 / 2 / 0 |
| Hidden checks passed (of 119) | -104 | -116 | -3 | -19 | +18 | -28 | -42.0 | -40.1 % | 5 / 1 / 0 |

**L-A1 minus L-A0** (limiting Aether's tools)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to L-A0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +2 | -1 | +9 | +13 | -3 | -3 | +2.8 | +0.0 % | 3 / 3 / 0 |
| API requests | +11 | +2 | +15 | -13 | -13 | +4 | +1.0 | +5.2 % | 2 / 4 / 0 |
| Uncached input tokens | +5,999 | -9,962 | +9,365 | -13,971 | -8,943 | -7,354 | -4,144.3 | -9.4 % | 4 / 2 / 0 |
| Cache read tokens | +255,232 | +43,264 | +474,112 | -481,024 | -452,352 | +228,352 | +11,264.0 | +2.0 % | 2 / 4 / 0 |
| Input tokens over all requests (uncached + cache read + cache write) | +261,231 | +33,302 | +483,477 | -494,995 | -461,295 | +220,998 | +7,119.7 | +1.2 % | 2 / 4 / 0 |
| Output tokens | -3,170 | +531 | +1,692 | -3,790 | -6,449 | +6,767 | -736.5 | -6.5 % | 3 / 3 / 0 |
| Reasoning tokens, total | +0 | +0 | +0 | +0 | +0 | -21 | -3.5 | -100.0 % | 1 / 0 / 5 |
| API-equivalent USD | +0.0016 | -0.0003 | +0.0065 | -0.0081 | -0.0086 | +0.0049 | -0.0007 | -4.3 % | 3 / 3 / 0 |
| Wall seconds | -4.0 | +107.3 | -82.2 | -30.2 | -57.5 | -105.8 | -28.7 | -21.7 % | 5 / 1 / 0 |
| Hidden checks passed (of 119) | +76 | +117 | +4 | +25 | -2 | +17 | +39.5 | +62.9 % | 1 / 5 / 0 |

**L-A1 minus L-N0** (Aether with minimal tools against native)

| Measure | Block 1 | Block 2 | Block 3 | Block 4 | Block 5 | Block 6 | Mean difference | Relative to L-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +938 | +940 | +942 | +951 | +946 | +935 | +942.0 | +6.0 % | 0 / 6 / 0 |
| API requests | -4 | -6 | -21 | -6 | -2 | +18 | -3.5 | -14.7 % | 5 / 1 / 0 |
| Uncached input tokens | -2,182 | -9,495 | -19,104 | -15,580 | +1,976 | +21,736 | -3,774.8 | -8.7 % | 4 / 2 / 0 |
| Cache read tokens | -109,568 | -236,032 | -697,088 | -204,288 | -28,160 | +785,920 | -81,536.0 | -12.4 % | 5 / 1 / 0 |
| Input tokens over all requests (uncached + cache read + cache write) | -111,750 | -245,527 | -716,192 | -219,868 | -26,184 | +807,656 | -85,310.8 | -12.1 % | 5 / 1 / 0 |
| Output tokens | -3,564 | -6,054 | -4,126 | -4,821 | +762 | +4,827 | -2,162.7 | -17.0 % | 4 / 2 / 0 |
| Reasoning tokens, total | -4,084 | -968 | +0 | -2,659 | +0 | -2,065 | -1,629.3 | -100.0 % | 4 / 0 / 2 |
| API-equivalent USD | -0.0031 | -0.0063 | -0.0109 | -0.0060 | +0.0003 | +0.0124 | -0.0023 | -13.2 % | 4 / 2 / 0 |
| Wall seconds | -25.6 | +33.0 | -165.5 | -47.7 | +1.1 | +17.7 | -31.2 | -23.1 % | 3 / 3 / 0 |
| Hidden checks passed (of 119) | -28 | +1 | +1 | +6 | +16 | -11 | -2.5 | -2.4 % | 2 / 4 / 0 |

### Where the input tokens differ

With R the number of requests and F the first request's input tokens, the
difference in input tokens over all requests between an arm a and an arm b
splits exactly into three parts:

- **larger fixed context**: R_b x (F_a - F_b), what b's requests would cost
  more if each carried a's first-request size;
- **more requests**: (R_a - R_b) x F_a, the extra requests at the fixed size;
- **conversation growth**: the rest, which is what the requests carry beyond
  the first request's size.

**L-A0 minus L-N0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | 15 vs 30 | 16,568 vs 15,632 | 506,739 vs 879,720 | -372,981 | +28,080 | -248,520 | -152,541 |
| 2 | 11 vs 19 | 16,569 vs 15,628 | 299,255 vs 578,084 | -278,829 | +17,879 | -132,552 | -164,156 |
| 3 | 8 vs 44 | 16,561 vs 15,628 | 195,959 vs 1,395,628 | -1,199,669 | +41,052 | -596,196 | -644,525 |
| 4 | 25 vs 18 | 16,564 vs 15,626 | 802,385 vs 527,258 | +275,127 | +16,884 | +115,948 | +142,295 |
| 5 | 26 vs 15 | 16,577 vs 15,628 | 812,760 vs 377,649 | +435,111 | +14,235 | +182,347 | +238,529 |
| 6 | 31 vs 17 | 16,572 vs 15,634 | 1,044,307 vs 457,649 | +586,658 | +15,946 | +232,008 | +338,704 |
| Mean (6) | | | | -92,430 | +22,346 | -74,494 | -40,282 |

**L-A1 minus L-N0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | 26 vs 30 | 16,570 vs 15,632 | 767,970 vs 879,720 | -111,750 | +28,140 | -66,280 | -73,610 |
| 2 | 13 vs 19 | 16,568 vs 15,628 | 332,557 vs 578,084 | -245,527 | +17,860 | -99,408 | -163,979 |
| 3 | 23 vs 44 | 16,570 vs 15,628 | 679,436 vs 1,395,628 | -716,192 | +41,448 | -347,970 | -409,670 |
| 4 | 12 vs 18 | 16,577 vs 15,626 | 307,390 vs 527,258 | -219,868 | +17,118 | -99,462 | -137,524 |
| 5 | 13 vs 15 | 16,574 vs 15,628 | 351,465 vs 377,649 | -26,184 | +14,190 | -33,148 | -7,226 |
| 6 | 35 vs 17 | 16,569 vs 15,634 | 1,265,305 vs 457,649 | +807,656 | +15,895 | +298,242 | +493,519 |
| Mean (6) | | | | -85,311 | +22,442 | -58,004 | -49,748 |

**L-A1 minus L-A0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | 26 vs 15 | 16,570 vs 16,568 | 767,970 vs 506,739 | +261,231 | +30 | +182,270 | +78,931 |
| 2 | 13 vs 11 | 16,568 vs 16,569 | 332,557 vs 299,255 | +33,302 | -11 | +33,136 | +177 |
| 3 | 23 vs 8 | 16,570 vs 16,561 | 679,436 vs 195,959 | +483,477 | +72 | +248,550 | +234,855 |
| 4 | 12 vs 25 | 16,577 vs 16,564 | 307,390 vs 802,385 | -494,995 | +325 | -215,501 | -279,819 |
| 5 | 13 vs 26 | 16,574 vs 16,577 | 351,465 vs 812,760 | -461,295 | -78 | -215,462 | -245,755 |
| 6 | 35 vs 31 | 16,569 vs 16,572 | 1,265,305 vs 1,044,307 | +220,998 | -93 | +66,276 | +154,815 |
| Mean (6) | | | | +7,120 | +41 | +16,545 | -9,466 |

### The counted runs, read again

The sessions of counted runs 1 to 4 are still in Codex's own session log, and
the reader's figures for them equal the published per-trial rows (uncached
input, output and cache read tokens match in all 16). Run 5 ran elsewhere and
its sessions were not found. These are counted trials read a second time, not
new ones:

| Run | Case | Requests, native | Requests, Aether | First-request input, native | First-request input, Aether | Input tokens, all requests, native | Input tokens, all requests, Aether |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | matchbook | 35 | 11 | 15,626 | 16,546 | 1,162,052 | 279,303 |
| 1 | gridcalc | 23 | 20 | 15,599 | 16,530 | 723,655 | 608,150 |
| 2 | matchbook | 14 | 15 | 15,620 | 16,557 | 351,246 | 403,902 |
| 2 | gridcalc | 130 | 69 | 15,597 | 16,526 | 7,289,071 | 3,051,750 |
| 3 | matchbook | 47 | 17 | 15,620 | 16,561 | 1,574,768 | 474,681 |
| 3 | gridcalc | 98 | 104 | 15,601 | 16,534 | 4,366,030 | 5,419,650 |
| 4 | matchbook | 16 | 145 | 15,624 | 16,552 | 381,256 | 9,914,874 |
| 4 | gridcalc | 32 | 32 | 15,601 | 16,534 | 1,102,476 | 1,039,459 |

Three things follow, the first two from this table and the third from the
published per-trial table of all 10 pairs:

- The fixed part is the same in the counted runs: the first request is 920 to
  941 tokens larger through Aether in 8 of 8 pairs.
- Aether did not make more requests as a rule: 413 against 395 in total,
  fewer in 4 pairs, more in 3, equal in 1. Both sides have long runs: 130 and
  98 requests natively, 145 and 104 through Aether.
- **One pair is more than the whole counted gap.** In run 4 on `matchbook`
  the Aether trial made 145 requests and the native trial 16. That one pair
  is +9,297,664 cache read tokens, +0.1297 USD, +821.6 seconds and +26,327
  output tokens. The sums over all 10 counted pairs are +5,253,376, +0.0953,
  +644.0 and +27,453. Over the other 9 pairs Aether is lower in the mean on
  cache read (-449,365 per pair), cost (-0.0038 USD) and wall time (-19.7
  seconds), and level on output (+125).

### Verdict for Luna

**No overhead per trial in tokens, cost or time was found, because in these
blocks there is none to find. Hidden checks are a separate matter and are not
settled.** In 6 blocks L-A0 used fewer input tokens than L-N0 in 3 and more
in 3 (mean -92,430 of 702,665), cost less in 3 and more in 3 (mean -0.0016
USD), and was faster in 4.

What is measured:

1. **A fixed cost of 939 input tokens per request**, about 6 % of a request's
   fixed context: 16,569 against 15,629 on the first request, higher in 6 of
   6 blocks (by 933 to 949) and in 8 of 8 counted pairs. Over a trial it is
   the "larger fixed context" column: +14,235 to +41,052 tokens, mean
   +22,346, about 3 % of a trial's input tokens.
2. **Aether's tool list is not what that cost is.** Limiting the agent to
   two tools changes the first request by +3 tokens (16,571 against 16,569;
   lower in 3 blocks, higher in 3).
3. **Where the 939 tokens come from, in characters**: the four differences
   listed above, net +7,080 characters. The tool definitions are not
   recorded, so their share is unknown. 7,080 characters is more than 939
   tokens of text, so the native side's tool definitions are probably the
   larger set; that last step is an inference, not a measurement.
4. **The rest is how long the run goes**, and that varies more inside an arm
   than between arms: 15 to 44 requests in L-N0, 8 to 31 in L-A0, 12 to 35 in
   L-A1. The "more requests" and "conversation growth" columns carry the
   sign of the request count in every block.
5. **The counted Luna gap is one long trial**, as shown above.

What 6 blocks do not show:

- **Whether long runs are more likely through Aether.** The counted runs
  have one of 145 requests through Aether and one of 130 natively; these
  blocks have none above 44. A difference in the rate of such runs would
  need many more trials than 6, or than 16.
- **Whether Aether's full tool list costs Luna hidden checks.** These blocks
  alone point that way. L-A0 passed fewer checks than L-N0 in 5 of 6 blocks
  (mean 62.8 against 104.8; in the 4 blocks without a zero score, 94.3
  against 102.3, lower in 3). L-A1, which differs from L-A0 only in the tool
  list, passed 102.3 in the mean: more than L-A0 in 5 of 6 blocks, and level
  with L-N0 (lower in 2, higher in 4). The counted runs do not point that
  way: in their five `matchbook` pairs the same Aether cell passed 111, 99,
  117, 96 and 100 against 95, 80, 111, 104 and 118 natively, more in 3 pairs
  and fewer in 2. Over all 11 `matchbook` pairs of the two counted cells that
  is fewer in 7 and more in 4, with both zero scores on Aether's side. That
  is not enough to say the tool list costs checks, and too much to dismiss.
  It is the one Luna result here that asks for more blocks, of L-A0 against
  L-A1. The cause of the two hangs was not looked for beyond the re-grade.
- **Reasoning tokens.** L-N0 reported reasoning in 4 of 6 trials (968 to
  4,084 tokens); the Aether arms reported 0 in 11 of 12. In the counted runs
  1 to 4 it was 1 of 8 natively and 1 of 8 through Aether. Both sides record
  effort `low` in the session. Not explained, and small against 10,000 to
  13,000 output tokens.
- **Anything about `gridcalc`**, which was not run here.

## What this points to

Nothing here was changed in Aether. For neither model does the diagnosis
locate a defect in Aether that costs tokens, so there is no repair to
propose. It points to one line to keep, to one optional change in Aether, to
one change it advises against, and to the arms that would finish the Haiku
question and the open Luna one.

*Iteration 2 took points 1, 2, 4 and 5 further: the words of point 1 were
removed for engine turns on the operator's decision, the flag of point 2 was
added in the benchmark only, and the arms of points 4 and 5 were run. See
[Iteration 2](#iteration-2). The points below stand as iteration 1 wrote
them.*

**1. Keep, in Aether: the effort in the first message.**

- Where: `crates/aether-context/src/render.rs`, `runtime_state`, line 92,
  which writes " Effort low." after "Turn 1." for an agent that has an
  effort set.
- Change: none. It is the one located source of Haiku's saving. No Rust
  test under `crates/` was found that asserts those words, so a rewording
  would pass unnoticed; a test in `crates/aether-context/src/tests.rs` that
  the runtime status of an agent with an effort names it would pin them.
- Cost in Aether's function of removing or rewording them, measured
  natively: about 10,000 more thinking tokens and 13,000 more output tokens
  per Haiku trial on this case (6 of 6 blocks), with no more checks passed.
- How to test a rewording: H-N1 against H-N2 with the new words in the
  envelope fixture (`evals/harness_bench_treatments.py`); nothing in Aether
  has to change for that.

**2. Optional, in Aether: let an agent run without the operator's MCP
servers.**

- Where: `crates/aether-engine/src/claude_code.rs`, `args()`, the block at
  lines 200 to 213 that passes Aether's own server with `--mcp-config`.
- Change: also pass `--strict-mcp-config` there when a new per-agent (or
  daemon) setting says the agent does not inherit the operator's MCP
  servers. Default unchanged. The CLI then loads only the server named by
  `--mcp-config`, which is Aether's own. Aether already passes this pair of
  flags for an agent without network access (lines 145 to 151).
- Cost in Aether's function: an agent with the setting loses the operator's
  connectors, which some agents are started for. That is why it is a setting
  and not the default.
- What it buys, measured: the first request of every Claude agent stops
  carrying 54 tool names from two servers and 3,719 characters of their
  instructions. In tokens this was not isolated by any arm.
- How to test: a unit test of `args()` with and without the setting (the
  tests of the network-denied path already assert the flag), then the
  second arm below against H-A0.

**3. Not advised, in Aether: the Codex approval policy.**

- Where: `crates/aether-engine/src/codex_app.rs`, `thread_params`, lines 189
  to 190: an agent that bypasses permissions and has network access runs
  with `("on-request", "danger-full-access")`.
- Change that would remove the text: `never` there, as natively. It would
  take the 5,782 characters of escalation instructions out of every request.
- Cost in Aether's function: the comment above `thread_params` gives it.
  With `never`, Codex refuses outright any command that an operator's prompt
  rule covers, with no request Aether could answer. `on-request` was chosen
  for that reason.
- What it would buy: at most the permission text's share of 939 tokens per
  request, which is about 3 % of a Luna trial's input tokens and is read
  from cache after the first request. That does not pay for the cost.

**4. The next two arms for Haiku**, both in `evals/` only, for the half of
the saving that the words do not explain:

1. `diag-native-haiku-low-nocoord`: H-N0 with `--disallowedTools` and the
   list Aether passes (`NATIVE_COORDINATION_TOOLS`,
   `crates/aether-engine/src/claude_code.rs` line 21), added in
   `native_claude_command` (`evals/harness_bench_config.py`, line 84) for a
   condition that asks for it. If the design response falls to Aether's
   size, the cause is the four orchestration tools the native CLI loads and
   Aether removes.
2. `diag-aether-haiku-low-noconnectors`: H-A0 with the operator's
   connectors kept out, by having the receipt shim the benchmark already
   puts in front of the CLI (`evals/harness_bench_receipts.py`, line 62)
   add `--strict-mcp-config` for a condition that asks for it. If the
   design response grows to the native size, the cause is the connectors.

If neither moves it, what is left is the two completion tools and the launch
flags that the model does not see.

**5. For Luna's hidden checks: more blocks of two arms that exist.** L-A0
against L-A1 (`aether-single-luna-low` against
`aether-solo-minimal-luna-low`), nothing to write. If L-A0 keeps passing
fewer checks, the tool list an agent is given by default is the place to
look: `tools` on the agent, which the minimal arm sets to `complete_task` and
`fail_task`.

## Is the comparison fair to both sides?

*This section describes protocol version 1 and the binary of iteration 1.
From iteration 2 the first two rows of the table no longer hold: Aether's
first message to an engine has no "Effort low.", and under protocol version 2
neither side loads the operator's connectors.*

What the solver is given differs between the sides in ways that are not the
thing being compared. All of it is measured above:

| Difference | Native | Aether | Effect |
| --- | --- | --- | --- |
| The words "Effort low." in the first message | absent; the effort is only the `--effort low` flag | present, with the same flag | natively the words alone lower Haiku's thinking by about 10,000 tokens a trial. They are part of what Aether is, so this is the treatment and not a flaw of the setup; a native user can type them |
| The operator's MCP connectors, Claude | none: the benchmark passes `--strict-mcp-config` | loaded: 54 tools named from two servers, 3,719 characters of their instructions | Aether's solver reads context that has nothing to do with the task. Not the benchmark's intent: the flag was added so that a native trial could not reach an Aether bridge |
| Claude Code's own orchestration tools | `Agent`, `ListAgents`, `ScheduleWakeup`, `Workflow` loaded (24,807 characters), 5 agent types listed | removed; Aether's own tools instead (about 24,500 characters in H-A0, 2,500 in H-A1) | this is the treatment itself, on a task whose text tells both sides not to delegate |
| Codex approval text | 362 characters (`never`) | 6,144 characters (`on-request`) | fixed cost to Aether on every request |
| Codex skill listing | 4,751 characters | 8,030 characters | fixed cost to Aether on every request |
| Codex multi-agent text | 2,671 characters | none | fixed cost to native on every request |

In tokens the net is against Aether on both models: +3,643 on Haiku's first
request (H-A0 against H-N0) and +939 on Luna's. Whether any of it changes
what the solver does is not shown. The one part that was removed in an arm
(Aether's tool list, H-A1 and L-A1) changed no token, cost or time measure
consistently; for Luna's hidden checks see the verdict above.

Equal on both sides, but worth knowing: both solvers also read the
operator's own instruction file (about 6,000 characters) and, in Claude Code,
a skill listing of 18,269 characters. Results therefore depend on the account
they were run from, on both sides alike.

**The smallest change that makes both sides start equal** is the second arm
above made the rule: the receipt shim adds `--strict-mcp-config` to every Claude process
of an Aether trial (`evals/harness_bench_receipts.py`, line 62). One line, no
Rust. Then neither side loads the operator's connectors and Aether's own
server still loads. It changes a counted treatment, so it needs a new
protocol version, and results before and after it are not to be pooled. For
Codex there is no such change without touching Aether: the approval policy
and the skill listing are set where Aether starts the thread. That difference
is 939 tokens per request and is best reported as known.

*Iteration 2 made this change: it is protocol version 2.*

## Iteration 2

Iteration 1, above, located part of Haiku's difference in the words "Effort
low." and left the rest between two candidates, and it found one way in which
the benchmark started the two sides of a Claude pair unequal. Iteration 2
acts on both and tests again:

1. Aether no longer repeats the effort in the status line of an agent whose
   engine already passes the effort to its CLI.
2. The benchmark starts both sides of a Claude pair with the same MCP set.
   That is protocol version 2.
3. 60 more trials on `matchbook`, every Aether arm on the changed
   binary: Haiku in 6 blocks of 5 arms, Luna in 10 blocks of 3. They are
   diagnostic like the first 48: all are reported here and none is counted.

The sections above are iteration 1 as it was written, with its figures
unchanged. Where iteration 2 overtakes a statement there, a note says so.

### What was changed before the retest

**In Aether: the status line of an engine turn no longer names the effort.**

- Where: `crates/aether-context/src/render.rs`. `engine_runtime_state` (line
  183) writes the status line of an agent that runs on a CLI engine. It now
  builds the line with `status_text(&brief, None)` (line 190), and so ends
  the first message with "Turn 1." where it ended it with "Turn 1. Effort
  low.".
- Why there: every engine passes the agent's effort to its CLI (Claude Code
  as `--effort`, Codex as `model_reasoning_effort`, the Codex app server per
  turn as well). In the status line the effort was stated a second time, and
  iteration 1 measured that the second statement lowers Haiku's thinking
  below what the flag alone asks for.
- Which caller keeps the words: `runtime_state` (line 87), the status line of
  an agent that calls a provider's API directly. There the effort is sent as
  a request parameter only to a model whose profile has the effort
  capability. For any other model the status line is the only place the
  effort is stated, so the words stay.
- Test: `engine_status_leaves_the_effort_to_the_cli_and_keeps_the_other_figures`
  in `crates/aether-context/src/tests.rs`. It holds both halves: the engine
  line of an agent with effort low, a token budget and a turn limit is
  exactly "Turn 3. Token budget remaining: 5000. Turns used 2/9.", and the
  API line of the same agent still reads "Turn 3. Effort low. ...".
- What it gives up, as iteration 1 measured it natively: with the words
  (H-N1) a Haiku trial cost 0.0965 USD in the mean and passed 118.7 checks;
  without them (H-N2) 0.1559 USD and 117.2. The change trades that saving
  for an agent that works at the effort it was asked for.
- Checked in the retest: the first message of an Aether Haiku trial, read
  from its session transcript, equals iteration 1's no-effort envelope
  fixture built for the same workspace, character for character, apart from
  the random nonce and task id. It is 1,479 characters (iteration 1's
  fixture was 1,481: the trial directory's path, which the envelope prints
  twice, was one character longer there). For Luna the envelope went from
  1,479 to 1,467 characters, the 12 of " Effort low.", and the first request
  from 16,569 to 16,558 input tokens.

**In the benchmark: both sides of a Claude pair start with the same MCP set.**

- Before: a native Claude trial was started with `--strict-mcp-config` and
  an Aether one was not, so only the Aether solver was shown the operator's
  connectors (the 54 tool names of two MCP servers and 3,719 characters of
  server instructions).
- Now: in an Aether trial the receipt shim, which the benchmark already puts
  in front of the CLI, adds `--strict-mcp-config` to every print-mode launch
  (`evals/harness_bench_receipts.py`), ahead of Aether's own arguments. The
  CLI then loads only the servers named with `--mcp-config`, and Aether
  names its own. Every receipt records whether the flag was added.
- Aether itself is unchanged by this. Outside the benchmark an agent still
  inherits the operator's servers.
- `PROTOCOL_VERSION = 2` (`evals/harness_bench_config.py`) is written into
  every manifest; a manifest without the field is version 1.
  `harness_bench_compare.py` and `harness_bench_report.py` refuse to merge
  manifests of two versions.
- **The published five-run figures were measured under version 1** and are
  left as they are. Under version 2 `aether-single-haiku-low` starts
  differently, so its figures are not to be pooled with them. No counted run
  under version 2 existed when this iteration was written; iteration 3 made
  them ([Iteration 3](#iteration-3)).

**Aether's own server still loads under the flag.** In all 6 trials of the
version 2 cell (H-A2) the transcript's prompt snapshot lists the 20
`mcp__aether__*` tools among the always-loaded ones, the same 20 as in the
arm without the flag. The CLI learns a tool's schema from its server, so a listed schema means a
connected server. What was not observed under the flag is a call: no H-A2
trial called an Aether tool, where 3 of 6 H-A0 trials called
`complete_task` once. All 12 workflows completed, because a turn that ends
without the call completes the task as well. Whether the connectors make
Haiku more likely to call `complete_task` (3 of 6 against 0 of 6) is not
shown by 6 blocks.

**The call was checked in iteration 3, and it works.** One diagnostic trial
of `diag-aether-haiku-low-complete-task` on `ledger` (the version 2 cell
with one sentence asking the solver to finish by calling `complete_task`):
the shim added the flag, the solver called `mcp__aether__complete_task`, and
the isolated daemon recorded the call as succeeded and the task as completed
within 17 ms; the trial passed 22/22. So Aether's own server answers a call
under `--strict-mcp-config`. The record is in the
[version 2 report](harness-lower-tier-results-2026-10-10-protocol-2.md#not-counted-the-complete_task-check).

| Arm | Block | Trial | `--strict-mcp-config` added by the shim | `mcp__aether__*` tools loaded | Deferred tools named | MCP server instructions, characters | Calls to `mcp__aether__*` tools | Workflow | Hidden checks |
| --- | ---: | --- | --- | ---: | ---: | ---: | --- | --- | ---: |
| H-A0 | 11 | `a0df35966c` | no | 20 | 67 | 3,719 | `complete_task` 1 | completed | 119 |
| H-A0 | 12 | `394d17eb04` | no | 20 | 67 | 3,719 | none | completed | 117 |
| H-A0 | 13 | `69768ad699` | no | 20 | 67 | 3,719 | `complete_task` 1 | completed | 117 |
| H-A0 | 14 | `aa071a5fb9` | no | 20 | 67 | 3,719 | none | completed | 119 |
| H-A0 | 15 | `e9844f86c7` | no | 20 | 67 | 3,719 | `complete_task` 1 | completed | 119 |
| H-A0 | 16 | `6edb2173d4` | no | 20 | 67 | 3,719 | none | completed | 119 |
| H-A2 | 11 | `0de0d4b775` | yes | 20 | 10 | none recorded | none | completed | 119 |
| H-A2 | 12 | `e80bad7755` | yes | 20 | 10 | none recorded | none | completed | 119 |
| H-A2 | 13 | `d415bb39d6` | yes | 20 | 10 | none recorded | none | completed | 119 |
| H-A2 | 14 | `37fd4c9a8b` | yes | 20 | 10 | none recorded | none | completed | 119 |
| H-A2 | 15 | `4da6820b66` | yes | 20 | 10 | none recorded | none | completed | 119 |
| H-A2 | 16 | `9b0a337509` | yes | 20 | 10 | none recorded | none | completed | 119 |

**Codex was checked for the same asymmetry. Nothing was changed there.** A
native Codex trial is started with `-c mcp_servers={}`. An Aether Codex turn
does not clear that table, so a Codex MCP server configured on the machine
would load through Aether only. This machine configures none, and the Luna
context tables of both iterations show no MCP block on either side. With no
measured difference there is no evidence to change the Codex side on;
`evals/harness_bench_protocol.md` records the gap as known.

**Two smaller corrections from the review of iteration 1.**

- `--diagnostic` is now checked against the resolved conditions, so a
  padded or grouped name of a diagnostic arm cannot run without the flag
  (`evals/harness_bench.py`, `parse_args`; test in
  `evals/test_harness_bench_diagnostics.py`).
- "Before the first edit" counts the response that makes the edit, for both
  CLIs. The reader always did that; its description now says it, and a test
  holds it for a Claude and a Codex transcript
  (`evals/test_harness_bench_transcript.py`).

### The arms of iteration 2

All arms run the frozen case `matchbook` at requested effort low. The Aether
arms run the binary with the change above.

| Arm | Condition | Differs from | In |
| --- | --- | --- | --- |
| H-N0 | `native-haiku-low` | | the counted native cell, as in iteration 1 |
| H-N3 | `diag-native-haiku-low-nocoord` | H-N0 | `--disallowedTools` with the ten names Aether passes for an agent with network access (`NATIVE_COORDINATION_TOOLS`, `crates/aether-engine/src/claude_code.rs` line 21) |
| H-N4 | `diag-native-haiku-low-nocoord-connectors` | H-N3 | the benchmark's `--strict-mcp-config` is left out, so the operator's connectors load as they did through Aether under version 1 |
| H-A0 | `diag-aether-haiku-low-connectors` | H-A2 | the shim does not add `--strict-mcp-config`: the Aether cell as it started under version 1 |
| H-A2 | `aether-single-haiku-low` | | the counted Aether cell as it starts under version 2: one agent on `claude-code/claude-haiku-5-5`, connectors not loaded |
| L-N0 | `native-luna-low` | | the counted native cell, as in iteration 1 |
| L-A0 | `aether-single-luna-low` | | the counted Aether cell: one agent on `codex/gpt-6-luna` |
| L-A1 | `aether-solo-minimal-luna-low` | L-A0 | the agent's Aether tools are limited to `complete_task` and `fail_task` |

The matched pairs are **H-A2 against H-N0** and **L-A0 against L-N0**. The
stop rule is held against those.

Where an arm is not what its name in iteration 1 was, or not exactly its one
change:

- **H-A0 is not iteration 1's H-A0.** It starts the same way (connectors
  loading) but on the changed binary, so its first message has no "Effort
  low.". Comparing the two is a comparison across two sittings about an
  hour apart, and is marked as that wherever it is made.
- **H-A2 is new.** Iteration 1 had no Aether arm without the connectors.
- **H-N3 removes more than the model was shown.** Of the ten names, the
  native CLI loads four in every request (`Agent`, `ListAgents`,
  `ScheduleWakeup`, `Workflow`) and names others as deferred tools. With
  the list passed, the always-loaded tools go from 11 to 7, their schema
  from 43,600 to 18,793 characters, and the deferred tools named from 15 to
  10.
- **H-N4 loads whatever the operator's account configures.** In blocks 11,
  12, 14 and 16 that is 57 more deferred tool names than H-N3 (67 against
  10) and 3,719 characters of server instructions, the same as H-A0 shows.
  Of the 57, 54 are the tools of the operator's two MCP servers, the figure
  iteration 1 gives; the other 3 are the difference in the CLI's own
  deferred names (13 with the connectors by iteration 1's count, 10
  without). **In blocks 13 and 15 H-N4 loaded a smaller connector set**: 18
  deferred names and 1,893 characters of instructions. H-N4 minus H-N3 is
  lower in both of those blocks as well (-22,970 and -7,589 output tokens),
  so the 5 of 6 does not rest on the full set. Before launch the operator's
  Claude configuration was read and holds no Aether server, so a native
  trial could not reach a daemon; the benchmark also strips `AETHER_*` from
  a native trial's environment.
- **H-A2 against H-N3 is still not one change.** Both lack the coordination
  tools and the connectors. H-A2 has Aether's 20 tools (27 always loaded,
  43,321 characters of schema) and the envelope around the task.

### How iteration 2 was run

| | |
| --- | --- |
| Aether binary | `aether 0.1.1`, `cargo build --release` at commit `f346e09b` (the status line change), SHA-256 `251c0b08f359c8a679a76831dea1d892b0c2a0a2c34e51427323c8b25d4d4145`, not installed; every trial ran on its own isolated daemon |
| Benchmark code | commit `ad3d9b5a`, from a frozen copy of that commit, so later commits could not change a running batch; every manifest records the commit with a clean tree and `protocol_version` 2 |
| Native CLIs | `claude` 2.1.294 (Claude Code), `codex-cli` 0.162.1, as in iteration 1 and the counted runs |
| Case | `matchbook`, 119 hidden checks, the same frozen digest as in iteration 1 |
| Blocks | Haiku 6, Luna 10, fixed before the first trial and all run; nothing was stopped early. A block is one runner invocation with `--runs 1`: every arm of the model once, all started together |
| Seeds | Haiku 11 to 16, Luna 11 to 20, one per block |
| Trials at once | Haiku `--jobs 5`, Luna `--jobs 3` |
| Timeout | `--timeout 1200` seconds; one trial reached it: `f3a914ce1f`, L-A1 in Luna block 14, stopped after 1,253.8 seconds (see below). The longest other trial took 477.0 seconds (H-N3 in block 14) |
| Flag | `--diagnostic` on every invocation, recorded in every manifest |
| Trials | Haiku 30 (5 arms, 6 blocks), Luna 30 (3 arms, 10 blocks) |

Before every block the available memory and the free space on the home
volume were read and required to be at least 3 GB and 15 GB:

| Block (seed) | Haiku start (UTC) | Haiku end | Available memory (MB) | Free disk (GB) | Luna start (UTC) | Luna end | Available memory (MB) | Free disk (GB) |
| ---: | --- | --- | ---: | ---: | --- | --- | ---: | ---: |
| 11 | 20:02:54 | 20:10:26 | 8,677 | 36 | 20:02:55 | 20:05:57 | 8,636 | 36 |
| 12 | 20:10:26 | 20:16:21 | 8,499 | 36 | 20:05:57 | 20:10:42 | 8,246 | 36 |
| 13 | 20:34:20 | 20:40:43 | 4,214 | 36 | 20:10:42 | 20:12:49 | 7,953 | 36 |
| 14 | 20:40:43 | 20:48:41 | 8,503 | 36 | 20:12:49 | 20:35:06 | 7,837 | 36 |
| 15 | 20:48:41 | 20:54:57 | 8,714 | 36 | 20:35:06 | 20:38:24 | 9,612 | 36 |
| 16 | 20:54:57 | 21:01:58 | 9,062 | 36 | 20:38:24 | 20:43:02 | 8,326 | 36 |
| 17 |  |  |  |  | 20:43:02 | 20:44:29 | 8,566 | 36 |
| 18 |  |  |  |  | 20:44:29 | 20:46:59 | 8,434 | 36 |
| 19 |  |  |  |  | 20:46:59 | 20:50:02 | 8,999 | 36 |
| 20 |  |  |  |  | 20:50:02 | 20:54:40 | 8,626 | 36 |

- **The two batches ran at the same time**, so up to 8 trials ran at once.
  The arms of one block shared their load, and blocks are only compared
  within themselves.
- **No launch was aborted and no manifest was re-priced.** Every runner
  invocation ended with exit status 0.
- **Iteration 2 started 33 minutes after iteration 1 ended**, on the same
  machine and accounts.
- **One Luna trial held the machine's memory for 18 minutes.** The solver
  of `f3a914ce1f` (L-A1, block 14) started a test of its own that looped
  and grew until the 1,200 second timeout stopped the trial. Swap in use
  reached 17 GB and the load average passed 200. The Haiku runner, which
  reads the memory before each block, waited from 20:16:21 to 20:34:20
  before it started block 13. The other two trials of Luna block 14 had
  ended before it began; Haiku block 12 ended just as it began. Token
  figures do not depend on the load; wall seconds of Haiku block 13 may be
  a little long, as that trial's grading was still running when it started.
- **A guard ran from 20:36:13 to the end.** It would have killed any
  process started inside a trial workspace that held more than 4 GB, the
  grader excepted. It never fired. It was added for the machine's sake and
  was the same for every arm.

Figures are read as in iteration 1: token, cost, time and hidden-check
figures from the manifests, the rest from each solver's session transcript.

### Haiku, iteration 2

#### What the first request carried

**What the first request carried (as the CLI recorded it; one value when every trial agrees)**

| Arm | Always-loaded tools | Their schema, characters | System prompt, characters | Deferred tools named | MCP server instructions, characters | First user message, characters |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 11 | 43,600 | 9,850 | 15 | unknown | 789 |
| H-N3 | 7 | 18,793 | 9,850 | 10 | unknown | 789 |
| H-N4 | 7 | 18,793 | 9,850 | 18 to 67 | 1,893 to 3,719 | 789 |
| H-A0 | 27 | 43,321 | 9,850 | 67 | 3,719 | 1,479 |
| H-A2 | 27 | 43,321 | 9,850 | 10 | unknown | 1,479 |

- H-N3 against H-N0: has nothing more; lacks `Agent`, `ListAgents`, `ScheduleWakeup`, `Workflow`.
- H-N4 against H-N0: has nothing more; lacks `Agent`, `ListAgents`, `ScheduleWakeup`, `Workflow`.
- H-A0 against H-N0: has `mcp__aether__ask_user`, `mcp__aether__cancel_agent`, `mcp__aether__complete_task`, `mcp__aether__create_schedule`, `mcp__aether__delegate`, `mcp__aether__fail_task`, `mcp__aether__inspect_state`, `mcp__aether__integrate_workspace`, `mcp__aether__list_agents`, `mcp__aether__list_models`, `mcp__aether__list_schedules`, `mcp__aether__list_tasks`, `mcp__aether__permission`, `mcp__aether__read_artifact`, `mcp__aether__read_task`, `mcp__aether__remember`, `mcp__aether__request_checkpoint`, `mcp__aether__send_message`, `mcp__aether__update_schedule`, `mcp__aether__wait_for`; lacks `Agent`, `ListAgents`, `ScheduleWakeup`, `Workflow`.
- H-A2 against H-N0: has `mcp__aether__ask_user`, `mcp__aether__cancel_agent`, `mcp__aether__complete_task`, `mcp__aether__create_schedule`, `mcp__aether__delegate`, `mcp__aether__fail_task`, `mcp__aether__inspect_state`, `mcp__aether__integrate_workspace`, `mcp__aether__list_agents`, `mcp__aether__list_models`, `mcp__aether__list_schedules`, `mcp__aether__list_tasks`, `mcp__aether__permission`, `mcp__aether__read_artifact`, `mcp__aether__read_task`, `mcp__aether__remember`, `mcp__aether__request_checkpoint`, `mcp__aether__send_message`, `mcp__aether__update_schedule`, `mcp__aether__wait_for`; lacks `Agent`, `ListAgents`, `ScheduleWakeup`, `Workflow`.
- H-N0 always loads `Agent`, `Bash`, `Edit`, `ListAgents`, `Read`, `ReportFindings`, `ScheduleWakeup`, `Skill`, `ToolSearch`, `Workflow`, `Write`.

- **Under version 2 the two first requests are nearly the same size**:
  23,343 input tokens through Aether (H-A2) against 23,097 natively, 246
  apart. Under version 1 (H-A0) Aether's was 3,628 larger.
- **The flag takes out what it was meant to**: 57 deferred tool names (67
  against 10; 54 of them are the connectors' tools) and the 3,719
  characters of server instructions. No H-A2 transcript records any MCP
  server instructions.
- **Removing the coordination tools makes the native request 5,881 tokens
  smaller** (H-N3 against H-N0), the same in every block.

#### One line per arm

| Arm | n | First-request input tokens | Thinking tokens before the first edit | Thinking tokens, total | Output tokens | Uncached input tokens | API-equivalent USD | Wall seconds | Hidden checks passed (of 119) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 6 | 23,096.7 | 52,622.2 | 55,104.0 | 75,793.7 | 29.0 | 0.1378 | 316.7 | 118.3 |
| H-N3 | 6 | 17,215.3 | 55,624.3 | 62,434.8 | 87,666.3 | 38.0 | 0.1725 | 379.9 | 119.0 |
| H-N4 | 6 | 20,144.0 | 49,423.8 | 51,419.3 | 70,700.8 | 29.0 | 0.0879 | 301.4 | 118.7 |
| H-A0 | 6 | 26,724.3 | 45,111.3 | 46,486.8 | 65,070.2 | 23.0 | 0.0871 | 271.9 | 118.3 |
| H-A2 | 6 | 23,342.5 | 56,847.3 | 60,994.0 | 83,656.3 | 30.3 | 0.1726 | 353.4 | 119.0 |

#### Per arm, every trial

**First-request input tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 23,096 | 23,096 | 23,098 | 23,096 | 23,098 | 23,096 | 23,096.7 (6) |
| H-N3 | 17,214 | 17,214 | 17,216 | 17,214 | 17,216 | 17,218 | 17,215.3 (6) |
| H-N4 | 20,602 | 20,600 | 19,230 | 20,604 | 19,226 | 20,602 | 20,144.0 (6) |
| H-A0 | 26,721 | 26,718 | 26,722 | 26,731 | 26,727 | 26,727 | 26,724.3 (6) |
| H-A2 | 23,341 | 23,341 | 23,340 | 23,352 | 23,338 | 23,343 | 23,342.5 (6) |

**Thinking tokens before the first edit**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 41,686 | 48,894 | 46,447 | 51,117 | 61,571 | 66,018 | 52,622.2 (6) |
| H-N3 | 48,510 | 44,531 | 65,306 | 63,412 | 53,905 | 58,082 | 55,624.3 (6) |
| H-N4 | 42,237 | 54,747 | 43,611 | 54,415 | 54,924 | 46,609 | 49,423.8 (6) |
| H-A0 | 47,706 | 41,671 | 45,199 | 36,951 | 46,345 | 52,796 | 45,111.3 (6) |
| H-A2 | 74,585 | 55,628 | 64,372 | 43,097 | 44,885 | 58,517 | 56,847.3 (6) |

**Thinking tokens, total**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 42,898 | 50,328 | 47,111 | 52,641 | 66,040 | 71,606 | 55,104.0 (6) |
| H-N3 | 59,752 | 49,523 | 66,713 | 73,081 | 61,232 | 64,308 | 62,434.8 (6) |
| H-N4 | 44,178 | 56,984 | 46,364 | 55,400 | 56,564 | 49,026 | 51,419.3 (6) |
| H-A0 | 50,168 | 43,200 | 45,992 | 38,049 | 47,018 | 54,494 | 46,486.8 (6) |
| H-A2 | 81,111 | 58,063 | 65,793 | 43,872 | 50,303 | 66,822 | 60,994.0 (6) |

**Thinking tokens of the largest single response**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 41,657 | 48,789 | 46,433 | 51,048 | 61,319 | 65,988 | 52,539.0 (6) |
| H-N3 | 48,460 | 44,444 | 65,292 | 63,398 | 53,622 | 58,061 | 55,546.2 (6) |
| H-N4 | 42,209 | 54,709 | 43,597 | 54,401 | 54,910 | 46,575 | 49,400.2 (6) |
| H-A0 | 47,692 | 41,671 | 45,115 | 36,931 | 46,345 | 52,796 | 45,091.7 (6) |
| H-A2 | 74,532 | 55,517 | 64,360 | 43,068 | 44,871 | 58,503 | 56,808.5 (6) |

**Request that held the largest Thinking response**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 3 | 4 | 3 | 3 | 4 | 3 | 3.3 (6) |
| H-N3 | 3 | 3 | 3 | 3 | 4 | 3 | 3.2 (6) |
| H-N4 | 3 | 3 | 3 | 3 | 3 | 3 | 3.0 (6) |
| H-A0 | 3 | 3 | 3 | 3 | 3 | 3 | 3.0 (6) |
| H-A2 | 3 | 3 | 3 | 3 | 3 | 3 | 3.0 (6) |

**Output tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 61,996 | 69,064 | 65,598 | 74,777 | 88,773 | 94,554 | 75,793.7 (6) |
| H-N3 | 86,743 | 77,871 | 89,512 | 101,309 | 82,616 | 87,947 | 87,666.3 (6) |
| H-N4 | 62,339 | 78,631 | 66,542 | 73,834 | 75,027 | 67,832 | 70,700.8 (6) |
| H-A0 | 72,571 | 62,191 | 59,563 | 54,061 | 68,115 | 73,920 | 65,070.2 (6) |
| H-A2 | 108,155 | 79,158 | 89,091 | 61,071 | 71,876 | 92,587 | 83,656.3 (6) |

**Uncached input tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 22 | 26 | 22 | 30 | 40 | 34 | 29.0 (6) |
| H-N3 | 36 | 46 | 32 | 38 | 46 | 30 | 38.0 (6) |
| H-N4 | 24 | 36 | 34 | 24 | 22 | 34 | 29.0 (6) |
| H-A0 | 20 | 26 | 20 | 30 | 22 | 20 | 23.0 (6) |
| H-A2 | 38 | 28 | 24 | 30 | 28 | 34 | 30.3 (6) |

**Cache read tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 726,397 | 926,471 | 741,298 | 1,174,455 | 1,794,958 | 1,617,156 | 1,163,455.8 (6) |
| H-N3 | 1,442,677 | 1,847,589 | 1,432,553 | 1,802,572 | 1,813,568 | 1,212,064 | 1,591,837.2 (6) |
| H-N4 | 762,524 | 1,470,435 | 1,240,060 | 860,732 | 772,925 | 1,238,027 | 1,057,450.5 (6) |
| H-A0 | 734,932 | 929,615 | 657,744 | 1,005,570 | 807,165 | 734,985 | 811,668.5 (6) |
| H-A2 | 1,992,397 | 1,145,980 | 1,028,895 | 1,087,790 | 1,023,995 | 1,573,149 | 1,308,701.0 (6) |

**Cache write tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 87,058 | 97,650 | 87,771 | 99,171 | 114,631 | 118,986 | 100,877.8 (6) |
| H-N3 | 115,897 | 107,875 | 118,504 | 128,810 | 106,164 | 111,107 | 114,726.2 (6) |
| H-N4 | 87,465 | 105,930 | 94,163 | 99,882 | 99,425 | 94,423 | 96,881.3 (6) |
| H-A0 | 103,047 | 90,511 | 85,056 | 81,101 | 97,968 | 102,723 | 93,401.0 (6) |
| H-A2 | 131,043 | 104,064 | 115,646 | 86,390 | 94,816 | 117,244 | 108,200.5 (6) |

**API-equivalent USD**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 0.0557 | 0.1059 | 0.0578 | 0.1217 | 0.2156 | 0.2699 | 0.1378 (6) |
| H-N3 | 0.1574 | 0.1411 | 0.1898 | 0.2365 | 0.1558 | 0.1546 | 0.1725 (6) |
| H-N4 | 0.0563 | 0.1287 | 0.0645 | 0.1058 | 0.1069 | 0.0652 | 0.0879 (6) |
| H-A0 | 0.1202 | 0.0766 | 0.0534 | 0.0533 | 0.1074 | 0.1120 | 0.0871 (6) |
| H-A2 | 0.3311 | 0.1335 | 0.2310 | 0.0587 | 0.0932 | 0.1879 | 0.1726 (6) |

**Wall seconds**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 251.0 | 287.4 | 272.2 | 312.7 | 374.5 | 402.2 | 316.7 (6) |
| H-N3 | 355.7 | 336.2 | 379.9 | 477.0 | 346.2 | 384.3 | 379.9 (6) |
| H-N4 | 263.0 | 352.9 | 284.0 | 313.1 | 304.2 | 291.6 | 301.4 (6) |
| H-A0 | 289.9 | 261.5 | 252.3 | 244.9 | 274.9 | 307.8 | 271.9 (6) |
| H-A2 | 450.6 | 338.1 | 364.4 | 253.0 | 294.7 | 419.3 | 353.4 (6) |

**Hidden checks passed (of 119)**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 119 | 117 | 119 | 119 | 119 | 117 | 118.3 (6) |
| H-N3 | 119 | 119 | 119 | 119 | 119 | 119 | 119.0 (6) |
| H-N4 | 117 | 119 | 119 | 119 | 119 | 119 | 118.7 (6) |
| H-A0 | 119 | 117 | 117 | 119 | 119 | 119 | 118.3 (6) |
| H-A2 | 119 | 119 | 119 | 119 | 119 | 119 | 119.0 (6) |

**API requests**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 11 | 13 | 11 | 15 | 20 | 17 | 14.5 (6) |
| H-N3 | 18 | 23 | 16 | 19 | 23 | 15 | 19.0 (6) |
| H-N4 | 12 | 18 | 17 | 12 | 11 | 17 | 14.5 (6) |
| H-A0 | 10 | 13 | 10 | 15 | 11 | 10 | 11.5 (6) |
| H-A2 | 19 | 14 | 12 | 15 | 14 | 17 | 15.2 (6) |

**Test runs**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 3 | 4 | 3 | 3 | 9 | 5 | 4.5 (6) |
| H-N3 | 5 | 8 | 5 | 6 | 7 | 4 | 5.8 (6) |
| H-N4 | 3 | 6 | 5 | 4 | 2 | 5 | 4.2 (6) |
| H-A0 | 3 | 5 | 2 | 3 | 3 | 4 | 3.3 (6) |
| H-A2 | 5 | 3 | 5 | 4 | 3 | 6 | 4.3 (6) |

**Tests in the solver's last test run**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 41 | 35 | 46 | 45 | 40 | 34 | 40.2 (6) |
| H-N3 | 42 | 44 | 48 | 45 | 31 | 39 | 41.5 (6) |
| H-N4 | 36 | 45 | 30 | 41 | 36 | 28 | 36.0 (6) |
| H-A0 | 60 | 45 | 26 | 33 | 55 | 36 | 42.5 (6) |
| H-A2 | 61 | 35 | 56 | 37 | 1 | 46 | 39.3 (6) |

**Seconds from the last change to the end**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H-N0 | 18.2 | 21.7 | 9.0 | 20.1 | 19.1 | 35.6 | 20.6 (6) |
| H-N3 | 10.1 | 23.1 | 23.7 | 35.5 | 29.6 | 35.1 | 26.2 (6) |
| H-N4 | 9.6 | 20.6 | 27.1 | 18.2 | 23.6 | 24.3 | 20.6 (6) |
| H-A0 | 10.7 | 9.4 | 10.1 | 11.7 | 10.1 | 6.9 | 9.8 (6) |
| H-A2 | 15.1 | 28.0 | 15.2 | 6.9 | 19.9 | 56.8 | 23.6 (6) |

#### Contrasts, per block

Each table is the first arm minus the second in the same block.

**H-A2 minus H-N0** (the matched pair of this iteration)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean difference | Relative to H-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +245 | +245 | +242 | +256 | +240 | +247 | +245.8 | +1.1 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | +32,899 | +6,734 | +17,925 | -8,020 | -16,686 | -7,501 | +4,225.2 | +8.0 % | 3 / 3 / 0 |
| Thinking tokens, total | +38,213 | +7,735 | +18,682 | -8,769 | -15,737 | -4,784 | +5,890.0 | +10.7 % | 3 / 3 / 0 |
| Thinking tokens of the largest single response | +32,875 | +6,728 | +17,927 | -7,980 | -16,448 | -7,485 | +4,269.5 | +8.1 % | 3 / 3 / 0 |
| Output tokens | +46,159 | +10,094 | +23,493 | -13,706 | -16,897 | -1,967 | +7,862.7 | +10.4 % | 3 / 3 / 0 |
| Uncached input tokens | +16 | +2 | +2 | +0 | -12 | +0 | +1.3 | +4.6 % | 1 / 3 / 2 |
| API requests | +8 | +1 | +1 | +0 | -6 | +0 | +0.7 | +4.6 % | 1 / 3 / 2 |
| Test runs | +2 | -1 | +2 | +1 | -6 | +1 | -0.2 | -3.7 % | 2 / 4 / 0 |
| Tests in the solver's last test run | +20 | +0 | +10 | -8 | -39 | +12 | -0.8 | -2.1 % | 2 / 3 / 1 |
| Cache read tokens | +1,266,000 | +219,509 | +287,597 | -86,665 | -770,963 | -44,007 | +145,245.2 | +12.5 % | 3 / 3 / 0 |
| Cache write tokens | +43,985 | +6,414 | +27,875 | -12,781 | -19,815 | -1,742 | +7,322.7 | +7.3 % | 3 / 3 / 0 |
| API-equivalent USD | +0.2754 | +0.0277 | +0.1733 | -0.0630 | -0.1224 | -0.0820 | +0.0348 | +25.3 % | 3 / 3 / 0 |
| Wall seconds | +199.7 | +50.7 | +92.3 | -59.7 | -79.8 | +17.1 | +36.7 | +11.6 % | 2 / 4 / 0 |
| Hidden checks passed (of 119) | +0 | +2 | +0 | +0 | +0 | +2 | +0.7 | +0.6 % | 0 / 2 / 4 |

**H-A0 minus H-N0** (the pair as it started under protocol version 1, on the fixed binary)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean difference | Relative to H-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +3,625 | +3,622 | +3,624 | +3,635 | +3,629 | +3,631 | +3,627.7 | +15.7 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | +6,020 | -7,223 | -1,248 | -14,166 | -15,226 | -13,222 | -7,510.8 | -14.3 % | 5 / 1 / 0 |
| Thinking tokens, total | +7,270 | -7,128 | -1,119 | -14,592 | -19,022 | -17,112 | -8,617.2 | -15.6 % | 5 / 1 / 0 |
| Thinking tokens of the largest single response | +6,035 | -7,118 | -1,318 | -14,117 | -14,974 | -13,192 | -7,447.3 | -14.2 % | 5 / 1 / 0 |
| Output tokens | +10,575 | -6,873 | -6,035 | -20,716 | -20,658 | -20,634 | -10,723.5 | -14.1 % | 5 / 1 / 0 |
| Uncached input tokens | -2 | +0 | -2 | +0 | -18 | -14 | -6.0 | -20.7 % | 4 / 0 / 2 |
| API requests | -1 | +0 | -1 | +0 | -9 | -7 | -3.0 | -20.7 % | 4 / 0 / 2 |
| Test runs | +0 | +1 | -1 | +0 | -6 | -1 | -1.2 | -25.9 % | 3 / 1 / 2 |
| Tests in the solver's last test run | +19 | +10 | -20 | -12 | +15 | +2 | +2.3 | +5.8 % | 2 / 4 / 0 |
| Cache read tokens | +8,535 | +3,144 | -83,554 | -168,885 | -987,793 | -882,171 | -351,787.3 | -30.2 % | 4 / 2 / 0 |
| Cache write tokens | +15,989 | -7,139 | -2,715 | -18,070 | -16,663 | -16,263 | -7,476.8 | -7.4 % | 5 / 1 / 0 |
| API-equivalent USD | +0.0645 | -0.0293 | -0.0044 | -0.0684 | -0.1082 | -0.1579 | -0.0506 | -36.7 % | 5 / 1 / 0 |
| Wall seconds | +39.0 | -25.9 | -19.8 | -67.8 | -99.7 | -94.4 | -44.8 | -14.1 % | 5 / 1 / 0 |
| Hidden checks passed (of 119) | +0 | +0 | -2 | +0 | +0 | +2 | +0.0 | +0.0 % | 1 / 1 / 4 |

**H-N3 minus H-N0** (removing the CLI's coordination tools, natively)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean difference | Relative to H-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | -5,882 | -5,882 | -5,882 | -5,882 | -5,882 | -5,878 | -5,881.3 | -25.5 % | 6 / 0 / 0 |
| Thinking tokens before the first edit | +6,824 | -4,363 | +18,859 | +12,295 | -7,666 | -7,936 | +3,002.2 | +5.7 % | 3 / 3 / 0 |
| Thinking tokens, total | +16,854 | -805 | +19,602 | +20,440 | -4,808 | -7,298 | +7,330.8 | +13.3 % | 3 / 3 / 0 |
| Thinking tokens of the largest single response | +6,803 | -4,345 | +18,859 | +12,350 | -7,697 | -7,927 | +3,007.2 | +5.7 % | 3 / 3 / 0 |
| Output tokens | +24,747 | +8,807 | +23,914 | +26,532 | -6,157 | -6,607 | +11,872.7 | +15.7 % | 2 / 4 / 0 |
| Uncached input tokens | +14 | +20 | +10 | +8 | +6 | -4 | +9.0 | +31.0 % | 1 / 5 / 0 |
| API requests | +7 | +10 | +5 | +4 | +3 | -2 | +4.5 | +31.0 % | 1 / 5 / 0 |
| Test runs | +2 | +4 | +2 | +3 | -2 | -1 | +1.3 | +29.6 % | 2 / 4 / 0 |
| Tests in the solver's last test run | +1 | +9 | +2 | +0 | -9 | +5 | +1.3 | +3.3 % | 1 / 4 / 1 |
| Cache read tokens | +716,280 | +921,118 | +691,255 | +628,117 | +18,610 | -405,092 | +428,381.3 | +36.8 % | 1 / 5 / 0 |
| Cache write tokens | +28,839 | +10,225 | +30,733 | +29,639 | -8,467 | -7,879 | +13,848.3 | +13.7 % | 2 / 4 / 0 |
| API-equivalent USD | +0.1017 | +0.0353 | +0.1320 | +0.1148 | -0.0598 | -0.1153 | +0.0348 | +25.2 % | 2 / 4 / 0 |
| Wall seconds | +104.7 | +48.8 | +107.8 | +164.2 | -28.4 | -17.9 | +63.2 | +20.0 % | 2 / 4 / 0 |
| Hidden checks passed (of 119) | +0 | +2 | +0 | +0 | +0 | +2 | +0.7 | +0.6 % | 0 / 2 / 4 |

**H-N4 minus H-N3** (loading the connectors, natively)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean difference | Relative to H-N3 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +3,388 | +3,386 | +2,014 | +3,390 | +2,010 | +3,384 | +2,928.7 | +17.0 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | -6,273 | +10,216 | -21,695 | -8,997 | +1,019 | -11,473 | -6,200.5 | -11.1 % | 4 / 2 / 0 |
| Thinking tokens, total | -15,574 | +7,461 | -20,349 | -17,681 | -4,668 | -15,282 | -11,015.5 | -17.6 % | 5 / 1 / 0 |
| Thinking tokens of the largest single response | -6,251 | +10,265 | -21,695 | -8,997 | +1,288 | -11,486 | -6,146.0 | -11.1 % | 4 / 2 / 0 |
| Output tokens | -24,404 | +760 | -22,970 | -27,475 | -7,589 | -20,115 | -16,965.5 | -19.4 % | 5 / 1 / 0 |
| Uncached input tokens | -12 | -10 | +2 | -14 | -24 | +4 | -9.0 | -23.7 % | 4 / 2 / 0 |
| API requests | -6 | -5 | +1 | -7 | -12 | +2 | -4.5 | -23.7 % | 4 / 2 / 0 |
| Test runs | -2 | -2 | +0 | -2 | -5 | +1 | -1.7 | -28.6 % | 4 / 1 / 1 |
| Tests in the solver's last test run | -6 | +1 | -18 | -4 | +5 | -11 | -5.5 | -13.3 % | 4 / 2 / 0 |
| Cache read tokens | -680,153 | -377,154 | -192,493 | -941,840 | -1,040,643 | +25,963 | -534,386.7 | -33.6 % | 5 / 1 / 0 |
| Cache write tokens | -28,432 | -1,945 | -24,341 | -28,928 | -6,739 | -16,684 | -17,844.8 | -15.6 % | 6 / 0 / 0 |
| API-equivalent USD | -0.1011 | -0.0125 | -0.1253 | -0.1307 | -0.0489 | -0.0894 | -0.0847 | -49.1 % | 6 / 0 / 0 |
| Wall seconds | -92.7 | +16.6 | -95.9 | -163.8 | -42.0 | -92.7 | -78.4 | -20.6 % | 5 / 1 / 0 |
| Hidden checks passed (of 119) | -2 | +0 | +0 | +0 | +0 | +0 | -0.3 | -0.3 % | 1 / 0 / 5 |

**H-A0 minus H-A2** (loading the connectors, through Aether)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean difference | Relative to H-A2 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +3,380 | +3,377 | +3,382 | +3,379 | +3,389 | +3,384 | +3,381.8 | +14.5 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | -26,879 | -13,957 | -19,173 | -6,146 | +1,460 | -5,721 | -11,736.0 | -20.6 % | 5 / 1 / 0 |
| Thinking tokens, total | -30,943 | -14,863 | -19,801 | -5,823 | -3,285 | -12,328 | -14,507.2 | -23.8 % | 6 / 0 / 0 |
| Thinking tokens of the largest single response | -26,840 | -13,846 | -19,245 | -6,137 | +1,474 | -5,707 | -11,716.8 | -20.6 % | 5 / 1 / 0 |
| Output tokens | -35,584 | -16,967 | -29,528 | -7,010 | -3,761 | -18,667 | -18,586.2 | -22.2 % | 6 / 0 / 0 |
| Uncached input tokens | -18 | -2 | -4 | +0 | -6 | -14 | -7.3 | -24.2 % | 5 / 0 / 1 |
| API requests | -9 | -1 | -2 | +0 | -3 | -7 | -3.7 | -24.2 % | 5 / 0 / 1 |
| Test runs | -2 | +2 | -3 | -1 | +0 | -2 | -1.0 | -23.1 % | 4 / 1 / 1 |
| Tests in the solver's last test run | -1 | +10 | -30 | -4 | +54 | -10 | +3.2 | +8.1 % | 4 / 2 / 0 |
| Cache read tokens | -1,257,465 | -216,365 | -371,151 | -82,220 | -216,830 | -838,164 | -497,032.5 | -38.0 % | 6 / 0 / 0 |
| Cache write tokens | -27,996 | -13,553 | -30,590 | -5,289 | +3,152 | -14,521 | -14,799.5 | -13.7 % | 5 / 1 / 0 |
| API-equivalent USD | -0.2109 | -0.0569 | -0.1777 | -0.0054 | +0.0142 | -0.0759 | -0.0854 | -49.5 % | 5 / 1 / 0 |
| Wall seconds | -160.7 | -76.6 | -112.1 | -8.1 | -19.9 | -111.4 | -81.5 | -23.1 % | 6 / 0 / 0 |
| Hidden checks passed (of 119) | +0 | -2 | -2 | +0 | +0 | +0 | -0.7 | -0.6 % | 2 / 0 / 4 |

**H-A2 minus H-N3** (Aether against native with the same tools removed and no connectors)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean difference | Relative to H-N3 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +6,127 | +6,127 | +6,124 | +6,138 | +6,122 | +6,125 | +6,127.2 | +35.6 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | +26,075 | +11,097 | -934 | -20,315 | -9,020 | +435 | +1,223.0 | +2.2 % | 3 / 3 / 0 |
| Thinking tokens, total | +21,359 | +8,540 | -920 | -29,209 | -10,929 | +2,514 | -1,440.8 | -2.3 % | 3 / 3 / 0 |
| Thinking tokens of the largest single response | +26,072 | +11,073 | -932 | -20,330 | -8,751 | +442 | +1,262.3 | +2.3 % | 3 / 3 / 0 |
| Output tokens | +21,412 | +1,287 | -421 | -40,238 | -10,740 | +4,640 | -4,010.0 | -4.6 % | 3 / 3 / 0 |
| Uncached input tokens | +2 | -18 | -8 | -8 | -18 | +4 | -7.7 | -20.2 % | 4 / 2 / 0 |
| API requests | +1 | -9 | -4 | -4 | -9 | +2 | -3.8 | -20.2 % | 4 / 2 / 0 |
| Test runs | +0 | -5 | +0 | -2 | -4 | +2 | -1.5 | -25.7 % | 3 / 1 / 2 |
| Tests in the solver's last test run | +19 | -9 | +8 | -8 | -30 | +7 | -2.2 | -5.2 % | 3 / 3 / 0 |
| Cache read tokens | +549,720 | -701,609 | -403,658 | -714,782 | -789,573 | +361,085 | -283,136.2 | -17.8 % | 4 / 2 / 0 |
| Cache write tokens | +15,146 | -3,811 | -2,858 | -42,420 | -11,348 | +6,137 | -6,525.7 | -5.7 % | 4 / 2 / 0 |
| API-equivalent USD | +0.1737 | -0.0076 | +0.0412 | -0.1778 | -0.0626 | +0.0333 | +0.0000 | +0.0 % | 3 / 3 / 0 |
| Wall seconds | +94.9 | +1.9 | -15.5 | -223.9 | -51.4 | +35.0 | -26.5 | -7.0 % | 3 / 3 / 0 |
| Hidden checks passed (of 119) | +0 | +0 | +0 | +0 | +0 | +0 | +0.0 | +0.0 % | 0 / 0 / 6 |

**H-A0 minus H-N4** (Aether against native with the same tools removed and the connectors)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean difference | Relative to H-N4 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +6,119 | +6,118 | +7,492 | +6,127 | +7,501 | +6,125 | +6,580.3 | +32.7 % | 0 / 6 / 0 |
| Thinking tokens before the first edit | +5,469 | -13,076 | +1,588 | -17,464 | -8,579 | +6,187 | -4,312.5 | -8.7 % | 3 / 3 / 0 |
| Thinking tokens, total | +5,990 | -13,784 | -372 | -17,351 | -9,546 | +5,468 | -4,932.5 | -9.6 % | 4 / 2 / 0 |
| Thinking tokens of the largest single response | +5,483 | -13,038 | +1,518 | -17,470 | -8,565 | +6,221 | -4,308.5 | -8.7 % | 3 / 3 / 0 |
| Output tokens | +10,232 | -16,440 | -6,979 | -19,773 | -6,912 | +6,088 | -5,630.7 | -8.0 % | 4 / 2 / 0 |
| Uncached input tokens | -4 | -10 | -14 | +6 | +0 | -14 | -6.0 | -20.7 % | 4 / 1 / 1 |
| API requests | -2 | -5 | -7 | +3 | +0 | -7 | -3.0 | -20.7 % | 4 / 1 / 1 |
| Test runs | +0 | -1 | -3 | -1 | +1 | -1 | -0.8 | -20.0 % | 4 / 1 / 1 |
| Tests in the solver's last test run | +24 | +0 | -4 | -8 | +19 | +8 | +6.5 | +18.1 % | 2 / 3 / 1 |
| Cache read tokens | -27,592 | -540,820 | -582,316 | +144,838 | +34,240 | -503,042 | -245,782.0 | -23.2 % | 4 / 2 / 0 |
| Cache write tokens | +15,582 | -15,419 | -9,107 | -18,781 | -1,457 | +8,300 | -3,480.3 | -3.6 % | 4 / 2 / 0 |
| API-equivalent USD | +0.0639 | -0.0521 | -0.0111 | -0.0525 | +0.0005 | +0.0468 | -0.0007 | -0.8 % | 3 / 3 / 0 |
| Wall seconds | +26.9 | -91.4 | -31.7 | -68.2 | -29.3 | +16.3 | -29.6 | -9.8 % | 4 / 2 / 0 |
| Hidden checks passed (of 119) | +2 | -2 | -2 | +0 | +0 | +0 | -0.3 | -0.3 % | 2 / 1 / 3 |

#### The single changes side by side

**Thinking tokens of the largest single response**

| Difference | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The matched pair: H-A2 minus H-N0 | +32,875 | +6,728 | +17,927 | -7,980 | -16,448 | -7,485 | +4,270 | 3 / 3 / 0 |
| The version 1 start: H-A0 minus H-N0 | +6,035 | -7,118 | -1,318 | -14,117 | -14,974 | -13,192 | -7,447 | 5 / 1 / 0 |
| Coordination tools removed, natively: H-N3 minus H-N0 | +6,803 | -4,345 | +18,859 | +12,350 | -7,697 | -7,927 | +3,007 | 3 / 3 / 0 |
| Connectors loaded, natively: H-N4 minus H-N3 | -6,251 | +10,265 | -21,695 | -8,997 | +1,288 | -11,486 | -6,146 | 4 / 2 / 0 |
| Connectors loaded, through Aether: H-A0 minus H-A2 | -26,840 | -13,846 | -19,245 | -6,137 | +1,474 | -5,707 | -11,717 | 5 / 1 / 0 |
| Aether against native, same tools removed, no connectors: H-A2 minus H-N3 | +26,072 | +11,073 | -932 | -20,330 | -8,751 | +442 | +1,262 | 3 / 3 / 0 |

**Thinking tokens, total**

| Difference | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The matched pair: H-A2 minus H-N0 | +38,213 | +7,735 | +18,682 | -8,769 | -15,737 | -4,784 | +5,890 | 3 / 3 / 0 |
| The version 1 start: H-A0 minus H-N0 | +7,270 | -7,128 | -1,119 | -14,592 | -19,022 | -17,112 | -8,617 | 5 / 1 / 0 |
| Coordination tools removed, natively: H-N3 minus H-N0 | +16,854 | -805 | +19,602 | +20,440 | -4,808 | -7,298 | +7,331 | 3 / 3 / 0 |
| Connectors loaded, natively: H-N4 minus H-N3 | -15,574 | +7,461 | -20,349 | -17,681 | -4,668 | -15,282 | -11,016 | 5 / 1 / 0 |
| Connectors loaded, through Aether: H-A0 minus H-A2 | -30,943 | -14,863 | -19,801 | -5,823 | -3,285 | -12,328 | -14,507 | 6 / 0 / 0 |
| Aether against native, same tools removed, no connectors: H-A2 minus H-N3 | +21,359 | +8,540 | -920 | -29,209 | -10,929 | +2,514 | -1,441 | 3 / 3 / 0 |

**Output tokens**

| Difference | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The matched pair: H-A2 minus H-N0 | +46,159 | +10,094 | +23,493 | -13,706 | -16,897 | -1,967 | +7,863 | 3 / 3 / 0 |
| The version 1 start: H-A0 minus H-N0 | +10,575 | -6,873 | -6,035 | -20,716 | -20,658 | -20,634 | -10,724 | 5 / 1 / 0 |
| Coordination tools removed, natively: H-N3 minus H-N0 | +24,747 | +8,807 | +23,914 | +26,532 | -6,157 | -6,607 | +11,873 | 2 / 4 / 0 |
| Connectors loaded, natively: H-N4 minus H-N3 | -24,404 | +760 | -22,970 | -27,475 | -7,589 | -20,115 | -16,966 | 5 / 1 / 0 |
| Connectors loaded, through Aether: H-A0 minus H-A2 | -35,584 | -16,967 | -29,528 | -7,010 | -3,761 | -18,667 | -18,586 | 6 / 0 / 0 |
| Aether against native, same tools removed, no connectors: H-A2 minus H-N3 | +21,412 | +1,287 | -421 | -40,238 | -10,740 | +4,640 | -4,010 | 3 / 3 / 0 |

#### Against iteration 1

The two iterations ran about an hour apart. The native cell (H-N0) is the same
treatment in both, so its two columns show how much a mean of 6 moves
between sittings with nothing changed.

| Measure | H-N0, iteration 1 | H-N0, iteration 2 | H-A0, iteration 1 (with the words) | H-A0, iteration 2 (without the words) | H-A2, iteration 2 | Gap in iteration 1: H-A0 minus H-N0 | Same gap in iteration 2 | Matched gap in iteration 2: H-A2 minus H-N0 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Thinking tokens before the first edit | 55,893 (6) | 52,622 (6) | 41,000 (5) | 45,111 (6) | 56,847 (6) | -14,893 | -7,511 | +4,225 |
| Thinking tokens, total | 59,409 (6) | 55,104 (6) | 42,332 (6) | 46,487 (6) | 60,994 (6) | -17,077 | -8,617 | +5,890 |
| Thinking tokens of the largest single response | 55,853 (6) | 52,539 (6) | 41,484 (6) | 45,092 (6) | 56,808 (6) | -14,370 | -7,447 | +4,270 |
| Output tokens | 81,209 (6) | 75,794 (6) | 59,442 (6) | 65,070 (6) | 83,656 (6) | -21,767 | -10,724 | +7,863 |
| API-equivalent USD | 0.1736 (6) | 0.1378 (6) | 0.0592 (6) | 0.0871 (6) | 0.1726 (6) | -0.1144 | -0.0506 | +0.0348 |
| Wall seconds | 340.0 (6) | 316.7 (6) | 245.1 (6) | 271.9 (6) | 353.4 (6) | -94.9 | -44.8 | +36.7 |
| Hidden checks passed (of 119) | 119.0 (6) | 118.3 (6) | 118.7 (6) | 118.3 (6) | 119.0 (6) | -0.3 | +0.0 | +0.7 |

Read across the two sittings, with the connectors loading as they did in
iteration 1:

- **Aether without the words (H-A0) thinks more than it did with them**:
  46,487 thinking tokens against 42,332, and 65,070 output tokens against
  59,442.
- **The native cell moved too, with nothing changed**: 55,104 against
  59,409 thinking tokens, 75,794 against 81,209 output tokens. A mean of 6
  moves by about 4,300 thinking tokens from one sitting to the next.
- **The gap is about half of what it was**: from -17,077 to -8,617 thinking
  tokens and from -21,767 to -10,724 output tokens. It closed by 8,460 and
  11,043. Iteration 1 measured the words natively at -10,072 and -12,983,
  so the sizes agree.
- **How much of the closing is the fix cannot be told more exactly.** About
  half of it is Aether moving up and half is the native cell moving down,
  and the two sittings are not one block. No arm of iteration 2 carries the
  words, because the binary no longer writes them.
- Hidden checks did not move: 118.3 against 118.7 for Aether, 118.3 against
  119.0 natively.

#### Where the input tokens differ

**H-A2 minus H-N0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 11 | 19 vs 11 | 23,341 vs 23,096 | 2,123,478 vs 813,477 | +1,310,001 | +2,695 | +186,728 | +1,120,578 |
| 12 | 14 vs 13 | 23,341 vs 23,096 | 1,250,072 vs 1,024,147 | +225,925 | +3,185 | +23,341 | +199,399 |
| 13 | 12 vs 11 | 23,340 vs 23,098 | 1,144,565 vs 829,091 | +315,474 | +2,662 | +23,340 | +289,472 |
| 14 | 15 vs 15 | 23,352 vs 23,096 | 1,174,210 vs 1,273,656 | -99,446 | +3,840 | +0 | -103,286 |
| 15 | 14 vs 20 | 23,338 vs 23,098 | 1,118,839 vs 1,909,629 | -790,790 | +4,800 | -140,028 | -655,562 |
| 16 | 17 vs 17 | 23,343 vs 23,096 | 1,690,427 vs 1,736,176 | -45,749 | +4,199 | +0 | -49,948 |
| Mean (6) | | | | +152,569 | +3,564 | +15,564 | +133,442 |

#### The stop rule for Haiku

**The stop rule on H-A2 against H-N0, 6 pairs**

| Measure | Native mean | Aether mean | Mean difference | Relative to native | Aether lower / higher / equal | Part 1: not one-sided | Part 2: within 5 % |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| Output tokens | 75,793.7 | 83,656.3 | +7,862.7 | +10.4 % | 3 / 3 / 0 | met | not met |
| Uncached input tokens | 29.0 | 30.3 | +1.3 | +4.6 % | 1 / 3 / 2 | not part of it | met |
| API-equivalent USD | 0.1378 | 0.1726 | +0.0348 | +25.3 % | 3 / 3 / 0 | met | not met |
| Hidden checks passed (of 119) | 118.3 | 119.0 | +0.7 | +0.6 % | 0 / 2 / 4 | met | not part of it |

- **Part 1, not one-sided: met on all three measures.** Output tokens are
  lower through Aether in 3 of 6 pairs and higher in 3; cost is lower in 3
  and higher in 3; hidden checks are lower in none, higher in 2 and equal in
  4. In iteration 1 the counted pair was lower through Aether in 6 of 6 on
  output tokens.
- **Part 2, mean difference within 5 % of the native mean: not met on
  output tokens and cost, met on uncached input tokens.** Output tokens
  +10.4 % (83,656 against 75,794); cost +25.3 % (0.1726 against 0.1378
  USD); uncached input tokens +4.6 % (30.3 against 29.0). The last carries
  no weight: it is 30 tokens a trial, because nearly all of Haiku's input is
  read from or written to the cache.
- **The stop rule is not met for Haiku.**

How much weight part 2 can bear here: the difference in a single pair runs
from -16,897 to +46,159 output tokens. Block 11 alone carries the mean:
the other five pairs average +203. Six pairs cannot place a mean within
5 % of 75,794, which is 3,790 tokens. "Not met" is the honest reading of
the rule; it does not show that Aether now costs Haiku tokens.

#### Verdict for Haiku, iteration 2

**With the words gone and both sides starting equal, Haiku through Aether
is no longer below native: the matched pair is 3 blocks each way, with
Aether 10 % above native in output tokens in the mean. The half that
iteration 1 could not place is likely the connectors.**

What is measured:

1. **Loading the operator's connectors lowers Haiku's thinking and
   output.** Through Aether (H-A0 against H-A2): -14,507 thinking tokens
   and -18,586 output tokens, lower in 6 of 6 blocks. Natively (H-N4
   against H-N3): -11,016 and -16,966, lower in 5 of 6. Under version 1
   Aether loaded them and the native side did not. Iteration 1 measured the
   unplaced half at -9,093 thinking tokens and -11,046 output tokens.
2. **It is not the coordination tools, at least not in the direction
   guessed.** Iteration 1 ranked first the guess that the tools the native
   CLI loads make it think more. Removing them natively (H-N3 against H-N0)
   did not lower thinking: +7,331 tokens, lower in 3 blocks and higher in
   3; output +11,873, lower in 2 and higher in 4. If they act at all, they
   act the other way.
3. **With the same tools removed and no connectors, Aether and native are
   level** (H-A2 against H-N3): -1,441 thinking tokens and -4,010 output
   tokens, lower in 3 blocks and higher in 3. Aether's own 20 tools and its
   envelope show no effect, as the two-tool arm found in iteration 1.
4. **The matched pair** (H-A2 against H-N0): +5,890 thinking tokens,
   +7,863 output tokens and +0.0348 USD, each lower in 3 blocks and higher
   in 3. It is the sum of points 2 and 3.
5. **Hidden checks are at the ceiling in every arm**: 118.3 to 119.0 of
   119. Of 30 trials, 25 passed all 119 and 5 passed 117. H-A2 passed 119
   in 6 of 6.

What is hypothesis:

- **Why the connectors lower thinking.** They add 57 tool names and 3,719
  characters of instructions that have nothing to do with the task. One
  reading is that Haiku thinks less before acting the more tools it is
  shown; the coordination tools would fit it (4 loaded tools and 5 deferred
  names fewer, more output). No arm varies how many tools are shown without
  also varying which, so this is not tested.
- **Whether the 10 % of the matched pair is a cost of Aether.** In the mean
  it is what removing the coordination tools does natively, and neither
  part of it is consistent across blocks.

How sure. That loading the connectors lowers Haiku's output: likely. Two
separate contrasts agree, 11 of 12 blocks one way. That the connectors are
the whole of the half iteration 1 left open: the sizes agree, and 6 blocks
do not pin them down. That removing the coordination tools raises output:
not shown, 4 of 6. That Aether is now above native: not shown, 3 of 6. That
Aether is no longer consistently below native: yes, as far as 6 blocks go.

What 6 blocks do not show:

- **Whether thinking more buys accuracy.** Every arm is at the ceiling on
  this case.
- **Anything about another case or another effort.**
- **What a counted run under version 2 would find.** None had been made
  when this was written; see [Iteration 3](#iteration-3).

### Luna, iteration 2

#### What the first turn was given

**What the first turn was given (characters per block, as the rollout recorded it)**

| Block of context | L-N0 | L-A0 | L-A1 |
| --- | ---: | ---: | ---: |
| `developer: <collaboration_mode># Collab` | 920 | 920 | 920 |
| `developer: <multi_agent_mode>Any earlie` | 271 | 0 | 0 |
| `developer: <multi_agent_role>You are `/` | 2,400 | 0 | 0 |
| `developer: <permissions instructions>` | 362 | 6,144 | 6,144 |
| `developer: <recommended_plugins>` | 901 | 901 | 901 |
| `developer: <skills_instructions>` | 4,751 to 8,030 | 8,030 | 8,030 |
| `user: # AGENTS.md instructions` | 5,884 | 5,884 | 5,884 |
| `user: <environment_context>` | 575 | 575 | 575 |
| `user: the task` | 789 | 0 | 0 |
| `user: the task (in the Aether envelope)` | 0 | 1,467 | 1,467 |
| Total | 16,853 to 20,132 | 23,921 | 23,921 |

#### One line per arm

| Arm | n | First-request input tokens | Reasoning tokens before the first edit | Reasoning tokens, total | Output tokens | Uncached input tokens | API-equivalent USD | Wall seconds | Hidden checks passed (of 119) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 10 | 15,705.9 | 68.0 (8 known) | 1,992.6 | 12,047.8 | 39,719.2 | 0.0164 | 141.7 | 88.0 |
| L-A0 | 10 | 16,557.6 | 0.0 | 39.5 | 10,972.2 | 49,473.0 | 0.0185 | 113.4 | 79.0 |
| L-A1 | 10 | 16,556.5 | 0.0 (8 known) | 489.5 | 11,872.1 | 52,468.4 | 0.0198 | 248.2 | 85.2 |

#### Per arm, every trial

**First-request input tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 15,630 | 15,630 | 15,628 | 15,628 | 16,403 | 15,626 | 15,628 | 15,626 | 15,630 | 15,630 | 15,705.9 (10) |
| L-A0 | 16,557 | 16,558 | 16,567 | 16,559 | 16,559 | 16,565 | 16,553 | 16,556 | 16,551 | 16,551 | 16,557.6 (10) |
| L-A1 | 16,550 | 16,557 | 16,562 | 16,560 | 16,548 | 16,558 | 16,555 | 16,565 | 16,557 | 16,553 | 16,556.5 (10) |

**Reasoning tokens before the first edit**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | unknown | 0 | 0 | 0 | 0 | 0 | 544 | 0 | unknown | 0 | 68.0 (8) |
| L-A0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0.0 (10) |
| L-A1 | 0 | 0 | 0 | 0 | unknown | 0 | unknown | 0 | 0 | 0 | 0.0 (8) |

**Reasoning tokens, total**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 479 | 4,104 | 0 | 0 | 10,229 | 0 | 1,882 | 3,232 | 0 | 0 | 1,992.6 (10) |
| L-A0 | 0 | 0 | 0 | 0 | 395 | 0 | 0 | 0 | 0 | 0 | 39.5 (10) |
| L-A1 | 0 | 522 | 1,946 | 0 | 0 | 0 | 0 | 0 | 2,427 | 0 | 489.5 (10) |

**Output tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 7,484 | 12,974 | 13,813 | 8,422 | 24,937 | 8,263 | 10,227 | 16,078 | 5,704 | 12,576 | 12,047.8 (10) |
| L-A0 | 9,780 | 12,016 | 12,620 | 8,682 | 12,111 | 11,549 | 9,897 | 8,441 | 13,107 | 11,519 | 10,972.2 (10) |
| L-A1 | 16,260 | 10,526 | 13,242 | 11,907 | 9,329 | 12,943 | 10,757 | 8,372 | 17,466 | 7,919 | 11,872.1 (10) |

**Uncached input tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 26,160 | 44,044 | 58,773 | 34,120 | 68,892 | 20,330 | 22,810 | 52,911 | 17,454 | 51,698 | 39,719.2 (10) |
| L-A0 | 33,714 | 55,749 | 65,433 | 49,413 | 48,690 | 56,334 | 37,667 | 46,347 | 59,628 | 41,755 | 49,473.0 (10) |
| L-A1 | 69,754 | 77,994 | 55,053 | 85,204 | 32,872 | 49,992 | 31,971 | 32,372 | 64,592 | 24,880 | 52,468.4 (10) |

**Cache read tokens**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 236,800 | 601,344 | 1,110,784 | 471,296 | 1,170,176 | 244,992 | 223,744 | 1,136,384 | 162,816 | 1,044,480 | 640,281.6 (10) |
| L-A0 | 309,760 | 1,037,312 | 1,068,544 | 560,896 | 972,800 | 1,165,056 | 625,920 | 348,416 | 1,268,736 | 688,128 | 804,556.8 (10) |
| L-A1 | 2,112,768 | 600,320 | 818,944 | 1,495,808 | 405,760 | 740,352 | 356,864 | 540,672 | 1,266,176 | 236,032 | 857,369.6 (10) |

**Input tokens over all requests (uncached + cache read + cache write)**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 262,960 | 645,388 | 1,169,557 | 505,416 | 1,239,068 | 265,322 | 246,554 | 1,189,295 | 180,270 | 1,096,178 | 680,000.8 (10) |
| L-A0 | 343,474 | 1,093,061 | 1,133,977 | 610,309 | 1,021,490 | 1,221,390 | 663,587 | 394,763 | 1,328,364 | 729,883 | 854,029.8 (10) |
| L-A1 | 2,182,522 | 678,314 | 873,997 | 1,581,012 | 438,632 | 790,344 | 388,835 | 573,044 | 1,330,768 | 260,912 | 909,838.0 (10) |

**API-equivalent USD**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0.0087 | 0.0169 | 0.0239 | 0.0123 | 0.0311 | 0.0086 | 0.0096 | 0.0247 | 0.0062 | 0.0219 | 0.0164 (10) |
| L-A0 | 0.0114 | 0.0220 | 0.0235 | 0.0149 | 0.0207 | 0.0231 | 0.0150 | 0.0123 | 0.0252 | 0.0168 | 0.0185 (10) |
| L-A1 | 0.0362 | 0.0191 | 0.0203 | 0.0294 | 0.0120 | 0.0189 | 0.0121 | 0.0128 | 0.0279 | 0.0088 | 0.0198 (10) |

**Wall seconds**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 67.4 | 282.2 | 125.8 | 70.2 | 187.1 | 67.1 | 72.6 | 148.8 | 119.4 | 276.1 | 141.7 (10) |
| L-A0 | 75.7 | 127.4 | 113.1 | 77.6 | 108.9 | 114.2 | 85.8 | 64.9 | 113.2 | 253.2 | 113.4 (10) |
| L-A1 | 179.5 | 91.1 | 120.7 | 1,253.8 | 194.8 | 276.5 | 76.0 | 74.6 | 154.8 | 60.0 | 248.2 (10) |

**Hidden checks passed (of 119)**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 99 | 109 | 112 | 99 | 114 | 0 | 110 | 119 | 0 | 118 | 88.0 (10) |
| L-A0 | 0 | 98 | 114 | 87 | 113 | 97 | 61 | 0 | 107 | 113 | 79.0 (10) |
| L-A1 | 111 | 0 | 108 | 0 | 117 | 92 | 115 | 116 | 111 | 82 | 85.2 (10) |

**API requests**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 11 | 23 | 37 | 19 | 33 | 11 | 10 | 37 | 8 | 36 | 22.5 (10) |
| L-A0 | 14 | 36 | 34 | 21 | 32 | 37 | 23 | 15 | 40 | 24 | 27.6 (10) |
| L-A1 | 59 | 23 | 29 | 49 | 15 | 24 | 14 | 20 | 38 | 10 | 28.1 (10) |

**Test runs**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 3 | 2 | 6 | 4 | 5 | 2 | 3 | 6 | 2 | 4 | 3.7 (10) |
| L-A0 | 2 | 10 | 5 | 0 | 7 | 6 | 5 | 0 | 8 | 5 | 4.8 (10) |
| L-A1 | 10 | 6 | 4 | 6 | 6 | 5 | 1 | 4 | 7 | 4 | 5.3 (10) |

**Tests in the solver's last test run**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 4 | 4 | 4 | 4 | 4 | 4 | 4 | 4 | 4 | 10 | 4.6 (10) |
| L-A0 | 4 | 4 | 10 | unknown | 11 | 4 | 4 | unknown | 4 | 10 | 6.4 (8) |
| L-A1 | 4 | 4 | 4 | 4 | 4 | 4 | unknown | 4 | 4 | 4 | 4.0 (9) |

**Seconds from the last change to the end**

| Arm | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | unknown | 46.9 | 15.6 | 5.9 | 7.2 | 6.8 | 13.1 | 19.7 | unknown | 15.5 | 16.3 (8) |
| L-A0 | 12.2 | 11.9 | 6.8 | 4.5 | 6.2 | 18.4 | 5.0 | 6.5 | 6.6 | 14.2 | 9.2 (10) |
| L-A1 | 8.7 | 5.2 | 10.8 | 587.3 | unknown | 37.1 | unknown | 7.7 | 5.1 | 5.8 | 83.5 (8) |

#### Contrasts, per block

**L-A0 minus L-N0** (the matched pair of this iteration)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean difference | Relative to L-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +927 | +928 | +939 | +931 | +156 | +939 | +925 | +930 | +921 | +921 | +851.7 | +5.4 % | 0 / 10 / 0 |
| API requests | +3 | +13 | -3 | +2 | -1 | +26 | +13 | -22 | +32 | -12 | +5.1 | +22.7 % | 4 / 6 / 0 |
| Uncached input tokens | +7,554 | +11,705 | +6,660 | +15,293 | -20,202 | +36,004 | +14,857 | -6,564 | +42,174 | -9,943 | +9,753.8 | +24.6 % | 3 / 7 / 0 |
| Cache read tokens | +72,960 | +435,968 | -42,240 | +89,600 | -197,376 | +920,064 | +402,176 | -787,968 | +1,105,920 | -356,352 | +164,275.2 | +25.7 % | 4 / 6 / 0 |
| Input tokens over all requests (uncached + cache read + cache write) | +80,514 | +447,673 | -35,580 | +104,893 | -217,578 | +956,068 | +417,033 | -794,532 | +1,148,094 | -366,295 | +174,029.0 | +25.6 % | 4 / 6 / 0 |
| Output tokens | +2,296 | -958 | -1,193 | +260 | -12,826 | +3,286 | -330 | -7,637 | +7,403 | -1,057 | -1,075.6 | -8.9 % | 6 / 4 / 0 |
| Reasoning tokens, total | -479 | -4,104 | +0 | +0 | -9,834 | +0 | -1,882 | -3,232 | +0 | +0 | -1,953.1 | -98.0 % | 5 / 0 / 5 |
| API-equivalent USD | +0.0026 | +0.0051 | -0.0004 | +0.0026 | -0.0104 | +0.0144 | +0.0053 | -0.0124 | +0.0190 | -0.0051 | +0.0021 | +12.7 % | 4 / 6 / 0 |
| Wall seconds | +8.3 | -154.8 | -12.7 | +7.5 | -78.2 | +47.0 | +13.2 | -83.9 | -6.2 | -23.0 | -28.3 | -20.0 % | 6 / 4 / 0 |
| Hidden checks passed (of 119) | -99 | -11 | +2 | -12 | -1 | +97 | -49 | -119 | +107 | -5 | -9.0 | -10.2 % | 7 / 3 / 0 |

**L-A1 minus L-A0** (limiting Aether's tools)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean difference | Relative to L-A0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | -7 | -1 | -5 | +1 | -11 | -7 | +2 | +9 | +6 | +2 | -1.1 | -0.0 % | 5 / 5 / 0 |
| API requests | +45 | -13 | -5 | +28 | -17 | -13 | -9 | +5 | -2 | -14 | +0.5 | +1.8 % | 7 / 3 / 0 |
| Uncached input tokens | +36,040 | +22,245 | -10,380 | +35,791 | -15,818 | -6,342 | -5,696 | -13,975 | +4,964 | -16,875 | +2,995.4 | +6.1 % | 6 / 4 / 0 |
| Cache read tokens | +1,803,008 | -436,992 | -249,600 | +934,912 | -567,040 | -424,704 | -269,056 | +192,256 | -2,560 | -452,096 | +52,812.8 | +6.6 % | 7 / 3 / 0 |
| Input tokens over all requests (uncached + cache read + cache write) | +1,839,048 | -414,747 | -259,980 | +970,703 | -582,858 | -431,046 | -274,752 | +178,281 | +2,404 | -468,971 | +55,808.2 | +6.5 % | 6 / 4 / 0 |
| Output tokens | +6,480 | -1,490 | +622 | +3,225 | -2,782 | +1,394 | +860 | -69 | +4,359 | -3,600 | +899.9 | +8.2 % | 4 / 6 / 0 |
| Reasoning tokens, total | +0 | +522 | +1,946 | +0 | -395 | +0 | +0 | +0 | +2,427 | +0 | +450.0 | +1139.2 % | 1 / 3 / 6 |
| API-equivalent USD | +0.0249 | -0.0029 | -0.0032 | +0.0145 | -0.0086 | -0.0042 | -0.0028 | +0.0005 | +0.0027 | -0.0080 | +0.0013 | +6.9 % | 6 / 4 / 0 |
| Wall seconds | +103.8 | -36.3 | +7.7 | +1,176.2 | +85.9 | +162.4 | -9.7 | +9.7 | +41.6 | -193.2 | +134.8 | +118.9 % | 3 / 7 / 0 |
| Hidden checks passed (of 119) | +111 | -98 | -6 | -87 | +4 | -5 | +54 | +116 | +4 | -31 | +6.2 | +7.8 % | 5 / 5 / 0 |

**L-A1 minus L-N0** (Aether with minimal tools against native)

| Measure | Block 11 | Block 12 | Block 13 | Block 14 | Block 15 | Block 16 | Block 17 | Block 18 | Block 19 | Block 20 | Mean difference | Relative to L-N0 mean | Lower / higher / equal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First-request input tokens | +920 | +927 | +934 | +932 | +145 | +932 | +927 | +939 | +927 | +923 | +850.6 | +5.4 % | 0 / 10 / 0 |
| API requests | +48 | +0 | -8 | +30 | -18 | +13 | +4 | -17 | +30 | -26 | +5.6 | +24.9 % | 4 / 5 / 1 |
| Uncached input tokens | +43,594 | +33,950 | -3,720 | +51,084 | -36,020 | +29,662 | +9,161 | -20,539 | +47,138 | -26,818 | +12,749.2 | +32.1 % | 4 / 6 / 0 |
| Cache read tokens | +1,875,968 | -1,024 | -291,840 | +1,024,512 | -764,416 | +495,360 | +133,120 | -595,712 | +1,103,360 | -808,448 | +217,088.0 | +33.9 % | 5 / 5 / 0 |
| Input tokens over all requests (uncached + cache read + cache write) | +1,919,562 | +32,926 | -295,560 | +1,075,596 | -800,436 | +525,022 | +142,281 | -616,251 | +1,150,498 | -835,266 | +229,837.2 | +33.8 % | 4 / 6 / 0 |
| Output tokens | +8,776 | -2,448 | -571 | +3,485 | -15,608 | +4,680 | +530 | -7,706 | +11,762 | -4,657 | -175.7 | -1.5 % | 5 / 5 / 0 |
| Reasoning tokens, total | -479 | -3,582 | +1,946 | +0 | -10,229 | +0 | -1,882 | -3,232 | +2,427 | +0 | -1,503.1 | -75.4 % | 5 / 2 / 3 |
| API-equivalent USD | +0.0275 | +0.0022 | -0.0036 | +0.0171 | -0.0191 | +0.0103 | +0.0025 | -0.0119 | +0.0216 | -0.0131 | +0.0034 | +20.5 % | 4 / 6 / 0 |
| Wall seconds | +112.1 | -191.1 | -5.0 | +1,183.7 | +7.8 | +209.4 | +3.5 | -74.2 | +35.4 | -216.1 | +106.5 | +75.2 % | 4 / 6 / 0 |
| Hidden checks passed (of 119) | +12 | -109 | -4 | -99 | +3 | +92 | +5 | -3 | +111 | -36 | -2.8 | -3.2 % | 5 / 5 / 0 |

#### Where the input tokens differ

**L-A0 minus L-N0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 11 | 14 vs 11 | 16,557 vs 15,630 | 343,474 vs 262,960 | +80,514 | +10,197 | +49,671 | +20,646 |
| 12 | 36 vs 23 | 16,558 vs 15,630 | 1,093,061 vs 645,388 | +447,673 | +21,344 | +215,254 | +211,075 |
| 13 | 34 vs 37 | 16,567 vs 15,628 | 1,133,977 vs 1,169,557 | -35,580 | +34,743 | -49,701 | -20,622 |
| 14 | 21 vs 19 | 16,559 vs 15,628 | 610,309 vs 505,416 | +104,893 | +17,689 | +33,118 | +54,086 |
| 15 | 32 vs 33 | 16,559 vs 16,403 | 1,021,490 vs 1,239,068 | -217,578 | +5,148 | -16,559 | -206,167 |
| 16 | 37 vs 11 | 16,565 vs 15,626 | 1,221,390 vs 265,322 | +956,068 | +10,329 | +430,690 | +515,049 |
| 17 | 23 vs 10 | 16,553 vs 15,628 | 663,587 vs 246,554 | +417,033 | +9,250 | +215,189 | +192,594 |
| 18 | 15 vs 37 | 16,556 vs 15,626 | 394,763 vs 1,189,295 | -794,532 | +34,410 | -364,232 | -464,710 |
| 19 | 40 vs 8 | 16,551 vs 15,630 | 1,328,364 vs 180,270 | +1,148,094 | +7,368 | +529,632 | +611,094 |
| 20 | 24 vs 36 | 16,551 vs 15,630 | 729,883 vs 1,096,178 | -366,295 | +33,156 | -198,612 | -200,839 |
| Mean (10) | | | | +174,029 | +18,363 | +84,445 | +71,221 |

**L-A1 minus L-A0: where the input tokens differ**

| Block | Requests | First-request input | Input tokens, all requests | Difference | Larger fixed context | More requests | Conversation growth |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 11 | 59 vs 14 | 16,550 vs 16,557 | 2,182,522 vs 343,474 | +1,839,048 | -98 | +744,750 | +1,094,396 |
| 12 | 23 vs 36 | 16,557 vs 16,558 | 678,314 vs 1,093,061 | -414,747 | -36 | -215,241 | -199,470 |
| 13 | 29 vs 34 | 16,562 vs 16,567 | 873,997 vs 1,133,977 | -259,980 | -170 | -82,810 | -177,000 |
| 14 | 49 vs 21 | 16,560 vs 16,559 | 1,581,012 vs 610,309 | +970,703 | +21 | +463,680 | +507,002 |
| 15 | 15 vs 32 | 16,548 vs 16,559 | 438,632 vs 1,021,490 | -582,858 | -352 | -281,316 | -301,190 |
| 16 | 24 vs 37 | 16,558 vs 16,565 | 790,344 vs 1,221,390 | -431,046 | -259 | -215,254 | -215,533 |
| 17 | 14 vs 23 | 16,555 vs 16,553 | 388,835 vs 663,587 | -274,752 | +46 | -148,995 | -125,803 |
| 18 | 20 vs 15 | 16,565 vs 16,556 | 573,044 vs 394,763 | +178,281 | +135 | +82,825 | +95,321 |
| 19 | 38 vs 40 | 16,557 vs 16,551 | 1,330,768 vs 1,328,364 | +2,404 | +240 | -33,114 | +35,278 |
| 20 | 10 vs 24 | 16,553 vs 16,551 | 260,912 vs 729,883 | -468,971 | +48 | -231,742 | -237,277 |
| Mean (10) | | | | +55,808 | -42 | +8,278 | +47,572 |

#### Hidden checks, both iterations together

| Iteration | Block | L-N0 | L-A0 | L-A1 |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 1 | 104 | 0 | 76 |
| 1 | 2 | 116 | 0 | 117 |
| 1 | 3 | 102 | 99 | 103 |
| 1 | 4 | 105 | 86 | 111 |
| 1 | 5 | 84 | 102 | 100 |
| 1 | 6 | 118 | 90 | 107 |
| 2 | 11 | 99 | 0 | 111 |
| 2 | 12 | 109 | 98 | 0 |
| 2 | 13 | 112 | 114 | 108 |
| 2 | 14 | 99 | 87 | 0 |
| 2 | 15 | 114 | 113 | 117 |
| 2 | 16 | 0 | 97 | 92 |
| 2 | 17 | 110 | 61 | 115 |
| 2 | 18 | 119 | 0 | 116 |
| 2 | 19 | 0 | 107 | 111 |
| 2 | 20 | 118 | 113 | 82 |

| Blocks | n | L-N0 mean | L-A0 mean | L-A1 mean | L-A0 below / above / equal to L-N0 | L-A0 below / above / equal to L-A1 | L-A1 below / above / equal to L-N0 | Trials under half: L-N0 | L-A0 | L-A1 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Iteration 1 | 6 | 104.8 | 62.8 | 102.3 | 5 / 1 / 0 | 5 / 1 / 0 | 2 / 4 / 0 | 0 | 2 | 0 |
| Iteration 2 | 10 | 88.0 | 79.0 | 85.2 | 7 / 3 / 0 | 5 / 5 / 0 | 5 / 5 / 0 | 2 | 2 | 2 |
| Both | 16 | 94.3 | 72.9 | 91.6 | 12 / 4 / 0 | 10 / 6 / 0 | 7 / 9 / 0 | 2 | 4 | 2 |

Over the 16 blocks of both iterations:

- **Full-tool Aether (L-A0) passed fewer checks than Codex (L-N0) in 12 of
  16 blocks** (5 of 6, then 7 of 10) and more in 4. Means: 72.9 against
  94.3.
- **L-A0 passed fewer checks than the two-tool cell (L-A1) in 10 of 16
  blocks** (5 of 6, then 5 of 10) and more in 6. Means: 72.9 against 91.6.
- **The two-tool cell against Codex is level**: fewer in 7 of 16, more in 9.
  Means: 91.6 against 94.3.
- **A trial at 0 moves these means more than anything else does.** There are
  8 in 48 trials: 2 of 16 for L-N0, 4 of 16 for L-A0, 2 of 16 for L-A1. In
  iteration 1 both were in L-A0. In iteration 2 they are 2, 2 and 2.
- Among the trials that did not score 0, the means are 107.8 for L-N0 (14
  trials), 97.3 for L-A0 (12) and 104.7 for L-A1 (14).

#### The low trials

Every Luna trial of this iteration, with what its session shows. A test
suite run is a command that names `pytest` or `unittest`; an ad hoc check is
a `python -c` or heredoc command. For a trial under half of the hidden
checks, each of the 119 hidden tests was run again in its own process on a
copy of the trial's final files, with 5 seconds per test.

| Arm | Block | Trial | Hidden checks | Wall seconds | Commands | Test suite runs | Ad hoc checks | of them naming iceberg | File changes | `complete_task` calls | Workflow | Per test: passed / failed / hung |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| L-A0 | 11 | `a2211ba7ba` | 0 | 75.7 | 7 | 2 | 2 | 2 | 7 | 0 | completed | 108 / 4 / 7 |
| L-A0 | 12 | `2063ca0ec7` | 98 | 127.4 | 23 | 10 | 11 | 10 | 15 | 0 | completed |  |
| L-A0 | 13 | `310a6b7b7d` | 114 | 113.1 | 15 | 5 | 0 | 0 | 14 | 0 | completed |  |
| L-A0 | 14 | `40b36b067b` | 87 | 77.6 | 8 | 0 | 0 | 0 | 11 | 0 | completed |  |
| L-A0 | 15 | `f1c71e686b` | 113 | 108.9 | 17 | 7 | 7 | 4 | 14 | 0 | completed |  |
| L-A0 | 16 | `9e48001f0a` | 97 | 114.2 | 20 | 6 | 4 | 3 | 13 | 0 | completed |  |
| L-A0 | 17 | `543481a727` | 61 | 85.8 | 11 | 5 | 5 | 5 | 11 | 0 | completed |  |
| L-A0 | 18 | `0016713e9a` | 0 | 64.9 | 7 | 0 | 0 | 0 | 7 | 0 | completed | 0 / 119 / 0 |
| L-A0 | 19 | `93f6873849` | 107 | 113.2 | 23 | 8 | 11 | 6 | 14 | 0 | completed |  |
| L-A0 | 20 | `f880e524fd` | 113 | 253.2 | 12 | 5 | 2 | 0 | 9 | 0 | completed |  |
| L-A1 | 11 | `526835adc1` | 111 | 179.5 | 36 | 10 | 19 | 19 | 21 | 0 | completed |  |
| L-A1 | 12 | `8ff4c1d394` | 0 | 91.1 | 11 | 6 | 5 | 4 | 10 | 0 | completed | 103 / 11 / 5 |
| L-A1 | 13 | `73d2e7e008` | 108 | 120.7 | 19 | 4 | 8 | 7 | 8 | 0 | completed |  |
| L-A1 | 14 | `f3a914ce1f` | 0 | 1253.8 | 17 | 6 | 1 | 0 | 15 | 0 | failed | 72 / 10 / 37 |
| L-A1 | 15 | `d082ee0115` | 117 | 194.8 | 14 | 6 | 7 | 7 | 0 | 0 | completed |  |
| L-A1 | 16 | `f4d7165479` | 92 | 276.5 | 13 | 5 | 4 | 4 | 7 | 0 | completed |  |
| L-A1 | 17 | `63d361d442` | 115 | 76.0 | 13 | 1 | 10 | 8 | 0 | 0 | completed |  |
| L-A1 | 18 | `2c4ba9099b` | 116 | 74.6 | 15 | 4 | 6 | 3 | 3 | 0 | completed |  |
| L-A1 | 19 | `e8eb90cf40` | 111 | 154.8 | 24 | 7 | 7 | 6 | 11 | 0 | completed |  |
| L-A1 | 20 | `8becca20a1` | 82 | 60.0 | 10 | 4 | 3 | 1 | 1 | 0 | completed |  |
| L-N0 | 11 | `0ce1e64fd1` | 99 | 67.4 | 10 | 3 | 5 | 5 | 0 | 0 | completed |  |
| L-N0 | 12 | `1e5fa3b735` | 109 | 282.2 | 10 | 2 | 3 | 1 | 10 | 0 | completed |  |
| L-N0 | 13 | `1cfcda628e` | 112 | 125.8 | 22 | 6 | 8 | 6 | 12 | 0 | completed |  |
| L-N0 | 14 | `8356829a0b` | 99 | 70.2 | 13 | 4 | 3 | 2 | 5 | 0 | completed |  |
| L-N0 | 15 | `a576eaa5fa` | 114 | 187.1 | 18 | 5 | 9 | 8 | 12 | 0 | completed |  |
| L-N0 | 16 | `68fc219a91` | 0 | 67.1 | 6 | 2 | 2 | 1 | 4 | 0 | completed | 112 / 0 / 7 |
| L-N0 | 17 | `a5f3736c51` | 110 | 72.6 | 7 | 3 | 2 | 2 | 2 | 0 | completed |  |
| L-N0 | 18 | `9bec09150c` | 119 | 148.8 | 15 | 6 | 7 | 3 | 19 | 0 | completed |  |
| L-N0 | 19 | `7c6814aef0` | 0 | 119.4 | 7 | 2 | 1 | 1 | 0 | 0 | completed | 112 / 5 / 2 |
| L-N0 | 20 | `2fba39c0ba` | 118 | 276.1 | 19 | 4 | 3 | 2 | 14 | 0 | completed |  |

Six of the 30 trials passed fewer than half of the hidden checks. All six
passed none, two in each arm:

| Trial | Arm | Block | Grader's error | Per test: passed / failed / hung | Hung tests, by file | What the session shows |
| --- | --- | ---: | --- | --- | --- | --- |
| `a2211ba7ba` | L-A0 | 11 | hidden tests timed out after 60s | 108 / 4 / 7 | iceberg 7 | 7 file changes, ran the 4 visible tests twice and 2 ad hoc checks, ended after 76 seconds |
| `0016713e9a` | L-A0 | 18 | all 119 tests fail with `AssertionError` | 0 / 119 / 0 | none | ran no test and no check of its own; ended after 65 seconds |
| `8ff4c1d394` | L-A1 | 12 | hidden tests timed out after 60s | 103 / 11 / 5 | iceberg 1, journal 2, self-trade 2 | 6 runs of the visible tests, 5 ad hoc checks; its last message says it did not test every contract case |
| `f3a914ce1f` | L-A1 | 14 | hidden tests timed out after 60s | 72 / 10 / 37 | all six files | found an infinite loop with a test it wrote, changed the engine, then started a command that never returned; stopped by the 1,200 second timeout |
| `68fc219a91` | L-N0 | 16 | hidden tests timed out after 60s | 112 / 0 / 7 | iceberg 2, journal 3, self-trade 2 | 2 runs of the visible tests, 2 ad hoc checks, ended after 67 seconds |
| `7c6814aef0` | L-N0 | 19 | hidden tests timed out after 60s | 112 / 5 / 2 | iceberg 2 | 2 runs of the visible tests, 1 ad hoc check, ended after 119 seconds |

What they have in common, and what they do not:

1. **Five of the six are one failure: a loop that does not end.** The
   solution hangs in a few hidden tests, the grader runs all 119 in one
   process with one limit of 60 seconds, and the trial is given 0. Run one
   test per process, the same five solutions pass 72 to 112 of 119. In all
   five at least one hung test is an iceberg test: the loop is in the
   matching of an iceberg order whose visible part refills. Iteration 1's
   two trials at 0 had the same grader error.
2. **The hang is in every arm, Codex included.** Two of the five are
   native. It is a way Luna fails on this case, not something Aether adds.
3. **The sixth is a different failure, and it is an Aether trial.**
   `0016713e9a` does not hang. Its engine raises an assertion the solver
   wrote itself (`_adopt`) on the first `submit`, so every hidden test
   fails. The solver ran nothing: its messages say it audits "without
   executing tests, per the workspace instructions". No text in its first
   turn says that. The task says "You may add tests"; the other mentions of
   tests are a skill description and two examples in the approval text. Why
   it believed so is not known. The approval text is one of the things an
   Aether Codex turn carries and a native one does not (6,144 against 362
   characters); that it played a part is a guess from one trial.
4. **No trial ended through `complete_task`.** None of the 20 Aether trials
   of this iteration called it or any other Aether tool. Each ended its turn
   with a final message, as the native trials do. Ending early through an
   Aether tool is ruled out as the cause.
5. **The low trials did not test visibly less than the others.** They ran
   the visible tests 0 to 6 times; the 24 other trials ran them 0 to 10
   times, and a trial that ran none passed 87 (`40b36b067b`). The visible
   suite is 4 smoke tests and none reaches the loop, so running it does not
   show the hang. What the four quick hang trials share is that they ended
   in 67 to 119 seconds with no test of their own for iceberg refills that
   ran to the end.
6. **`f3a914ce1f` is the one long run of this iteration.** It found the
   loop, and its own test of it then held the machine's memory until the
   timeout. This is the case named under "How iteration 2 was run".

#### The stop rule for Luna

**The stop rule on L-A0 against L-N0, 10 pairs**

| Measure | Native mean | Aether mean | Mean difference | Relative to native | Aether lower / higher / equal | Part 1: not one-sided | Part 2: within 5 % |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| Output tokens | 12,047.8 | 10,972.2 | -1,075.6 | -8.9 % | 6 / 4 / 0 | met | not met |
| Uncached input tokens | 39,719.2 | 49,473.0 | +9,753.8 | +24.6 % | 3 / 7 / 0 | not part of it | not met |
| API-equivalent USD | 0.0164 | 0.0185 | +0.0021 | +12.7 % | 4 / 6 / 0 | met | not met |
| Hidden checks passed (of 119) | 88.0 | 79.0 | -9.0 | -10.2 % | 7 / 3 / 0 | met | not part of it |

- **Part 1, not one-sided: met on all three measures.** Output tokens are
  lower through Aether in 6 of 10 pairs and higher in 4; cost is lower in 4
  and higher in 6; hidden checks are lower in 7 and higher in 3.
- **Part 2, mean difference within 5 % of the native mean: not met on any
  of the three.** Output tokens -8.9 % (10,972 against 12,048), uncached
  input tokens +24.6 % (49,473 against 39,719, higher in 7 of 10), cost
  +12.7 % (0.0185 against 0.0164 USD).
- **The stop rule is not met for Luna.**

How much weight part 2 can bear here: the difference in a single pair runs
from -20,202 to +42,174 uncached input tokens and from -0.0124 to +0.0190
USD, and it follows the number of requests (27.6 through Aether against
22.5 natively in the mean; fewer in 4 pairs, more in 6). A mean of 10 such
pairs is not expected to land within 5 % even if the two sides were the
same; the two Aether arms, which differ only in their tool list, are 6.1 %
and 6.9 % apart on the same two measures. So "not met" is the honest
reading of the rule, and it does not show that Aether costs Luna tokens.

#### Verdict for Luna, iteration 2

**In tokens, cost and time nothing consistent was found again. In hidden
checks the pattern of iteration 1 did not repeat, and a smaller one is left
open.**

What is measured:

1. **The fixed cost on the first request is still there**: 16,558 against
   15,706 input tokens, higher in 10 of 10 pairs (by 921 to 939 in nine; by
   156 in block 15, where the native first request was itself larger).
   Without the effort words Aether's first request went from 16,569 to
   16,558.
2. **Beyond that, no measure is one-sided.** L-A0 used fewer output tokens
   than L-N0 in 6 of 10 pairs, cost less in 4, and was faster in 6. The
   means differ by more than 5 %, in both directions, as shown above.
3. **Trials at 0 are in every arm**: 2 of 10 each for L-N0, L-A0 and L-A1.
   Five of the six are the same hang, two of them native.
4. **Limiting Aether's tools did not raise the checks in iteration 2**:
   L-A1 passed more than L-A0 in 5 blocks and fewer in 5 (85.2 against
   79.0).

What this does to iteration 1's open question, whether Aether's full tool
list costs Luna hidden checks:

- **Not supported by iteration 2.** In iteration 1 both trials at 0 were in
  L-A0 and L-A1 was ahead in 5 of 6. In iteration 2 the two-tool cell has
  its own two trials at 0 and is ahead in 5 of 10.
- **Still open: whether Aether, with either tool list, costs Luna checks
  against Codex.** L-A0 is below L-N0 in 12 of 16 blocks over both
  iterations, and 10 checks below it among the trials that did not score 0
  (97.3 against 107.8). L-A1 is not below it (7 of 16, 104.7). Twelve of 16
  one way would come up by chance about once in 13 such comparisons, and
  more than one comparison was looked at. With the two-tool cell level
  with Codex, 16 blocks do not show an Aether effect and do not exclude a
  small one in the full-tool cell.

What 16 blocks do not show:

- **A rate of trials at 0 per arm.** 2, 4 and 2 of 16 cannot be told apart.
- **Why one Aether solver ran no tests** (`0016713e9a`).
- **Reasoning tokens.** L-N0 reported reasoning in 5 of 10 trials, L-A0 in
  1 of 10, as in iteration 1. Not explained.

### What iteration 2 points to

No further change to Aether is supported by this evidence. What was located
in iteration 1 is fixed, what it left open is placed, and the two results
that remain open are open for want of blocks, not for want of an arm.

**1. For Haiku: more blocks of the matched pair, with the removed tools
split in two.**

- Where: `evals/harness_bench_treatments.py`, `diagnostic_conditions`,
  lines 65 to 72, where `diag-native-haiku-low-nocoord` takes the whole of
  `NATIVE_COORDINATION_TOOLS`.
- Change: two more native arms that split that list.
  `diag-native-haiku-low-nocoord-loaded` disallows the four tools the CLI
  puts in every request (`Agent`, `ListAgents`, `ScheduleWakeup`,
  `Workflow`); `diag-native-haiku-low-nocoord-deferred` disallows the
  other six. `native_command` already passes whatever list a condition
  names. A test in `evals/test_harness_bench_diagnostics.py` holds that the
  two lists together are the Rust constant and share no name.
- How to test: blocks of H-N0, H-N3, the two new arms and H-A2 on
  `matchbook` at effort low, at least 12, because 6 left the matched pair
  at 3 each way; then the same on `gridcalc`, because every figure here is
  one case.
- What it decides: whether Aether's removal of the CLI's coordination
  tools (`crates/aether-engine/src/claude_code.rs`, `args()`, lines 122 to
  127) costs Haiku output tokens, and through which of them. Aether removes
  them so that an agent delegates through Aether and not past it. A change
  there costs that, and is not proposed before the effect is shown.

**2. The connectors, outside the benchmark.** An agent of an operator who
has connectors still loads them; only the benchmark keeps them out.
Iteration 1 listed as optional a setting that keeps them out of an agent
(`crates/aether-engine/src/claude_code.rs`, `args()`, the `--mcp-config`
block at lines 200 to 213). Iteration 2 gives its effect on Haiku: without
the connectors a trial used 18,586 more output tokens (6 of 6 blocks) and
passed 119.0 against 118.3 checks. Such a setting would make a Haiku agent
cost more, not less. It stays optional, for an operator who wants an
agent's context free of unrelated tools.

**3. For Luna: grade each hidden test on its own, beside the counted
figure.**

- Where: `evals/harness_bench_cases.py`, `grade`, lines 287 to 316. All
  hidden tests run in one process with one limit of 60 seconds, and a
  timeout reports 0 of 119.
- Change: when that run times out, run the tests again one per process
  with a limit each, and record the count in a second field. The counted
  `passed` stays what it is; changing it would be a new protocol version
  and is the operator's decision.
- How to test: a unit test with a fixture solution that hangs in one hidden
  test must report `passed` 0 and 118 in the new field. Then more blocks of
  L-N0, L-A0 and L-A1: on a measure that one loop does not set to 0, 16
  blocks give 107.8, 97.3 and 104.7 among the trials that did not hang.
- What it decides: whether full-tool Aether is below Codex in hidden
  checks, which 12 of 16 blocks suggest and do not show.

**4. For the machine: a memory limit per trial.** One Luna solver's own
test held the machine's memory for 18 minutes. The runner
(`evals/harness_bench.py`, where a trial's processes are started) should
start each trial under a memory limit, so that a runaway test fails inside
its trial. Until then a batch on a shared machine needs the guard that was
used here from 20:36.

**5. Not proposed: a change to Aether for Luna.** No Aether tool was called
in any Luna trial, the two-tool cell hangs as often as the full one, and
Codex hangs too.

## Iteration 3

- **The counted test was made again under protocol version 2**: five planned
  runs, 40 trials, on the binary without the effort words. The report is
  [harness-lower-tier-results-2026-10-10-protocol-2.md](harness-lower-tier-results-2026-10-10-protocol-2.md);
  the diagnostic figures above are not merged with it.
- **The stop rule is not met for either model.** Haiku: output tokens are no
  longer one-sided (lower through Aether in 5 of 10 pairs, higher in 5), but
  the mean differences are outside 5 % (output -6.5 %, uncached input
  -88.2 %, USD -20.0 %) and hidden checks are one-sided (lower in 2 pairs,
  equal in 8). Luna: no measure is one-sided, and the mean differences are
  outside 5 % against Aether (output +32.3 %, uncached input +43.9 %, USD
  +51.9 %).
- **The Luna accuracy question stays open**: Aether is below Codex in 6 of
  10 counted pairs, 2 of 5 on `matchbook` and 4 of 5 on `gridcalc`, with one
  trial at 0 from the grader's limit on each side.

## Iteration 4

Iteration 3 left GPT-6 Luna spending more through Aether than in Codex in the
mean of the counted pairs, with no measure one-sided. Iteration 4 looks
for the cause by experiment. It takes the five things known to differ in how
Aether starts Codex, gives each one that `codex exec` can reproduce an arm of
its own, and takes the largest of them away through Aether on a diagnostic
binary. It changes nothing in Aether: no file under `crates/` is touched by
its commits. Its 72 trials are diagnostic like the 108 before them: all are
reported here, none is counted, and the comparison and report tools refuse
their manifests.

What differs between the two sides, with the characters each puts into the
first turn ([What the first turn was given](#what-the-first-turn-was-given)):

1. the approval policy: `on-request` through Aether against `never`
   natively, both with full access (6,144 characters of permission
   instructions against 362);
2. Codex's multi-agent feature, which Aether switches off (2,671 characters
   absent through Aether);
3. the Aether envelope around the task (+690 characters at protocol
   version 1, +648 now that the effort words are gone);
4. `codex app-server` against `codex exec`: the app-server session lists
   more plugin skills (+3,279 characters);
5. Aether's own MCP server and its tools, which Codex does not record in
   the session.

### What the earlier sessions show

Before any trial, the Codex session logs of the Luna trials of iterations 1
to 3 were read again: the 26 matched pairs of `native-luna-low` and
`aether-single-luna-low` whose manifests are kept (6, 10 and 10; 21 on
`matchbook`, 5 on `gridcalc`). The logs were only read.

- **No approval or escalation request, in any of the 26 Aether sessions.**
  All 26 ran under `on-request` and all 26 native ones under `never`. No
  tool call through Aether carried `sandbox_permissions` or
  `require_escalated`, no session recorded an approval event, no command was
  refused for want of approval, and no message to the user spoke of
  permission, approval or escalation. The long permission text was sent and,
  as far as a session shows, never acted on.
- **The extra requests are more edits and more messages, not failures.**
  Aether minus native, per pair:

| Measure | Native mean | Aether mean | Mean difference | Aether lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 26.2 | 32.5 | +6.3 | 11 / 15 / 0 | -39 | +80 |
| Commands | 16.3 | 18.7 | +2.3 | 13 / 13 / 0 | -23 | +40 |
| Commands that exited non-zero | 3.3 | 3.1 | -0.2 | 11 / 13 / 2 | -13 | +5 |
| Test runs | 4.8 | 5.7 | +0.9 | 11 / 13 / 2 | -10 | +10 |
| Commands that only read | 3.8 | 5.6 | +1.8 | 8 / 13 / 5 | -3 | +21 |
| File changes | 8.9 | 13.1 | +4.2 | 7 / 17 / 2 | -26 | +43 |
| Messages to the user | 6.5 | 8.6 | +2.1 | 7 / 16 / 3 | -3 | +17 |
| Reasoning tokens | 1,576.4 | 105.7 | -1,470.8 | 14 / 2 / 10 | -9,834 | +245 |
| Output tokens | 13,034.1 | 14,056.5 | +1,022.4 | 14 / 12 / 0 | -12,826 | +14,651 |
| Uncached input tokens | 44,919.7 | 57,353.3 | +12,433.6 | 8 / 18 / 0 | -41,568 | +104,202 |

- **Uncached input follows the requests.** Per request the two sides are
  level: 1,715 uncached input tokens natively and 1,765 through Aether. The
  whole of the difference in uncached input is in how many requests a run
  makes.
- **One thing happens almost only on one side: reasoning.** 14 of the 26
  native trials reported reasoning tokens, 5 of the 26 Aether ones, and the
  means are 1,576 against 106. Iterations 1 and 2 noted this and left it
  unexplained. Both sides record effort `low` in the session.

So the approval policy does not act through requests for approval. If it
costs anything it does so as text: 5,782 more characters in the first turn
that tell the model how and when to ask. That is what arm L-A3 tests.

### The arms of iteration 4

All arms run GPT-6 Luna at requested effort low, protocol version 2, on the
frozen cases `matchbook` and `gridcalc`.

| Arm | Condition | Differs from | In |
| --- | --- | --- | --- |
| L-N0 | `native-luna-low` | | the counted native cell: `codex exec --json --dangerously-bypass-approvals-and-sandbox -c mcp_servers={}` |
| L-N5 | `diag-native-luna-low-onrequest` | L-N0 | `--sandbox danger-full-access -c approval_policy="on-request"` in place of the bypass flag: the policy and sandbox Aether sets. **`codex exec` does not apply the policy**; see below |
| L-N6 | `diag-native-luna-low-nomultiagent` | L-N0 | `-c agents.enabled=false -c features.multi_agent=false`, the two settings Aether passes (`crates/aether-engine/src/codex_app.rs`, `args()`, lines 118 and 120) |
| L-N7 | `diag-native-luna-low-envelope` | L-N0 | the task inside Aether's first-turn envelope as protocol version 2 sends it, without effort words |
| L-A0 | `aether-single-luna-low` | | the counted Aether cell: one agent on `codex/gpt-6-luna` |
| L-A3 | `diag-aether-luna-low-approval-never` | L-A0 | the diagnostic binary, on which a bypass turn with network starts its thread with approval `never`; the sandbox is unchanged |

The matched pair is **L-A0 against L-N0**.

Where an arm is not what its name says, or not exactly its one change:

- **L-N5 is a second native cell, not a native cell under `on-request`.**
  `codex exec` 0.162.1 starts its thread with approval `never` whatever it
  is asked for. Three forms were tried on a one-line task before the trials,
  and the session log of each was read:

  | Launched as | Approval policy of the turn | Sandbox of the turn | Permission instructions, characters |
  | --- | --- | --- | ---: |
  | `codex exec --sandbox danger-full-access -c approval_policy="on-request"` (the arm) | `never` | `danger-full-access` | 362 |
  | `codex --ask-for-approval on-request --sandbox danger-full-access exec` | `never` | `danger-full-access` | 362 |
  | `codex exec --approve-for-me` | `on-request` | `workspace-write`, no network | 947 |

  `--approve-for-me` cannot be combined with `--sandbox` (the CLI exits with
  status 2), so the one form that keeps `on-request` also changes the
  sandbox, takes the network away and hands approvals to an automatic
  reviewer, and its text is not the 6,144 characters Aether's sessions
  carry. No form of `codex exec` reproduces Aether's policy. The arm was run
  as specified all the same. Every one of its 12 sessions shows `never` and
  the 362 character text, and its first request is the size of L-N0's. **It
  therefore says nothing about the approval policy.** What it does give is
  a yardstick: L-N5 minus L-N0 is the difference between two native cells
  that the model cannot tell apart, in the same block and under the same
  load.
- **The approval policy is isolated only through Aether**, by L-A3 against
  L-A0.
- **L-A3 runs a binary that is not the release build.** It differs from it
  in one line, quoted below. The condition itself is the counted Aether
  cell's with the name of that patch beside it; the runner takes one
  `--binary` per invocation and refuses an arm that names a patched binary
  beside another Aether arm, so L-A3 ran as its own invocation, started in
  the same second as its block's other five arms.
- **L-N7's envelope is a fixture** with a fixed nonce and task id. It is
  the size of the real one.
- **L-A0 and L-A3 differ from L-N0 in more than the arms isolate.** The
  app-server's longer skill listing and Aether's own MCP tools are in both
  and in no native arm. What is left of L-A0 minus L-N0 after the isolated
  parts is theirs, together with whatever does not add up.

The patch of the diagnostic binary, never committed:

```diff
--- a/crates/aether-engine/src/codex_app.rs
+++ b/crates/aether-engine/src/codex_app.rs
@@ -187,7 +187,7 @@ impl CodexAppEngine {
 /// an ungated agent would silently escape its offline scope.
 pub fn thread_params(turn: &EngineTurn) -> (&'static str, Value) {
     let (approval, sandbox) = if turn.bypass_permissions && turn.network_access {
-        ("on-request", "danger-full-access")
+        ("never", "danger-full-access")
     } else if turn.bypass_permissions {
         ("never", "workspace-write")
     } else {
```

### How iteration 4 was run

| | |
| --- | --- |
| Aether binary, arms L-A0 | `aether 0.1.1`, `cargo build --release` at commit `231d472c`, SHA-256 `251c0b08f359c8a679a76831dea1d892b0c2a0a2c34e51427323c8b25d4d4145`, the same digest as in iterations 2 and 3; not installed |
| Diagnostic binary, arm L-A3 | the same source with the patch above, built into a target directory of its own, SHA-256 `894df72c75b7d80e2d118d86357c798255a7109d684dca61d955d03455a627b4`; not installed. The patch was reverted after the build and `git diff --quiet -- crates` held before the first trial |
| Benchmark code | commit `a9169533`; every manifest records it with a clean tree and `protocol_version` 2 |
| Native CLI | `codex-cli` 0.162.1, as in every earlier sitting |
| Cases | `matchbook` (119 hidden checks, digest `7f719f2d3e0b7c98228f9bdf9a922aea93b605cbec9cb3b46535fe66ab8d630a`) and `gridcalc` (154, `df35116f6492541fbdc8078c26355647483ff321332b91bb7ab3c9c975dd4758`), both recomputed before the first trial |
| Blocks | 6 per case, fixed in writing before the first trial and all run; nothing was stopped early. A block is one trial of each of the six arms on one case, all started together |
| Seeds | 21 to 26, one per block |
| Trials at once | two streams side by side, one per case: 12 trials at once. A block's five arms on the release binary ran as one invocation with `--jobs 5`, L-A3 as a second with `--jobs 1`, both started in the same second |
| Timeout | `--timeout 1200` seconds; no trial reached it, and the longest solver ran 512 seconds |
| Flag | `--diagnostic` on every invocation, recorded in every manifest |
| Trials | 72: 6 arms, 2 cases, 6 blocks |

Before every block the available memory and the free space on the home
volume were read and required to be at least 4 GB and 15 GB:

| Block (seed) | `matchbook` start (UTC) | end | Available memory (MB) | Free disk (GB) | `gridcalc` start (UTC) | end | Available memory (MB) | Free disk (GB) |
| ---: | --- | --- | ---: | ---: | --- | --- | ---: | ---: |
| 21 | 23:13:58 | 23:19:16 | 8,491 | 40 | 23:13:58 | 23:22:31 | 8,491 | 40 |
| 22 | 23:19:16 | 23:24:59 | 8,255 | 40 | 23:22:31 | 23:30:22 | 8,126 | 40 |
| 23 | 23:24:59 | 23:29:42 | 7,706 | 40 | 23:30:22 | 23:38:21 | 9,423 | 40 |
| 24 | 23:29:42 | 23:35:57 | 10,449 | 40 | 23:38:21 | 23:46:29 | 9,274 | 40 |
| 25 | 23:35:57 | 23:40:56 | 9,699 | 40 | 23:46:29 | 23:51:48 | 9,420 | 39 |
| 26 | 23:40:56 | 23:46:16 | 9,133 | 39 | 23:51:48 | 00:00:19 | 9,112 | 39 |

- **All 72 trials ran and are reported.** All 24 invocations exited 0, every
  trial's workflow completed, and no trial reached the timeout: the longest
  solver ran 512 seconds, the shortest 44.
- **The plan was kept.** Six blocks per case were fixed in writing before the
  first trial; all twelve ran, `matchbook` from 23:13:58 to 23:46:16 UTC and
  `gridcalc` from 23:13:58 to 00:00:19.
- **The machine held.** The lowest available memory at a block start was
  7,706 MB and the home volume had 39 or 40 GB free throughout. A guard
  watched for a trial's process holding more than 4 GB and never acted.
- **Every manifest records the benchmark commit with a clean tree**, the
  `diagnostic` flag and `protocol_version` 2. The 60 trials of the five arms
  on the release binary record its digest, the 12 of L-A3 the diagnostic
  one's.
- **Every solver's session log was found and read**: 72 of 72. Each session
  ran GPT-6 Luna at effort `low`.

Figures are read as before: token, cost, time and hidden-check figures from
the manifests, the rest from each solver's Codex session log.

### What the first turn was given

Characters of each block of context in the first turn, and the settings the
turn ran under, as each session log recorded them. One value where every
trial of an arm agrees, else the range; the task differs in length between
the two cases.

| Block of context | L-N0 | L-N5 | L-N6 | L-N7 | L-A0 | L-A3 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `developer: <collaboration_mode># Collab` | 920 | 920 | 920 | 920 | 920 | 920 |
| `developer: <multi_agent_mode>Any earlie` | 271 | 271 | 0 | 271 | 0 | 0 |
| `developer: <multi_agent_role>You are `/` | 2,400 | 2,400 | 0 | 2,400 | 0 | 0 |
| `developer: <permissions instructions>` | 362 | 362 | 362 | 362 | 6,144 | 362 |
| `developer: <recommended_plugins>` | 901 | 901 | 901 | 901 | 901 | 901 |
| `developer: <skills_instructions>` | 4,751 | 4,751 | 4,751 | 4,751 | 8,030 | 8,030 |
| `user: # AGENTS.md instructions` | 5,884 | 5,884 | 5,884 | 5,884 | 5,884 | 5,884 |
| `user: <environment_context>` | 545 | 545 | 545 | 545 | 545 | 545 |
| `user: the task, bare` | 731 to 789 | 731 to 789 | 731 to 789 | 0 | 0 | 0 |
| `user: the task, in the Aether envelope` | 0 | 0 | 0 | 1,379 to 1,437 | 1,379 to 1,437 | 1,379 to 1,437 |
| Total characters | 16,765 to 16,823 | 16,765 to 16,823 | 14,094 to 14,152 | 17,413 to 17,471 | 23,803 to 23,861 | 18,021 to 18,079 |
| Approval policy of the turn | `never` | `never` | `never` | `never` | `on-request` | `never` |
| Sandbox of the turn | `danger-full-access` | `danger-full-access` | `danger-full-access` | `danger-full-access` | `danger-full-access` | `danger-full-access` |
| Multi-agent version of the turn | `v2` | `v2` | `disabled` | `v2` | `disabled` | `disabled` |
| Effort of the turn | `low` | `low` | `low` | `low` | `low` | `low` |
| Started by | `codex_exec` | `codex_exec` | `codex_exec` | `codex_exec` | `aether` | `aether` |
| Codex CLI version | `0.162.1` | `0.162.1` | `0.162.1` | `0.162.1` | `0.162.1` | `0.162.1` |
| Sessions read | 12 | 12 | 12 | 12 | 12 | 12 |

- **Every arm is what it claims, L-N5 excepted as said.** L-N6 lacks the
  two multi-agent blocks (2,671 characters) and records the multi-agent
  version as `disabled`, as Aether's sessions do. L-N7 carries the task in
  the envelope at the size Aether's sessions show. L-A0 runs under
  `on-request` with the 6,144 character permission text. L-A3 runs under
  `never` with the 362 character text and is otherwise L-A0 block for block.
- **The long permission text is mostly not about this task.** Its 6,144
  characters are the rules for asking to run a command outside the sandbox
  (when to ask, how to word the request, how to propose a rule for later)
  and the list of command prefixes the machine's operator has approved
  before. Under full access no command needs any of it.
- **In tokens**, the first request carries 15,623 input tokens in L-N0 on
  `matchbook` and 15,600 on `gridcalc`, in the mean of 6 trials. L-N5 is the
  same (15,623 and 15,598). L-N6 is 1,558 and 1,560 below, L-N7 211 and 205
  above, L-A0 923 and 930 above, and L-A3 557 and 562 below. The permission
  text is therefore 1,480 and 1,493 tokens of L-A0's first request, and
  without it Aether's first request is smaller than Codex's own.

### One line per arm

**One line per arm, `matchbook`: the mean of each measure**

| Arm | n | API requests | First-request input tokens | Uncached input tokens | Cache read tokens | Output tokens | Reasoning tokens | API-equivalent USD | Wall seconds | Hidden checks passed | Hidden checks, with the per-test count where the grader timed out |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 6 | 31.0 | 15,623.3 | 50,218.2 | 979,200.0 | 12,183.2 | 0.0 | 0.0209 | 183.7 | 73.0 | 107.5 |
| L-N5 | 6 | 31.8 | 15,622.7 | 53,419.8 | 930,176.0 | 11,944.2 | 809.8 | 0.0206 | 152.6 | 73.8 | 108.3 |
| L-N6 | 6 | 34.0 | 14,065.3 | 61,803.5 | 1,126,400.0 | 16,146.7 | 0.0 | 0.0255 | 166.6 | 102.0 | 102.0 |
| L-N7 | 6 | 29.3 | 15,834.0 | 53,973.0 | 859,989.3 | 13,025.0 | 1,235.2 | 0.0205 | 107.5 | 102.8 | 102.8 |
| L-A0 | 6 | 18.8 | 16,546.2 | 37,397.5 | 521,173.3 | 11,569.3 | 692.7 | 0.0147 | 154.2 | 37.3 | 102.5 |
| L-A3 | 6 | 25.5 | 15,066.2 | 47,109.7 | 736,725.3 | 12,108.2 | 266.3 | 0.0181 | 91.8 | 53.3 | 103.5 |

**One line per arm, `gridcalc`: the mean of each measure**

| Arm | n | API requests | First-request input tokens | Uncached input tokens | Cache read tokens | Output tokens | Reasoning tokens | API-equivalent USD | Wall seconds | Hidden checks passed | Hidden checks, with the per-test count where the grader timed out |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 6 | 56.7 | 15,599.7 | 87,448.8 | 2,564,437.3 | 26,085.5 | 4,899.0 | 0.0474 | 251.2 | 136.7 | 136.7 |
| L-N5 | 6 | 46.8 | 15,597.7 | 66,759.8 | 1,681,536.0 | 20,139.5 | 1,591.0 | 0.0336 | 213.2 | 120.8 | 120.8 |
| L-N6 | 6 | 45.7 | 14,039.7 | 72,701.0 | 1,542,741.3 | 20,375.2 | 1,283.2 | 0.0329 | 239.8 | 129.2 | 129.2 |
| L-N7 | 6 | 46.5 | 15,804.3 | 62,929.0 | 1,644,586.7 | 19,771.8 | 1,685.7 | 0.0326 | 263.4 | 130.7 | 130.7 |
| L-A0 | 6 | 43.7 | 16,530.0 | 67,881.5 | 1,670,058.7 | 20,215.3 | 585.0 | 0.0336 | 221.5 | 103.7 | 119.7 |
| L-A3 | 6 | 75.5 | 15,037.3 | 100,549.2 | 3,234,816.0 | 26,670.0 | 0.0 | 0.0557 | 286.4 | 135.3 | 135.3 |

### Every trial

| Case | Block | Arm | Trial | Workflow | Requests | First-request input | Uncached input | Cache read | Output | Reasoning | USD | Wall seconds | Hidden checks | Per test: passed / failed / hung |
| --- | ---: | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `matchbook` | 21 | L-N0 | `95526ae56d` | completed | 36 | 15,622 | 84,705 | 1,198,080 | 14,851 | 0 | 0.0279 | 316.6 | 118 of 119 |  |
| `matchbook` | 21 | L-N5 | `50b7271173` | completed | 32 | 15,622 | 43,366 | 890,880 | 11,361 | 0 | 0.0189 | 97.7 | 0 of 119 | 98 / 17 / 4 |
| `matchbook` | 21 | L-N6 | `95d488dd8f` | completed | 27 | 14,065 | 53,347 | 781,568 | 13,694 | 0 | 0.0200 | 106.0 | 98 of 119 |  |
| `matchbook` | 21 | L-N7 | `398b6eaaa0` | completed | 23 | 15,834 | 34,923 | 590,080 | 8,390 | 0 | 0.0136 | 77.6 | 103 of 119 |  |
| `matchbook` | 21 | L-A0 | `e059865b2e` | completed | 12 | 16,546 | 26,639 | 297,984 | 8,236 | 0 | 0.0098 | 55.9 | 0 of 119 | 92 / 25 / 2 |
| `matchbook` | 21 | L-A3 | `4af2b85a32` | completed | 26 | 15,066 | 47,708 | 800,256 | 12,371 | 0 | 0.0190 | 92.8 | 0 of 119 | 101 / 16 / 2 |
| `matchbook` | 22 | L-N0 | `7584d545cf` | completed | 23 | 15,622 | 28,405 | 578,816 | 8,233 | 0 | 0.0127 | 68.8 | 101 of 119 |  |
| `matchbook` | 22 | L-N5 | `e0cd06ba8c` | completed | 33 | 15,624 | 45,961 | 951,040 | 11,739 | 0 | 0.0200 | 91.7 | 113 of 119 |  |
| `matchbook` | 22 | L-N6 | `3ef3845a58` | completed | 63 | 14,065 | 99,669 | 2,423,040 | 21,645 | 0 | 0.0450 | 186.8 | 95 of 119 |  |
| `matchbook` | 22 | L-N7 | `b9e822ef46` | completed | 37 | 15,830 | 68,366 | 1,099,520 | 17,059 | 1,718 | 0.0264 | 145.4 | 98 of 119 |  |
| `matchbook` | 22 | L-A0 | `128ed8ddb7` | completed | 23 | 16,545 | 45,528 | 669,952 | 17,170 | 4,156 | 0.0198 | 340.9 | 106 of 119 |  |
| `matchbook` | 22 | L-A3 | `7df83c19ba` | completed | 43 | 15,065 | 61,423 | 1,341,696 | 16,072 | 224 | 0.0276 | 124.6 | 0 of 119 | 99 / 19 / 1 |
| `matchbook` | 23 | L-N0 | `2d13bc3ae3` | completed | 57 | 15,626 | 74,050 | 2,169,600 | 18,861 | 0 | 0.0385 | 171.0 | 0 of 119 | 103 / 13 / 3 |
| `matchbook` | 23 | L-N5 | `a4fac13e97` | completed | 40 | 15,622 | 56,696 | 1,263,616 | 12,151 | 0 | 0.0244 | 281.6 | 116 of 119 |  |
| `matchbook` | 23 | L-N6 | `cf4e61442f` | completed | 31 | 14,065 | 51,253 | 930,048 | 13,103 | 0 | 0.0210 | 115.1 | 114 of 119 |  |
| `matchbook` | 23 | L-N7 | `6a4b72603a` | completed | 30 | 15,838 | 41,373 | 854,528 | 11,495 | 0 | 0.0184 | 101.5 | 101 of 119 |  |
| `matchbook` | 23 | L-A0 | `01b988187b` | completed | 12 | 16,549 | 30,664 | 263,168 | 7,060 | 0 | 0.0092 | 53.9 | 0 of 119 | 97 / 18 / 4 |
| `matchbook` | 23 | L-A3 | `1fe2974ccf` | completed | 13 | 15,067 | 28,028 | 273,152 | 5,802 | 0 | 0.0084 | 43.7 | 98 of 119 |  |
| `matchbook` | 24 | L-N0 | `8e24cb53b3` | completed | 20 | 15,626 | 29,011 | 489,472 | 8,564 | 0 | 0.0121 | 69.9 | 107 of 119 |  |
| `matchbook` | 24 | L-N5 | `78574197ed` | completed | 36 | 15,620 | 74,037 | 1,082,368 | 10,748 | 0 | 0.0236 | 104.4 | 101 of 119 |  |
| `matchbook` | 24 | L-N6 | `68b9b4c956` | completed | 25 | 14,067 | 56,123 | 728,832 | 18,830 | 0 | 0.0223 | 373.5 | 116 of 119 |  |
| `matchbook` | 24 | L-N7 | `26df97c7ca` | completed | 33 | 15,834 | 71,922 | 1,094,400 | 19,969 | 5,587 | 0.0281 | 158.5 | 119 of 119 |  |
| `matchbook` | 24 | L-A0 | `5458c79ec2` | completed | 24 | 16,550 | 41,783 | 696,320 | 12,145 | 0 | 0.0172 | 99.1 | 0 of 119 | 98 / 15 / 6 |
| `matchbook` | 24 | L-A3 | `9fabdc5d06` | completed | 31 | 15,069 | 78,164 | 914,688 | 18,322 | 1,374 | 0.0261 | 139.2 | 111 of 119 |  |
| `matchbook` | 25 | L-N0 | `a687bcf7d4` | completed | 14 | 15,624 | 25,439 | 337,408 | 7,500 | 0 | 0.0097 | 156.3 | 0 of 119 | 104 / 13 / 2 |
| `matchbook` | 25 | L-N5 | `acdbe3145b` | completed | 20 | 15,622 | 49,993 | 489,472 | 9,387 | 0 | 0.0146 | 199.8 | 113 of 119 |  |
| `matchbook` | 25 | L-N6 | `bf40266d33` | completed | 44 | 14,063 | 76,031 | 1,574,912 | 20,992 | 0 | 0.0338 | 157.2 | 87 of 119 |  |
| `matchbook` | 25 | L-N7 | `33caddefb7` | completed | 26 | 15,830 | 40,115 | 771,584 | 12,315 | 106 | 0.0179 | 86.1 | 115 of 119 |  |
| `matchbook` | 25 | L-A0 | `c7371d998c` | completed | 21 | 16,542 | 38,710 | 675,584 | 14,620 | 0 | 0.0179 | 296.1 | 118 of 119 |  |
| `matchbook` | 25 | L-A3 | `0f4510959d` | completed | 16 | 15,068 | 26,713 | 407,552 | 8,597 | 0 | 0.0110 | 67.1 | 111 of 119 |  |
| `matchbook` | 26 | L-N0 | `d71fc00058` | completed | 36 | 15,620 | 59,699 | 1,101,824 | 15,090 | 0 | 0.0245 | 319.4 | 112 of 119 |  |
| `matchbook` | 26 | L-N5 | `e19f7c86e8` | completed | 30 | 15,626 | 50,466 | 903,680 | 16,279 | 4,859 | 0.0222 | 140.1 | 0 of 119 | 109 / 8 / 2 |
| `matchbook` | 26 | L-N6 | `3ae5d52ac0` | completed | 14 | 14,067 | 34,398 | 320,000 | 8,616 | 0 | 0.0109 | 61.2 | 102 of 119 |  |
| `matchbook` | 26 | L-N7 | `2d4e97830f` | completed | 27 | 15,838 | 67,139 | 749,824 | 8,922 | 0 | 0.0187 | 75.7 | 81 of 119 |  |
| `matchbook` | 26 | L-A0 | `3fe4628668` | completed | 21 | 16,545 | 41,061 | 524,032 | 10,185 | 0 | 0.0144 | 79.6 | 0 of 119 | 104 / 13 / 2 |
| `matchbook` | 26 | L-A3 | `7b8217f017` | completed | 24 | 15,062 | 40,622 | 683,008 | 11,485 | 0 | 0.0166 | 83.5 | 0 of 119 | 101 / 17 / 1 |
| `gridcalc` | 21 | L-N0 | `93d0aa60ac` | completed | 116 | 15,599 | 170,111 | 6,709,248 | 57,281 | 18,882 | 0.1127 | 512.2 | 151 of 154 |  |
| `gridcalc` | 21 | L-N5 | `7b68052170` | completed | 30 | 15,597 | 44,153 | 830,976 | 13,173 | 0 | 0.0193 | 113.4 | 102 of 154 |  |
| `gridcalc` | 21 | L-N6 | `490182d24b` | completed | 39 | 14,038 | 94,408 | 1,215,744 | 18,385 | 0 | 0.0308 | 382.1 | 137 of 154 |  |
| `gridcalc` | 21 | L-N7 | `c7cafaa34a` | completed | 54 | 15,805 | 67,925 | 1,893,888 | 17,398 | 0 | 0.0344 | 151.8 | 114 of 154 |  |
| `gridcalc` | 21 | L-A0 | `b2496f8e3e` | completed | 28 | 16,529 | 48,060 | 858,112 | 16,769 | 3,510 | 0.0218 | 136.3 | 123 of 154 |  |
| `gridcalc` | 21 | L-A3 | `4d88ab0f64` | completed | 53 | 15,043 | 69,990 | 1,835,776 | 22,817 | 0 | 0.0368 | 192.5 | 141 of 154 |  |
| `gridcalc` | 22 | L-N0 | `ee4e59f0f6` | completed | 54 | 15,603 | 77,451 | 2,076,160 | 17,296 | 0 | 0.0372 | 161.7 | 139 of 154 |  |
| `gridcalc` | 22 | L-N5 | `d4ed8fdc7a` | completed | 72 | 15,601 | 83,128 | 2,799,616 | 22,626 | 0 | 0.0476 | 178.2 | 127 of 154 |  |
| `gridcalc` | 22 | L-N6 | `7acbbac01c` | completed | 42 | 14,040 | 66,167 | 1,390,080 | 21,718 | 0 | 0.0314 | 159.6 | 130 of 154 |  |
| `gridcalc` | 22 | L-N7 | `aa0da7496e` | completed | 44 | 15,809 | 62,166 | 1,521,664 | 22,577 | 4,311 | 0.0327 | 469.7 | 147 of 154 |  |
| `gridcalc` | 22 | L-A0 | `d5b8cbb30b` | completed | 72 | 16,530 | 104,711 | 3,183,616 | 24,910 | 0 | 0.0548 | 220.7 | 121 of 154 |  |
| `gridcalc` | 22 | L-A3 | `691d399fbc` | completed | 71 | 15,035 | 100,357 | 2,881,792 | 22,617 | 0 | 0.0502 | 202.5 | 128 of 154 |  |
| `gridcalc` | 23 | L-N0 | `53a3a8a386` | completed | 25 | 15,601 | 49,615 | 733,952 | 14,577 | 81 | 0.0196 | 109.6 | 116 of 154 |  |
| `gridcalc` | 23 | L-N5 | `876d270087` | completed | 25 | 15,595 | 47,233 | 769,792 | 24,568 | 9,546 | 0.0247 | 477.6 | 132 of 154 |  |
| `gridcalc` | 23 | L-N6 | `ef03410694` | completed | 61 | 14,036 | 84,684 | 2,275,072 | 22,472 | 0 | 0.0425 | 190.5 | 128 of 154 |  |
| `gridcalc` | 23 | L-N7 | `7b158bfec6` | completed | 50 | 15,809 | 59,303 | 1,718,784 | 17,006 | 0 | 0.0316 | 459.1 | 117 of 154 |  |
| `gridcalc` | 23 | L-A0 | `6b7d60753c` | completed | 42 | 16,531 | 65,888 | 1,444,352 | 19,094 | 0 | 0.0306 | 158.6 | 113 of 154 |  |
| `gridcalc` | 23 | L-A3 | `b3f3df625d` | completed | 59 | 15,042 | 85,043 | 2,125,312 | 20,805 | 0 | 0.0402 | 174.9 | 134 of 154 |  |
| `gridcalc` | 24 | L-N0 | `377cf1598c` | completed | 72 | 15,597 | 107,411 | 3,436,544 | 33,959 | 10,101 | 0.0621 | 275.6 | 141 of 154 |  |
| `gridcalc` | 24 | L-N5 | `390cda419f` | completed | 49 | 15,597 | 66,688 | 1,719,040 | 14,648 | 0 | 0.0312 | 137.9 | 117 of 154 |  |
| `gridcalc` | 24 | L-N6 | `90c68dcd2c` | completed | 55 | 14,042 | 72,419 | 1,905,920 | 18,916 | 0 | 0.0358 | 169.4 | 138 of 154 |  |
| `gridcalc` | 24 | L-N7 | `fee5514755` | completed | 50 | 15,801 | 71,659 | 2,014,720 | 18,610 | 0 | 0.0366 | 146.7 | 130 of 154 |  |
| `gridcalc` | 24 | L-A0 | `9afff0b6e1` | completed | 48 | 16,536 | 68,332 | 1,829,888 | 22,864 | 0 | 0.0366 | 190.1 | 134 of 154 |  |
| `gridcalc` | 24 | L-A3 | `2f9643e894` | completed | 68 | 15,039 | 96,302 | 2,823,168 | 22,298 | 0 | 0.0490 | 485.8 | 133 of 154 |  |
| `gridcalc` | 25 | L-N0 | `19df0850b3` | completed | 53 | 15,599 | 82,216 | 1,840,896 | 18,921 | 0 | 0.0361 | 161.0 | 134 of 154 |  |
| `gridcalc` | 25 | L-N5 | `7b91fbadd1` | completed | 62 | 15,599 | 101,747 | 2,601,472 | 26,159 | 0 | 0.0493 | 224.6 | 136 of 154 |  |
| `gridcalc` | 25 | L-N6 | `5207ab4d0b` | completed | 33 | 14,042 | 52,076 | 1,008,384 | 15,287 | 0 | 0.0229 | 317.6 | 117 of 154 |  |
| `gridcalc` | 25 | L-N7 | `ec628c69fa` | completed | 36 | 15,801 | 54,052 | 1,209,344 | 23,108 | 5,803 | 0.0291 | 194.0 | 141 of 154 |  |
| `gridcalc` | 25 | L-A0 | `4fff7b6301` | completed | 28 | 16,527 | 41,460 | 820,224 | 12,640 | 0 | 0.0187 | 113.2 | 0 of 154 | 96 / 49 / 9 |
| `gridcalc` | 25 | L-A3 | `b810578775` | completed | 98 | 15,028 | 112,432 | 3,933,696 | 26,857 | 0 | 0.0640 | 295.5 | 143 of 154 |  |
| `gridcalc` | 26 | L-N0 | `ae78fae46e` | completed | 20 | 15,599 | 37,889 | 589,824 | 14,479 | 330 | 0.0169 | 287.0 | 139 of 154 |  |
| `gridcalc` | 26 | L-N5 | `9468fc0030` | completed | 43 | 15,597 | 57,610 | 1,368,320 | 19,663 | 0 | 0.0293 | 147.6 | 111 of 154 |  |
| `gridcalc` | 26 | L-N6 | `18cfb449e0` | completed | 44 | 14,040 | 66,452 | 1,461,248 | 25,473 | 7,699 | 0.0340 | 219.8 | 125 of 154 |  |
| `gridcalc` | 26 | L-N7 | `2962658496` | completed | 45 | 15,801 | 62,469 | 1,509,120 | 19,932 | 0 | 0.0313 | 159.3 | 135 of 154 |  |
| `gridcalc` | 26 | L-A0 | `fb63427b3c` | completed | 44 | 16,527 | 78,838 | 1,884,160 | 25,015 | 0 | 0.0392 | 510.0 | 131 of 154 |  |
| `gridcalc` | 26 | L-A3 | `73073388ae` | completed | 104 | 15,037 | 139,171 | 5,809,152 | 44,626 | 0 | 0.0943 | 367.3 | 133 of 154 |  |

**Twelve trials were graded 0 by the grader's time limit**, not by a count
of failed checks: on `matchbook` 2 of 6 in L-N0, 2 in L-N5, 4 in L-A0 and 3
in L-A3, none in L-N6 or L-N7; on `gridcalc` 1 in L-A0. The grader runs a
case's hidden tests in one process, and a test that never returns takes the
whole grade with it. The last column is a second look at each of the twelve:
every hidden test run on its own in a copy of the trial's final files, with
5 seconds per test. It is not the grade. With it the twelve passed 92 to 109
of their checks, and 1 to 9 tests hung in each. On `matchbook` the hanging
tests are again in iceberg matching and in the journal's random histories,
as iteration 2 found, in native trials as in Aether ones; the one `gridcalc`
trial hangs in cycle detection.

Hidden checks are therefore read twice below: as counted, and with the
per-test count in place of the twelve zeros. The second reading is the
steadier one and is still not a grade.

### Per arm, every trial

**API requests, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 36 | 23 | 57 | 20 | 14 | 36 | 31.0 (6) |
| L-N5 | 32 | 33 | 40 | 36 | 20 | 30 | 31.8 (6) |
| L-N6 | 27 | 63 | 31 | 25 | 44 | 14 | 34.0 (6) |
| L-N7 | 23 | 37 | 30 | 33 | 26 | 27 | 29.3 (6) |
| L-A0 | 12 | 23 | 12 | 24 | 21 | 21 | 18.8 (6) |
| L-A3 | 26 | 43 | 13 | 31 | 16 | 24 | 25.5 (6) |

**First-request input tokens, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 15,622 | 15,622 | 15,626 | 15,626 | 15,624 | 15,620 | 15,623.3 (6) |
| L-N5 | 15,622 | 15,624 | 15,622 | 15,620 | 15,622 | 15,626 | 15,622.7 (6) |
| L-N6 | 14,065 | 14,065 | 14,065 | 14,067 | 14,063 | 14,067 | 14,065.3 (6) |
| L-N7 | 15,834 | 15,830 | 15,838 | 15,834 | 15,830 | 15,838 | 15,834.0 (6) |
| L-A0 | 16,546 | 16,545 | 16,549 | 16,550 | 16,542 | 16,545 | 16,546.2 (6) |
| L-A3 | 15,066 | 15,065 | 15,067 | 15,069 | 15,068 | 15,062 | 15,066.2 (6) |

**Uncached input tokens, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 84,705 | 28,405 | 74,050 | 29,011 | 25,439 | 59,699 | 50,218.2 (6) |
| L-N5 | 43,366 | 45,961 | 56,696 | 74,037 | 49,993 | 50,466 | 53,419.8 (6) |
| L-N6 | 53,347 | 99,669 | 51,253 | 56,123 | 76,031 | 34,398 | 61,803.5 (6) |
| L-N7 | 34,923 | 68,366 | 41,373 | 71,922 | 40,115 | 67,139 | 53,973.0 (6) |
| L-A0 | 26,639 | 45,528 | 30,664 | 41,783 | 38,710 | 41,061 | 37,397.5 (6) |
| L-A3 | 47,708 | 61,423 | 28,028 | 78,164 | 26,713 | 40,622 | 47,109.7 (6) |

**Cache read tokens, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 1,198,080 | 578,816 | 2,169,600 | 489,472 | 337,408 | 1,101,824 | 979,200.0 (6) |
| L-N5 | 890,880 | 951,040 | 1,263,616 | 1,082,368 | 489,472 | 903,680 | 930,176.0 (6) |
| L-N6 | 781,568 | 2,423,040 | 930,048 | 728,832 | 1,574,912 | 320,000 | 1,126,400.0 (6) |
| L-N7 | 590,080 | 1,099,520 | 854,528 | 1,094,400 | 771,584 | 749,824 | 859,989.3 (6) |
| L-A0 | 297,984 | 669,952 | 263,168 | 696,320 | 675,584 | 524,032 | 521,173.3 (6) |
| L-A3 | 800,256 | 1,341,696 | 273,152 | 914,688 | 407,552 | 683,008 | 736,725.3 (6) |

**Output tokens, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 14,851 | 8,233 | 18,861 | 8,564 | 7,500 | 15,090 | 12,183.2 (6) |
| L-N5 | 11,361 | 11,739 | 12,151 | 10,748 | 9,387 | 16,279 | 11,944.2 (6) |
| L-N6 | 13,694 | 21,645 | 13,103 | 18,830 | 20,992 | 8,616 | 16,146.7 (6) |
| L-N7 | 8,390 | 17,059 | 11,495 | 19,969 | 12,315 | 8,922 | 13,025.0 (6) |
| L-A0 | 8,236 | 17,170 | 7,060 | 12,145 | 14,620 | 10,185 | 11,569.3 (6) |
| L-A3 | 12,371 | 16,072 | 5,802 | 18,322 | 8,597 | 11,485 | 12,108.2 (6) |

**Reasoning tokens, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0 | 0 | 0 | 0 | 0 | 0 | 0.0 (6) |
| L-N5 | 0 | 0 | 0 | 0 | 0 | 4,859 | 809.8 (6) |
| L-N6 | 0 | 0 | 0 | 0 | 0 | 0 | 0.0 (6) |
| L-N7 | 0 | 1,718 | 0 | 5,587 | 106 | 0 | 1,235.2 (6) |
| L-A0 | 0 | 4,156 | 0 | 0 | 0 | 0 | 692.7 (6) |
| L-A3 | 0 | 224 | 0 | 1,374 | 0 | 0 | 266.3 (6) |

**API-equivalent USD, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0.0279 | 0.0127 | 0.0385 | 0.0121 | 0.0097 | 0.0245 | 0.0209 (6) |
| L-N5 | 0.0189 | 0.0200 | 0.0244 | 0.0236 | 0.0146 | 0.0222 | 0.0206 (6) |
| L-N6 | 0.0200 | 0.0450 | 0.0210 | 0.0223 | 0.0338 | 0.0109 | 0.0255 (6) |
| L-N7 | 0.0136 | 0.0264 | 0.0184 | 0.0281 | 0.0179 | 0.0187 | 0.0205 (6) |
| L-A0 | 0.0098 | 0.0198 | 0.0092 | 0.0172 | 0.0179 | 0.0144 | 0.0147 (6) |
| L-A3 | 0.0190 | 0.0276 | 0.0084 | 0.0261 | 0.0110 | 0.0166 | 0.0181 (6) |

**Wall seconds, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 316.6 | 68.8 | 171.0 | 69.9 | 156.3 | 319.4 | 183.7 (6) |
| L-N5 | 97.7 | 91.7 | 281.6 | 104.4 | 199.8 | 140.1 | 152.6 (6) |
| L-N6 | 106.0 | 186.8 | 115.1 | 373.5 | 157.2 | 61.2 | 166.6 (6) |
| L-N7 | 77.6 | 145.4 | 101.5 | 158.5 | 86.1 | 75.7 | 107.5 (6) |
| L-A0 | 55.9 | 340.9 | 53.9 | 99.1 | 296.1 | 79.6 | 154.2 (6) |
| L-A3 | 92.8 | 124.6 | 43.7 | 139.2 | 67.1 | 83.5 | 91.8 (6) |

**Hidden checks passed, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 118 | 101 | 0 | 107 | 0 | 112 | 73.0 (6) |
| L-N5 | 0 | 113 | 116 | 101 | 113 | 0 | 73.8 (6) |
| L-N6 | 98 | 95 | 114 | 116 | 87 | 102 | 102.0 (6) |
| L-N7 | 103 | 98 | 101 | 119 | 115 | 81 | 102.8 (6) |
| L-A0 | 0 | 106 | 0 | 0 | 118 | 0 | 37.3 (6) |
| L-A3 | 0 | 0 | 98 | 111 | 111 | 0 | 53.3 (6) |

**Hidden checks, with the per-test count where the grader timed out, `matchbook`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 118 | 101 | 103 | 107 | 104 | 112 | 107.5 (6) |
| L-N5 | 98 | 113 | 116 | 101 | 113 | 109 | 108.3 (6) |
| L-N6 | 98 | 95 | 114 | 116 | 87 | 102 | 102.0 (6) |
| L-N7 | 103 | 98 | 101 | 119 | 115 | 81 | 102.8 (6) |
| L-A0 | 92 | 106 | 97 | 98 | 118 | 104 | 102.5 (6) |
| L-A3 | 101 | 99 | 98 | 111 | 111 | 101 | 103.5 (6) |

**API requests, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 116 | 54 | 25 | 72 | 53 | 20 | 56.7 (6) |
| L-N5 | 30 | 72 | 25 | 49 | 62 | 43 | 46.8 (6) |
| L-N6 | 39 | 42 | 61 | 55 | 33 | 44 | 45.7 (6) |
| L-N7 | 54 | 44 | 50 | 50 | 36 | 45 | 46.5 (6) |
| L-A0 | 28 | 72 | 42 | 48 | 28 | 44 | 43.7 (6) |
| L-A3 | 53 | 71 | 59 | 68 | 98 | 104 | 75.5 (6) |

**First-request input tokens, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 15,599 | 15,603 | 15,601 | 15,597 | 15,599 | 15,599 | 15,599.7 (6) |
| L-N5 | 15,597 | 15,601 | 15,595 | 15,597 | 15,599 | 15,597 | 15,597.7 (6) |
| L-N6 | 14,038 | 14,040 | 14,036 | 14,042 | 14,042 | 14,040 | 14,039.7 (6) |
| L-N7 | 15,805 | 15,809 | 15,809 | 15,801 | 15,801 | 15,801 | 15,804.3 (6) |
| L-A0 | 16,529 | 16,530 | 16,531 | 16,536 | 16,527 | 16,527 | 16,530.0 (6) |
| L-A3 | 15,043 | 15,035 | 15,042 | 15,039 | 15,028 | 15,037 | 15,037.3 (6) |

**Uncached input tokens, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 170,111 | 77,451 | 49,615 | 107,411 | 82,216 | 37,889 | 87,448.8 (6) |
| L-N5 | 44,153 | 83,128 | 47,233 | 66,688 | 101,747 | 57,610 | 66,759.8 (6) |
| L-N6 | 94,408 | 66,167 | 84,684 | 72,419 | 52,076 | 66,452 | 72,701.0 (6) |
| L-N7 | 67,925 | 62,166 | 59,303 | 71,659 | 54,052 | 62,469 | 62,929.0 (6) |
| L-A0 | 48,060 | 104,711 | 65,888 | 68,332 | 41,460 | 78,838 | 67,881.5 (6) |
| L-A3 | 69,990 | 100,357 | 85,043 | 96,302 | 112,432 | 139,171 | 100,549.2 (6) |

**Cache read tokens, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 6,709,248 | 2,076,160 | 733,952 | 3,436,544 | 1,840,896 | 589,824 | 2,564,437.3 (6) |
| L-N5 | 830,976 | 2,799,616 | 769,792 | 1,719,040 | 2,601,472 | 1,368,320 | 1,681,536.0 (6) |
| L-N6 | 1,215,744 | 1,390,080 | 2,275,072 | 1,905,920 | 1,008,384 | 1,461,248 | 1,542,741.3 (6) |
| L-N7 | 1,893,888 | 1,521,664 | 1,718,784 | 2,014,720 | 1,209,344 | 1,509,120 | 1,644,586.7 (6) |
| L-A0 | 858,112 | 3,183,616 | 1,444,352 | 1,829,888 | 820,224 | 1,884,160 | 1,670,058.7 (6) |
| L-A3 | 1,835,776 | 2,881,792 | 2,125,312 | 2,823,168 | 3,933,696 | 5,809,152 | 3,234,816.0 (6) |

**Output tokens, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 57,281 | 17,296 | 14,577 | 33,959 | 18,921 | 14,479 | 26,085.5 (6) |
| L-N5 | 13,173 | 22,626 | 24,568 | 14,648 | 26,159 | 19,663 | 20,139.5 (6) |
| L-N6 | 18,385 | 21,718 | 22,472 | 18,916 | 15,287 | 25,473 | 20,375.2 (6) |
| L-N7 | 17,398 | 22,577 | 17,006 | 18,610 | 23,108 | 19,932 | 19,771.8 (6) |
| L-A0 | 16,769 | 24,910 | 19,094 | 22,864 | 12,640 | 25,015 | 20,215.3 (6) |
| L-A3 | 22,817 | 22,617 | 20,805 | 22,298 | 26,857 | 44,626 | 26,670.0 (6) |

**Reasoning tokens, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 18,882 | 0 | 81 | 10,101 | 0 | 330 | 4,899.0 (6) |
| L-N5 | 0 | 0 | 9,546 | 0 | 0 | 0 | 1,591.0 (6) |
| L-N6 | 0 | 0 | 0 | 0 | 0 | 7,699 | 1,283.2 (6) |
| L-N7 | 0 | 4,311 | 0 | 0 | 5,803 | 0 | 1,685.7 (6) |
| L-A0 | 3,510 | 0 | 0 | 0 | 0 | 0 | 585.0 (6) |
| L-A3 | 0 | 0 | 0 | 0 | 0 | 0 | 0.0 (6) |

**API-equivalent USD, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0.1127 | 0.0372 | 0.0196 | 0.0621 | 0.0361 | 0.0169 | 0.0474 (6) |
| L-N5 | 0.0193 | 0.0476 | 0.0247 | 0.0312 | 0.0493 | 0.0293 | 0.0336 (6) |
| L-N6 | 0.0308 | 0.0314 | 0.0425 | 0.0358 | 0.0229 | 0.0340 | 0.0329 (6) |
| L-N7 | 0.0344 | 0.0327 | 0.0316 | 0.0366 | 0.0291 | 0.0313 | 0.0326 (6) |
| L-A0 | 0.0218 | 0.0548 | 0.0306 | 0.0366 | 0.0187 | 0.0392 | 0.0336 (6) |
| L-A3 | 0.0368 | 0.0502 | 0.0402 | 0.0490 | 0.0640 | 0.0943 | 0.0557 (6) |

**Wall seconds, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 512.2 | 161.7 | 109.6 | 275.6 | 161.0 | 287.0 | 251.2 (6) |
| L-N5 | 113.4 | 178.2 | 477.6 | 137.9 | 224.6 | 147.6 | 213.2 (6) |
| L-N6 | 382.1 | 159.6 | 190.5 | 169.4 | 317.6 | 219.8 | 239.8 (6) |
| L-N7 | 151.8 | 469.7 | 459.1 | 146.7 | 194.0 | 159.3 | 263.4 (6) |
| L-A0 | 136.3 | 220.7 | 158.6 | 190.1 | 113.2 | 510.0 | 221.5 (6) |
| L-A3 | 192.5 | 202.5 | 174.9 | 485.8 | 295.5 | 367.3 | 286.4 (6) |

**Hidden checks passed, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 151 | 139 | 116 | 141 | 134 | 139 | 136.7 (6) |
| L-N5 | 102 | 127 | 132 | 117 | 136 | 111 | 120.8 (6) |
| L-N6 | 137 | 130 | 128 | 138 | 117 | 125 | 129.2 (6) |
| L-N7 | 114 | 147 | 117 | 130 | 141 | 135 | 130.7 (6) |
| L-A0 | 123 | 121 | 113 | 134 | 0 | 131 | 103.7 (6) |
| L-A3 | 141 | 128 | 134 | 133 | 143 | 133 | 135.3 (6) |

**Hidden checks, with the per-test count where the grader timed out, `gridcalc`**

| Arm | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean (n) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 151 | 139 | 116 | 141 | 134 | 139 | 136.7 (6) |
| L-N5 | 102 | 127 | 132 | 117 | 136 | 111 | 120.8 (6) |
| L-N6 | 137 | 130 | 128 | 138 | 117 | 125 | 129.2 (6) |
| L-N7 | 114 | 147 | 117 | 130 | 141 | 135 | 130.7 (6) |
| L-A0 | 123 | 121 | 113 | 134 | 96 | 131 | 119.7 (6) |
| L-A3 | 141 | 128 | 134 | 133 | 143 | 133 | 135.3 (6) |

### What the solvers did

Means over an arm's trials, from the session logs. A test run is a command
that names `pytest` or `unittest`; a command that only reads is one that
opens with `cat`, `sed -n`, `rg`, `grep`, `ls`, `nl`, `head`, `tail`, `find`,
`wc` or a read-only `git` subcommand.

| Case | Arm | n | Commands | Commands that exited non-zero | Test runs | Commands that only read | File changes | Messages to the user | Reasoning items | Trials with reasoning tokens | Escalation requests | Messages about permission | Aether tool calls | Other events |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `matchbook` | L-N0 | 6 | 22.2 | 2.0 | 5.7 | 5.8 | 6.5 | 9.5 | 0.0 | 0 of 6 | 0 | 0 | 0 | 0 |
| `matchbook` | L-N5 | 6 | 20.0 | 2.7 | 3.8 | 6.8 | 9.0 | 6.8 | 3.2 | 1 of 6 | 0 | 0 | 0 | 0 |
| `matchbook` | L-N6 | 6 | 22.5 | 5.7 | 9.8 | 6.7 | 13.3 | 9.3 | 0.0 | 0 of 6 | 0 | 0 | 0 | 0 |
| `matchbook` | L-N7 | 6 | 18.0 | 2.2 | 4.8 | 5.2 | 9.8 | 7.5 | 4.3 | 3 of 6 | 0 | 0 | 0 | 0 |
| `matchbook` | L-A0 | 6 | 11.3 | 1.7 | 4.0 | 3.2 | 5.7 | 5.5 | 2.7 | 1 of 6 | 0 | 0 | 0 | 0 |
| `matchbook` | L-A3 | 6 | 16.5 | 2.8 | 5.2 | 3.8 | 6.8 | 7.2 | 1.5 | 2 of 6 | 0 | 0 | 0 | 0 |
| `gridcalc` | L-N0 | 6 | 29.2 | 6.0 | 9.2 | 8.0 | 25.0 | 13.5 | 20.3 | 4 of 6 | 0 | 0 | 0 | 0 |
| `gridcalc` | L-N5 | 6 | 24.3 | 3.0 | 3.7 | 6.2 | 25.7 | 14.7 | 4.8 | 1 of 6 | 0 | 0 | 0 | 0 |
| `gridcalc` | L-N6 | 6 | 28.3 | 4.7 | 8.0 | 7.7 | 19.5 | 13.8 | 5.8 | 1 of 6 | 0 | 0 | 0 | 0 |
| `gridcalc` | L-N7 | 6 | 22.0 | 5.7 | 7.8 | 4.7 | 21.2 | 12.3 | 7.5 | 2 of 6 | 0 | 0 | 0 | 0 |
| `gridcalc` | L-A0 | 6 | 30.5 | 7.8 | 9.2 | 9.0 | 20.7 | 12.2 | 1.8 | 1 of 6 | 0 | 0 | 0 | 0 |
| `gridcalc` | L-A3 | 6 | 49.2 | 7.8 | 13.3 | 10.5 | 30.7 | 13.0 | 0.0 | 0 of 6 | 0 | 0 | 0 | 0 |

Per request, over all of an arm's trials:

| Case | Arm | Trials | Requests, all trials | Uncached input per request | Output per request | Cache read per request |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `matchbook` | L-N0 | 6 | 186 | 1,620 | 393 | 31,587 |
| `matchbook` | L-N5 | 6 | 191 | 1,678 | 375 | 29,220 |
| `matchbook` | L-N6 | 6 | 204 | 1,818 | 475 | 33,129 |
| `matchbook` | L-N7 | 6 | 176 | 1,840 | 444 | 29,318 |
| `matchbook` | L-A0 | 6 | 113 | 1,986 | 614 | 27,673 |
| `matchbook` | L-A3 | 6 | 153 | 1,847 | 475 | 28,891 |
| `gridcalc` | L-N0 | 6 | 340 | 1,543 | 460 | 45,255 |
| `gridcalc` | L-N5 | 6 | 281 | 1,425 | 430 | 35,905 |
| `gridcalc` | L-N6 | 6 | 274 | 1,592 | 446 | 33,783 |
| `gridcalc` | L-N7 | 6 | 279 | 1,353 | 425 | 35,367 |
| `gridcalc` | L-A0 | 6 | 262 | 1,555 | 463 | 38,246 |
| `gridcalc` | L-A3 | 6 | 453 | 1,332 | 353 | 42,845 |
| both | L-N0 | 12 | 526 | 1,570 | 437 | 40,422 |
| both | L-N5 | 12 | 472 | 1,528 | 408 | 33,200 |
| both | L-N6 | 12 | 478 | 1,688 | 458 | 33,504 |
| both | L-N7 | 12 | 455 | 1,542 | 432 | 33,027 |
| both | L-A0 | 12 | 375 | 1,684 | 509 | 35,060 |
| both | L-A3 | 12 | 606 | 1,462 | 384 | 39,322 |

- **No approval or escalation request in any of the 72 sessions**, the 12
  under `on-request` included: no tool call asked to leave the sandbox, no
  approval event was recorded and no message spoke of permission. Part 1
  found the same in 26 earlier sessions. Over 38 Aether sessions under
  `on-request` the count is 0 requests in 0 trials, and the 12 under `never`
  on the diagnostic binary show no command refused for want of approval.
- **No Aether tool was called** in any of the 24 Aether sessions.
- **Per request the arms are level.** Uncached input per request is 1,570
  tokens in L-N0, 1,684 in L-A0 and 1,462 in L-A3 over both cases. The
  native arms span 1,528 to 1,688 among themselves; L-A0 is inside that and
  L-A3 just below it. What an arm spends follows how many requests its runs
  made.
- **The arm without the permission text did more, in the mean, than the arm
  with it**: 16.5 commands a trial against 11.3 on `matchbook` and 49.2
  against 30.5 on `gridcalc`, more test runs (5.2 against 4.0, 13.3 against
  9.2) and more file changes (6.8 against 5.7, 30.7 against 20.7). These are
  means of 6; the count by block is under the contrasts.
- **Reasoning is not one-sided in this sitting.** Trials that reported
  reasoning tokens: 4 of 12 in L-N0, 2 in L-N5, 1 in L-N6, 5 in L-N7, 2 in
  L-A0 and 2 in L-A3. The earlier sessions had it in 14 of 26 native trials
  against 5 of 26 through Aether; here it is 12 of 48 against 4 of 24, and
  L-N5, which is L-N0 run again, differs from L-N0 by as much as L-A0 does.

### Contrasts, per block

Each difference is the first arm minus the second within one block. "Lower /
higher / equal" counts blocks.

**L-A0 minus L-N0, `matchbook`** (is the overhead there in this sitting?)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -24 | +0 | -45 | +4 | +7 | -15 | -12.2 | -39.2 % | 3 / 2 / 1 | -45 | +7 |
| First-request input tokens | +924 | +923 | +923 | +924 | +918 | +925 | +922.8 | +5.9 % | 0 / 6 / 0 | +918 | +925 |
| Uncached input tokens | -58,066 | +17,123 | -43,386 | +12,772 | +13,271 | -18,638 | -12,820.7 | -25.5 % | 3 / 3 / 0 | -58,066 | +17,123 |
| Cache read tokens | -900,096 | +91,136 | -1,906,432 | +206,848 | +338,176 | -577,792 | -458,026.7 | -46.8 % | 3 / 3 / 0 | -1,906,432 | +338,176 |
| Output tokens | -6,615 | +8,937 | -11,801 | +3,581 | +7,120 | -4,905 | -613.8 | -5.0 % | 3 / 3 / 0 | -11,801 | +8,937 |
| Reasoning tokens | +0 | +4,156 | +0 | +0 | +0 | +0 | +692.7 | unknown | 0 / 1 / 5 | +0 | +4,156 |
| API-equivalent USD | -0.0181 | +0.0071 | -0.0293 | +0.0051 | +0.0083 | -0.0101 | -0.0062 | -29.5 % | 3 / 3 / 0 | -0.0293 | +0.0083 |
| Wall seconds | -260.7 | +272.1 | -117.2 | +29.3 | +139.8 | -239.7 | -29.4 | -16.0 % | 3 / 3 / 0 | -260.7 | +272.1 |
| Hidden checks passed | -118 | +5 | +0 | -107 | +118 | -112 | -35.7 | -48.9 % | 3 / 2 / 1 | -118 | +118 |
| Hidden checks, with the per-test count where the grader timed out | -26 | +5 | -6 | -9 | +14 | -8 | -5.0 | -4.7 % | 4 / 2 / 0 | -26 | +14 |

**L-A0 minus L-N0, `gridcalc`** (is the overhead there in this sitting?)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -88 | +18 | +17 | -24 | -25 | +24 | -13.0 | -22.9 % | 3 / 3 / 0 | -88 | +24 |
| First-request input tokens | +930 | +927 | +930 | +939 | +928 | +928 | +930.3 | +6.0 % | 0 / 6 / 0 | +927 | +939 |
| Uncached input tokens | -122,051 | +27,260 | +16,273 | -39,079 | -40,756 | +40,949 | -19,567.3 | -22.4 % | 3 / 3 / 0 | -122,051 | +40,949 |
| Cache read tokens | -5,851,136 | +1,107,456 | +710,400 | -1,606,656 | -1,020,672 | +1,294,336 | -894,378.7 | -34.9 % | 3 / 3 / 0 | -5,851,136 | +1,294,336 |
| Output tokens | -40,512 | +7,614 | +4,517 | -11,095 | -6,281 | +10,536 | -5,870.2 | -22.5 % | 3 / 3 / 0 | -40,512 | +10,536 |
| Reasoning tokens | -15,372 | +0 | -81 | -10,101 | +0 | -330 | -4,314.0 | -88.1 % | 4 / 0 / 2 | -15,372 | +0 |
| API-equivalent USD | -0.0910 | +0.0176 | +0.0110 | -0.0255 | -0.0174 | +0.0223 | -0.0138 | -29.2 % | 3 / 3 / 0 | -0.0910 | +0.0223 |
| Wall seconds | -375.9 | +59.0 | +49.0 | -85.5 | -47.9 | +223.0 | -29.7 | -11.8 % | 3 / 3 / 0 | -375.9 | +223.0 |
| Hidden checks passed | -28 | -18 | -3 | -7 | -134 | -8 | -33.0 | -24.1 % | 6 / 0 / 0 | -134 | -3 |
| Hidden checks, with the per-test count where the grader timed out | -28 | -18 | -3 | -7 | -38 | -8 | -17.0 | -12.4 % | 6 / 0 / 0 | -38 | -3 |

**L-N5 minus L-N0, `matchbook`** (asking `codex exec` for the policy Aether sets)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -4 | +10 | -17 | +16 | +6 | -6 | +0.8 | +2.7 % | 3 / 3 / 0 | -17 | +16 |
| First-request input tokens | +0 | +2 | -4 | -6 | -2 | +6 | -0.7 | -0.0 % | 3 / 2 / 1 | -6 | +6 |
| Uncached input tokens | -41,339 | +17,556 | -17,354 | +45,026 | +24,554 | -9,233 | +3,201.7 | +6.4 % | 3 / 3 / 0 | -41,339 | +45,026 |
| Cache read tokens | -307,200 | +372,224 | -905,984 | +592,896 | +152,064 | -198,144 | -49,024.0 | -5.0 % | 3 / 3 / 0 | -905,984 | +592,896 |
| Output tokens | -3,490 | +3,506 | -6,710 | +2,184 | +1,887 | +1,189 | -239.0 | -2.0 % | 2 / 4 / 0 | -6,710 | +3,506 |
| Reasoning tokens | +0 | +0 | +0 | +0 | +0 | +4,859 | +809.8 | unknown | 0 / 1 / 5 | +0 | +4,859 |
| API-equivalent USD | -0.0090 | +0.0072 | -0.0142 | +0.0115 | +0.0049 | -0.0023 | -0.0003 | -1.4 % | 3 / 3 / 0 | -0.0142 | +0.0115 |
| Wall seconds | -218.9 | +22.9 | +110.6 | +34.6 | +43.5 | -179.2 | -31.1 | -16.9 % | 2 / 4 / 0 | -218.9 | +110.6 |
| Hidden checks passed | -118 | +12 | +116 | -6 | +113 | -112 | +0.8 | +1.1 % | 3 / 3 / 0 | -118 | +116 |
| Hidden checks, with the per-test count where the grader timed out | -20 | +12 | +13 | -6 | +9 | -3 | +0.8 | +0.8 % | 3 / 3 / 0 | -20 | +13 |

**L-N5 minus L-N0, `gridcalc`** (asking `codex exec` for the policy Aether sets)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -86 | +18 | +0 | -23 | +9 | +23 | -9.8 | -17.4 % | 2 / 3 / 1 | -86 | +23 |
| First-request input tokens | -2 | -2 | -6 | +0 | +0 | -2 | -2.0 | -0.0 % | 4 / 0 / 2 | -6 | +0 |
| Uncached input tokens | -125,958 | +5,677 | -2,382 | -40,723 | +19,531 | +19,721 | -20,689.0 | -23.7 % | 3 / 3 / 0 | -125,958 | +19,721 |
| Cache read tokens | -5,878,272 | +723,456 | +35,840 | -1,717,504 | +760,576 | +778,496 | -882,901.3 | -34.4 % | 2 / 4 / 0 | -5,878,272 | +778,496 |
| Output tokens | -44,108 | +5,330 | +9,991 | -19,311 | +7,238 | +5,184 | -5,946.0 | -22.8 % | 2 / 4 / 0 | -44,108 | +9,991 |
| Reasoning tokens | -18,882 | +0 | +9,465 | -10,101 | +0 | -330 | -3,308.0 | -67.5 % | 3 / 1 / 2 | -18,882 | +9,465 |
| API-equivalent USD | -0.0934 | +0.0105 | +0.0051 | -0.0309 | +0.0132 | +0.0123 | -0.0139 | -29.2 % | 2 / 4 / 0 | -0.0934 | +0.0132 |
| Wall seconds | -398.9 | +16.5 | +368.0 | -137.6 | +63.6 | -139.4 | -38.0 | -15.1 % | 3 / 3 / 0 | -398.9 | +368.0 |
| Hidden checks passed | -49 | -12 | +16 | -24 | +2 | -28 | -15.8 | -11.6 % | 4 / 2 / 0 | -49 | +16 |
| Hidden checks, with the per-test count where the grader timed out | -49 | -12 | +16 | -24 | +2 | -28 | -15.8 | -11.6 % | 4 / 2 / 0 | -49 | +16 |

**L-A3 minus L-A0, `matchbook`** (taking the `on-request` policy away through Aether)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-A0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | +14 | +20 | +1 | +7 | -5 | +3 | +6.7 | +35.4 % | 1 / 5 / 0 | -5 | +20 |
| First-request input tokens | -1,480 | -1,480 | -1,482 | -1,481 | -1,474 | -1,483 | -1,480.0 | -8.9 % | 6 / 0 / 0 | -1,483 | -1,474 |
| Uncached input tokens | +21,069 | +15,895 | -2,636 | +36,381 | -11,997 | -439 | +9,712.2 | +26.0 % | 3 / 3 / 0 | -11,997 | +36,381 |
| Cache read tokens | +502,272 | +671,744 | +9,984 | +218,368 | -268,032 | +158,976 | +215,552.0 | +41.4 % | 1 / 5 / 0 | -268,032 | +671,744 |
| Output tokens | +4,135 | -1,098 | -1,258 | +6,177 | -6,023 | +1,300 | +538.8 | +4.7 % | 3 / 3 / 0 | -6,023 | +6,177 |
| Reasoning tokens | +0 | -3,932 | +0 | +1,374 | +0 | +0 | -426.3 | -61.5 % | 1 / 1 / 4 | -3,932 | +1,374 |
| API-equivalent USD | +0.0092 | +0.0078 | -0.0008 | +0.0089 | -0.0069 | +0.0022 | +0.0034 | +23.0 % | 2 / 4 / 0 | -0.0069 | +0.0092 |
| Wall seconds | +36.9 | -216.3 | -10.2 | +40.1 | -228.9 | +3.8 | -62.4 | -40.5 % | 3 / 3 / 0 | -228.9 | +40.1 |
| Hidden checks passed | +0 | -106 | +98 | +111 | -7 | +0 | +16.0 | +42.9 % | 2 / 2 / 2 | -106 | +111 |
| Hidden checks, with the per-test count where the grader timed out | +9 | -7 | +1 | +13 | -7 | -3 | +1.0 | +1.0 % | 3 / 3 / 0 | -7 | +13 |

**L-A3 minus L-A0, `gridcalc`** (taking the `on-request` policy away through Aether)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-A0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | +25 | -1 | +17 | +20 | +70 | +60 | +31.8 | +72.9 % | 1 / 5 / 0 | -1 | +70 |
| First-request input tokens | -1,486 | -1,495 | -1,489 | -1,497 | -1,499 | -1,490 | -1,492.7 | -9.0 % | 6 / 0 / 0 | -1,499 | -1,486 |
| Uncached input tokens | +21,930 | -4,354 | +19,155 | +27,970 | +70,972 | +60,333 | +32,667.7 | +48.1 % | 1 / 5 / 0 | -4,354 | +70,972 |
| Cache read tokens | +977,664 | -301,824 | +680,960 | +993,280 | +3,113,472 | +3,924,992 | +1,564,757.3 | +93.7 % | 1 / 5 / 0 | -301,824 | +3,924,992 |
| Output tokens | +6,048 | -2,293 | +1,711 | -566 | +14,217 | +19,611 | +6,454.7 | +31.9 % | 2 / 4 / 0 | -2,293 | +19,611 |
| Reasoning tokens | -3,510 | +0 | +0 | +0 | +0 | +0 | -585.0 | -100.0 % | 1 / 0 / 5 | -3,510 | +0 |
| API-equivalent USD | +0.0150 | -0.0046 | +0.0096 | +0.0124 | +0.0453 | +0.0551 | +0.0221 | +65.9 % | 1 / 5 / 0 | -0.0046 | +0.0551 |
| Wall seconds | +56.2 | -18.2 | +16.4 | +295.7 | +182.3 | -142.6 | +65.0 | +29.3 % | 2 / 4 / 0 | -142.6 | +295.7 |
| Hidden checks passed | +18 | +7 | +21 | -1 | +143 | +2 | +31.7 | +30.5 % | 1 / 5 / 0 | -1 | +143 |
| Hidden checks, with the per-test count where the grader timed out | +18 | +7 | +21 | -1 | +47 | +2 | +15.7 | +13.1 % | 1 / 5 / 0 | -1 | +47 |

**L-A3 minus L-N0, `matchbook`** (Aether without the `on-request` policy against native)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -10 | +20 | -44 | +11 | +2 | -12 | -5.5 | -17.7 % | 3 / 3 / 0 | -44 | +20 |
| First-request input tokens | -556 | -557 | -559 | -557 | -556 | -558 | -557.2 | -3.6 % | 6 / 0 / 0 | -559 | -556 |
| Uncached input tokens | -36,997 | +33,018 | -46,022 | +49,153 | +1,274 | -19,077 | -3,108.5 | -6.2 % | 3 / 3 / 0 | -46,022 | +49,153 |
| Cache read tokens | -397,824 | +762,880 | -1,896,448 | +425,216 | +70,144 | -418,816 | -242,474.7 | -24.8 % | 3 / 3 / 0 | -1,896,448 | +762,880 |
| Output tokens | -2,480 | +7,839 | -13,059 | +9,758 | +1,097 | -3,605 | -75.0 | -0.6 % | 3 / 3 / 0 | -13,059 | +9,758 |
| Reasoning tokens | +0 | +224 | +0 | +1,374 | +0 | +0 | +266.3 | unknown | 0 / 2 / 4 | +0 | +1,374 |
| API-equivalent USD | -0.0089 | +0.0149 | -0.0301 | +0.0140 | +0.0014 | -0.0079 | -0.0028 | -13.3 % | 3 / 3 / 0 | -0.0301 | +0.0149 |
| Wall seconds | -223.8 | +55.9 | -127.4 | +69.3 | -89.1 | -235.9 | -91.8 | -50.0 % | 4 / 2 / 0 | -235.9 | +69.3 |
| Hidden checks passed | -118 | -101 | +98 | +4 | +111 | -112 | -19.7 | -26.9 % | 3 / 3 / 0 | -118 | +111 |
| Hidden checks, with the per-test count where the grader timed out | -17 | -2 | -5 | +4 | +7 | -11 | -4.0 | -3.7 % | 4 / 2 / 0 | -17 | +7 |

**L-A3 minus L-N0, `gridcalc`** (Aether without the `on-request` policy against native)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -63 | +17 | +34 | -4 | +45 | +84 | +18.8 | +33.2 % | 2 / 4 / 0 | -63 | +84 |
| First-request input tokens | -556 | -568 | -559 | -558 | -571 | -562 | -562.3 | -3.6 % | 6 / 0 / 0 | -571 | -556 |
| Uncached input tokens | -100,121 | +22,906 | +35,428 | -11,109 | +30,216 | +101,282 | +13,100.3 | +15.0 % | 2 / 4 / 0 | -100,121 | +101,282 |
| Cache read tokens | -4,873,472 | +805,632 | +1,391,360 | -613,376 | +2,092,800 | +5,219,328 | +670,378.7 | +26.1 % | 2 / 4 / 0 | -4,873,472 | +5,219,328 |
| Output tokens | -34,464 | +5,321 | +6,228 | -11,661 | +7,936 | +30,147 | +584.5 | +2.2 % | 2 / 4 / 0 | -34,464 | +30,147 |
| Reasoning tokens | -18,882 | +0 | -81 | -10,101 | +0 | -330 | -4,899.0 | -100.0 % | 4 / 0 / 2 | -18,882 | +0 |
| API-equivalent USD | -0.0760 | +0.0130 | +0.0206 | -0.0131 | +0.0279 | +0.0774 | +0.0083 | +17.5 % | 2 / 4 / 0 | -0.0760 | +0.0774 |
| Wall seconds | -319.7 | +40.9 | +65.3 | +210.2 | +134.5 | +80.3 | +35.2 | +14.0 % | 1 / 5 / 0 | -319.7 | +210.2 |
| Hidden checks passed | -10 | -11 | +18 | -8 | +9 | -6 | -1.3 | -1.0 % | 4 / 2 / 0 | -11 | +18 |
| Hidden checks, with the per-test count where the grader timed out | -10 | -11 | +18 | -8 | +9 | -6 | -1.3 | -1.0 % | 4 / 2 / 0 | -11 | +18 |

**L-N6 minus L-N0, `matchbook`** (the multi-agent feature switched off, natively)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -9 | +40 | -26 | +5 | +30 | -22 | +3.0 | +9.7 % | 3 / 3 / 0 | -26 | +40 |
| First-request input tokens | -1,557 | -1,557 | -1,561 | -1,559 | -1,561 | -1,553 | -1,558.0 | -10.0 % | 6 / 0 / 0 | -1,561 | -1,553 |
| Uncached input tokens | -31,358 | +71,264 | -22,797 | +27,112 | +50,592 | -25,301 | +11,585.3 | +23.1 % | 3 / 3 / 0 | -31,358 | +71,264 |
| Cache read tokens | -416,512 | +1,844,224 | -1,239,552 | +239,360 | +1,237,504 | -781,824 | +147,200.0 | +15.0 % | 3 / 3 / 0 | -1,239,552 | +1,844,224 |
| Output tokens | -1,157 | +13,412 | -5,758 | +10,266 | +13,492 | -6,474 | +3,963.5 | +32.5 % | 3 / 3 / 0 | -6,474 | +13,492 |
| Reasoning tokens | +0 | +0 | +0 | +0 | +0 | +0 | +0.0 | unknown | 0 / 0 / 6 | +0 | +0 |
| API-equivalent USD | -0.0079 | +0.0323 | -0.0176 | +0.0102 | +0.0242 | -0.0136 | +0.0046 | +22.1 % | 3 / 3 / 0 | -0.0176 | +0.0323 |
| Wall seconds | -210.6 | +118.0 | -55.9 | +303.6 | +0.9 | -258.2 | -17.0 | -9.3 % | 3 / 3 / 0 | -258.2 | +303.6 |
| Hidden checks passed | -20 | -6 | +114 | +9 | +87 | -10 | +29.0 | +39.7 % | 3 / 3 / 0 | -20 | +114 |
| Hidden checks, with the per-test count where the grader timed out | -20 | -6 | +11 | +9 | -17 | -10 | -5.5 | -5.1 % | 4 / 2 / 0 | -20 | +11 |

**L-N6 minus L-N0, `gridcalc`** (the multi-agent feature switched off, natively)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -77 | -12 | +36 | -17 | -20 | +24 | -11.0 | -19.4 % | 4 / 2 / 0 | -77 | +36 |
| First-request input tokens | -1,561 | -1,563 | -1,565 | -1,555 | -1,557 | -1,559 | -1,560.0 | -10.0 % | 6 / 0 / 0 | -1,565 | -1,555 |
| Uncached input tokens | -75,703 | -11,284 | +35,069 | -34,992 | -30,140 | +28,563 | -14,747.8 | -16.9 % | 4 / 2 / 0 | -75,703 | +35,069 |
| Cache read tokens | -5,493,504 | -686,080 | +1,541,120 | -1,530,624 | -832,512 | +871,424 | -1,021,696.0 | -39.8 % | 4 / 2 / 0 | -5,493,504 | +1,541,120 |
| Output tokens | -38,896 | +4,422 | +7,895 | -15,043 | -3,634 | +10,994 | -5,710.3 | -21.9 % | 3 / 3 / 0 | -38,896 | +10,994 |
| Reasoning tokens | -18,882 | +0 | -81 | -10,101 | +0 | +7,369 | -3,615.8 | -73.8 % | 3 / 1 / 2 | -18,882 | +7,369 |
| API-equivalent USD | -0.0820 | -0.0058 | +0.0229 | -0.0263 | -0.0132 | +0.0171 | -0.0145 | -30.7 % | 4 / 2 / 0 | -0.0820 | +0.0229 |
| Wall seconds | -130.2 | -2.1 | +80.9 | -106.2 | +156.6 | -67.2 | -11.4 | -4.5 % | 4 / 2 / 0 | -130.2 | +156.6 |
| Hidden checks passed | -14 | -9 | +12 | -3 | -17 | -14 | -7.5 | -5.5 % | 5 / 1 / 0 | -17 | +12 |
| Hidden checks, with the per-test count where the grader timed out | -14 | -9 | +12 | -3 | -17 | -14 | -7.5 | -5.5 % | 5 / 1 / 0 | -17 | +12 |

**L-N7 minus L-N0, `matchbook`** (the task inside the Aether envelope, natively)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -13 | +14 | -27 | +13 | +12 | -9 | -1.7 | -5.4 % | 3 / 3 / 0 | -27 | +14 |
| First-request input tokens | +212 | +208 | +212 | +208 | +206 | +218 | +210.7 | +1.3 % | 0 / 6 / 0 | +206 | +218 |
| Uncached input tokens | -49,782 | +39,961 | -32,677 | +42,911 | +14,676 | +7,440 | +3,754.8 | +7.5 % | 2 / 4 / 0 | -49,782 | +42,911 |
| Cache read tokens | -608,000 | +520,704 | -1,315,072 | +604,928 | +434,176 | -352,000 | -119,210.7 | -12.2 % | 3 / 3 / 0 | -1,315,072 | +604,928 |
| Output tokens | -6,461 | +8,826 | -7,366 | +11,405 | +4,815 | -6,168 | +841.8 | +6.9 % | 3 / 3 / 0 | -7,366 | +11,405 |
| Reasoning tokens | +0 | +1,718 | +0 | +5,587 | +106 | +0 | +1,235.2 | unknown | 0 / 3 / 3 | +0 | +5,587 |
| API-equivalent USD | -0.0143 | +0.0136 | -0.0201 | +0.0160 | +0.0082 | -0.0059 | -0.0004 | -1.9 % | 3 / 3 / 0 | -0.0201 | +0.0160 |
| Wall seconds | -239.0 | +76.6 | -69.5 | +88.6 | -70.2 | -243.7 | -76.2 | -41.5 % | 4 / 2 / 0 | -243.7 | +88.6 |
| Hidden checks passed | -15 | -3 | +101 | +12 | +115 | -31 | +29.8 | +40.9 % | 3 / 3 / 0 | -31 | +115 |
| Hidden checks, with the per-test count where the grader timed out | -15 | -3 | -2 | +12 | +11 | -31 | -4.7 | -4.3 % | 4 / 2 / 0 | -31 | +12 |

**L-N7 minus L-N0, `gridcalc`** (the task inside the Aether envelope, natively)

| Measure | Block 21 | Block 22 | Block 23 | Block 24 | Block 25 | Block 26 | Mean difference | Relative to L-N0 mean | Lower / higher / equal | Smallest | Largest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | -62 | -10 | +25 | -22 | -17 | +25 | -10.2 | -17.9 % | 4 / 2 / 0 | -62 | +25 |
| First-request input tokens | +206 | +206 | +208 | +204 | +202 | +202 | +204.7 | +1.3 % | 0 / 6 / 0 | +202 | +208 |
| Uncached input tokens | -102,186 | -15,285 | +9,688 | -35,752 | -28,164 | +24,580 | -24,519.8 | -28.0 % | 4 / 2 / 0 | -102,186 | +24,580 |
| Cache read tokens | -4,815,360 | -554,496 | +984,832 | -1,421,824 | -631,552 | +919,296 | -919,850.7 | -35.9 % | 4 / 2 / 0 | -4,815,360 | +984,832 |
| Output tokens | -39,883 | +5,281 | +2,429 | -15,349 | +4,187 | +5,453 | -6,313.7 | -24.2 % | 2 / 4 / 0 | -39,883 | +5,453 |
| Reasoning tokens | -18,882 | +4,311 | -81 | -10,101 | +5,803 | -330 | -3,213.3 | -65.6 % | 4 / 2 / 0 | -18,882 | +5,803 |
| API-equivalent USD | -0.0783 | -0.0044 | +0.0120 | -0.0255 | -0.0070 | +0.0144 | -0.0148 | -31.2 % | 4 / 2 / 0 | -0.0783 | +0.0144 |
| Wall seconds | -360.4 | +308.0 | +349.5 | -128.9 | +33.0 | -127.7 | +12.2 | +4.9 % | 3 / 3 / 0 | -360.4 | +349.5 |
| Hidden checks passed | -37 | +8 | +1 | -11 | +7 | -4 | -6.0 | -4.4 % | 3 / 3 / 0 | -37 | +8 |
| Hidden checks, with the per-test count where the grader timed out | -37 | +8 | +1 | -11 | +7 | -4 | -6.0 | -4.4 % | 3 / 3 / 0 | -37 | +8 |

### Contrasts over both cases

The twelve pairs of each contrast together.

**L-A0 minus L-N0, both cases** (is the overhead there in this sitting?)

| Measure | `matchbook` lower / higher / equal | `gridcalc` lower / higher / equal | Both lower / higher / equal | Smallest | Largest | Mean difference | Relative to L-N0 mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 3 / 2 / 1 | 3 / 3 / 0 | 6 / 5 / 1 | -88 | +24 | -12.6 | -28.7 % |
| Uncached input tokens | 3 / 3 / 0 | 3 / 3 / 0 | 6 / 6 / 0 | -122,051 | +40,949 | -16,194.0 | -23.5 % |
| Output tokens | 3 / 3 / 0 | 3 / 3 / 0 | 6 / 6 / 0 | -40,512 | +10,536 | -3,242.0 | -16.9 % |
| Reasoning tokens | 0 / 1 / 5 | 4 / 0 / 2 | 4 / 1 / 7 | -15,372 | +4,156 | -1,810.7 | -73.9 % |
| API-equivalent USD | 3 / 3 / 0 | 3 / 3 / 0 | 6 / 6 / 0 | -0.0910 | +0.0223 | -0.0100 | -29.3 % |
| Wall seconds | 3 / 3 / 0 | 3 / 3 / 0 | 6 / 6 / 0 | -375.9 | +272.1 | -29.6 | -13.6 % |
| Hidden checks passed | 3 / 2 / 1 | 6 / 0 / 0 | 9 / 2 / 1 | -134 | +118 | -34.3 | -32.8 % |
| Hidden checks, with the per-test count where the grader timed out | 4 / 2 / 0 | 6 / 0 / 0 | 10 / 2 / 0 | -38 | +14 | -11.0 | -9.0 % |

**L-N5 minus L-N0, both cases** (asking `codex exec` for the policy Aether sets)

| Measure | `matchbook` lower / higher / equal | `gridcalc` lower / higher / equal | Both lower / higher / equal | Smallest | Largest | Mean difference | Relative to L-N0 mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 3 / 3 / 0 | 2 / 3 / 1 | 5 / 6 / 1 | -86 | +23 | -4.5 | -10.3 % |
| Uncached input tokens | 3 / 3 / 0 | 3 / 3 / 0 | 6 / 6 / 0 | -125,958 | +45,026 | -8,743.7 | -12.7 % |
| Output tokens | 2 / 4 / 0 | 2 / 4 / 0 | 4 / 8 / 0 | -44,108 | +9,991 | -3,092.5 | -16.2 % |
| Reasoning tokens | 0 / 1 / 5 | 3 / 1 / 2 | 3 / 2 / 7 | -18,882 | +9,465 | -1,249.1 | -51.0 % |
| API-equivalent USD | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -0.0934 | +0.0132 | -0.0071 | -20.7 % |
| Wall seconds | 2 / 4 / 0 | 3 / 3 / 0 | 5 / 7 / 0 | -398.9 | +368.0 | -34.5 | -15.9 % |
| Hidden checks passed | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -118 | +116 | -7.5 | -7.2 % |
| Hidden checks, with the per-test count where the grader timed out | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -49 | +16 | -7.5 | -6.1 % |

**L-A3 minus L-A0, both cases** (taking the `on-request` policy away through Aether)

| Measure | `matchbook` lower / higher / equal | `gridcalc` lower / higher / equal | Both lower / higher / equal | Smallest | Largest | Mean difference | Relative to L-A0 mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 1 / 5 / 0 | 1 / 5 / 0 | 2 / 10 / 0 | -5 | +70 | +19.2 | +61.6 % |
| Uncached input tokens | 3 / 3 / 0 | 1 / 5 / 0 | 4 / 8 / 0 | -11,997 | +70,972 | +21,189.9 | +40.3 % |
| Output tokens | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -6,023 | +19,611 | +3,496.8 | +22.0 % |
| Reasoning tokens | 1 / 1 / 4 | 1 / 0 / 5 | 2 / 1 / 9 | -3,932 | +1,374 | -505.7 | -79.2 % |
| API-equivalent USD | 2 / 4 / 0 | 1 / 5 / 0 | 3 / 9 / 0 | -0.0069 | +0.0551 | +0.0128 | +52.8 % |
| Wall seconds | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -228.9 | +295.7 | +1.3 | +0.7 % |
| Hidden checks passed | 2 / 2 / 2 | 1 / 5 / 0 | 3 / 7 / 2 | -106 | +143 | +23.8 | +33.8 % |
| Hidden checks, with the per-test count where the grader timed out | 3 / 3 / 0 | 1 / 5 / 0 | 4 / 8 / 0 | -7 | +47 | +8.3 | +7.5 % |

**L-A3 minus L-N0, both cases** (Aether without the `on-request` policy against native)

| Measure | `matchbook` lower / higher / equal | `gridcalc` lower / higher / equal | Both lower / higher / equal | Smallest | Largest | Mean difference | Relative to L-N0 mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -63 | +84 | +6.7 | +15.2 % |
| Uncached input tokens | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -100,121 | +101,282 | +4,995.9 | +7.3 % |
| Output tokens | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -34,464 | +30,147 | +254.8 | +1.3 % |
| Reasoning tokens | 0 / 2 / 4 | 4 / 0 / 2 | 4 / 2 / 6 | -18,882 | +1,374 | -2,316.3 | -94.6 % |
| API-equivalent USD | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -0.0760 | +0.0774 | +0.0028 | +8.1 % |
| Wall seconds | 4 / 2 / 0 | 1 / 5 / 0 | 5 / 7 / 0 | -319.7 | +210.2 | -28.3 | -13.0 % |
| Hidden checks passed | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -118 | +111 | -10.5 | -10.0 % |
| Hidden checks, with the per-test count where the grader timed out | 4 / 2 / 0 | 4 / 2 / 0 | 8 / 4 / 0 | -17 | +18 | -2.7 | -2.2 % |

**L-N6 minus L-N0, both cases** (the multi-agent feature switched off, natively)

| Measure | `matchbook` lower / higher / equal | `gridcalc` lower / higher / equal | Both lower / higher / equal | Smallest | Largest | Mean difference | Relative to L-N0 mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -77 | +40 | -4.0 | -9.1 % |
| Uncached input tokens | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -75,703 | +71,264 | -1,581.2 | -2.3 % |
| Output tokens | 3 / 3 / 0 | 3 / 3 / 0 | 6 / 6 / 0 | -38,896 | +13,492 | -873.4 | -4.6 % |
| Reasoning tokens | 0 / 0 / 6 | 3 / 1 / 2 | 3 / 1 / 8 | -18,882 | +7,369 | -1,807.9 | -73.8 % |
| API-equivalent USD | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -0.0820 | +0.0323 | -0.0050 | -14.5 % |
| Wall seconds | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -258.2 | +303.6 | -14.2 | -6.5 % |
| Hidden checks passed | 3 / 3 / 0 | 5 / 1 / 0 | 8 / 4 / 0 | -20 | +114 | +10.8 | +10.3 % |
| Hidden checks, with the per-test count where the grader timed out | 4 / 2 / 0 | 5 / 1 / 0 | 9 / 3 / 0 | -20 | +12 | -6.5 | -5.3 % |

**L-N7 minus L-N0, both cases** (the task inside the Aether envelope, natively)

| Measure | `matchbook` lower / higher / equal | `gridcalc` lower / higher / equal | Both lower / higher / equal | Smallest | Largest | Mean difference | Relative to L-N0 mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -62 | +25 | -5.9 | -13.5 % |
| Uncached input tokens | 2 / 4 / 0 | 4 / 2 / 0 | 6 / 6 / 0 | -102,186 | +42,911 | -10,382.5 | -15.1 % |
| Output tokens | 3 / 3 / 0 | 2 / 4 / 0 | 5 / 7 / 0 | -39,883 | +11,405 | -2,735.9 | -14.3 % |
| Reasoning tokens | 0 / 3 / 3 | 4 / 2 / 0 | 4 / 5 / 3 | -18,882 | +5,803 | -989.1 | -40.4 % |
| API-equivalent USD | 3 / 3 / 0 | 4 / 2 / 0 | 7 / 5 / 0 | -0.0783 | +0.0160 | -0.0076 | -22.2 % |
| Wall seconds | 4 / 2 / 0 | 3 / 3 / 0 | 7 / 5 / 0 | -360.4 | +349.5 | -32.0 | -14.7 % |
| Hidden checks passed | 3 / 3 / 0 | 3 / 3 / 0 | 6 / 6 / 0 | -37 | +115 | +11.9 | +11.4 % |
| Hidden checks, with the per-test count where the grader timed out | 4 / 2 / 0 | 3 / 3 / 0 | 7 / 5 / 0 | -37 | +12 | -5.3 | -4.4 % |

### How much each change accounts for

Mean differences set beside the matched pair's. The last line is the matched
pair's difference less the three isolated parts. It is arithmetic on means of
six or twelve differences that each span more than the mean itself, and it
adds parts measured natively to one measured through Aether, which assumes
they do not interact.

**Mean differences beside the matched pair's, `matchbook`**

| Difference | pairs | API requests | Uncached input tokens | Output tokens | API-equivalent USD | Hidden checks passed | Hidden checks, with the per-test count where the grader timed out |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The matched pair: L-A0 minus L-N0 | 6 | -12.2 | -12,820.7 | -613.8 | -0.0062 | -35.7 | -5.0 |
| The `on-request` policy through Aether: L-A0 minus L-A3 | 6 | -6.7 | -9,712.2 | -538.8 | -0.0034 | -16.0 | -1.0 |
| Multi-agent feature off, natively: L-N6 minus L-N0 | 6 | +3.0 | +11,585.3 | +3,963.5 | +0.0046 | +29.0 | -5.5 |
| The envelope, natively: L-N7 minus L-N0 | 6 | -1.7 | +3,754.8 | +841.8 | -0.0004 | +29.8 | -4.7 |
| A second native cell (policy asked for, not applied): L-N5 minus L-N0 | 6 | +0.8 | +3,201.7 | -239.0 | -0.0003 | +0.8 | +0.8 |
| Left for what no arm isolates (the first line minus the next three) | | -6.8 | -18,448.7 | -4,880.3 | -0.0070 | -78.5 | +6.2 |

**Mean differences beside the matched pair's, `gridcalc`**

| Difference | pairs | API requests | Uncached input tokens | Output tokens | API-equivalent USD | Hidden checks passed | Hidden checks, with the per-test count where the grader timed out |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The matched pair: L-A0 minus L-N0 | 6 | -13.0 | -19,567.3 | -5,870.2 | -0.0138 | -33.0 | -17.0 |
| The `on-request` policy through Aether: L-A0 minus L-A3 | 6 | -31.8 | -32,667.7 | -6,454.7 | -0.0221 | -31.7 | -15.7 |
| Multi-agent feature off, natively: L-N6 minus L-N0 | 6 | -11.0 | -14,747.8 | -5,710.3 | -0.0145 | -7.5 | -7.5 |
| The envelope, natively: L-N7 minus L-N0 | 6 | -10.2 | -24,519.8 | -6,313.7 | -0.0148 | -6.0 | -6.0 |
| A second native cell (policy asked for, not applied): L-N5 minus L-N0 | 6 | -9.8 | -20,689.0 | -5,946.0 | -0.0139 | -15.8 | -15.8 |
| Left for what no arm isolates (the first line minus the next three) | | +40.0 | +52,368.0 | +12,608.5 | +0.0377 | +12.2 | +12.2 |

**Mean differences beside the matched pair's, both cases**

| Difference | pairs | API requests | Uncached input tokens | Output tokens | API-equivalent USD | Hidden checks passed | Hidden checks, with the per-test count where the grader timed out |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| The matched pair: L-A0 minus L-N0 | 12 | -12.6 | -16,194.0 | -3,242.0 | -0.0100 | -34.3 | -11.0 |
| The `on-request` policy through Aether: L-A0 minus L-A3 | 12 | -19.2 | -21,189.9 | -3,496.8 | -0.0128 | -23.8 | -8.3 |
| Multi-agent feature off, natively: L-N6 minus L-N0 | 12 | -4.0 | -1,581.2 | -873.4 | -0.0050 | +10.8 | -6.5 |
| The envelope, natively: L-N7 minus L-N0 | 12 | -5.9 | -10,382.5 | -2,735.9 | -0.0076 | +11.9 | -5.3 |
| A second native cell (policy asked for, not applied): L-N5 minus L-N0 | 12 | -4.5 | -8,743.7 | -3,092.5 | -0.0071 | -7.5 | -7.5 |
| Left for what no arm isolates (the first line minus the next three) | | +16.6 | +16,959.7 | +3,864.1 | +0.0153 | -33.2 | +9.2 |

- **There is no overhead in this sitting to divide.** The matched pair's
  mean differences are all below zero: L-A0 made 12.6 fewer requests than
  L-N0 and took 16,194 fewer uncached input tokens, and by block it is lower
  in 6 and higher in 6.
- **The one part isolated through Aether is as large as the whole.** With
  the `on-request` policy L-A0 made 19.2 fewer requests than L-A3 without it
  and took 21,190 fewer uncached input tokens, against 12.6 and 16,194 for
  the matched pair. On the means the policy accounts for all of the matched
  pair's difference and more, in the direction opposite to the one the loop
  set out to explain.
- **The two native parts are no larger than the yardstick.** Multi-agent off
  (-1,581 uncached input tokens) and the envelope (-10,383) stand beside
  -8,744 for L-N5, which changes nothing. Neither can be told from a second
  run of the same cell.
- **What is left over changes sign with the case**: -18,449 uncached input
  tokens on `matchbook`, +52,368 on `gridcalc`, +16,960 over both. That is
  the size of the noise, not of the skill listing or of Aether's tools.
  Neither has an arm, and nothing here measures them apart. L-A3 minus L-N0
  holds both together with the envelope and the multi-agent setting, and is
  lower in 5 blocks and higher in 7 on requests, uncached input and cost.

### Verdict for Luna, iteration 4

**The cause is not located, and in this sitting there was no overhead to
locate.** Twelve pairs of one sitting do not settle a difference either way.

What the contrasts show, each on the twelve pairs of both cases:

- **L-A0 minus L-N0, the matched pair: no overhead.** Aether is lower in 6
  pairs and higher in 6 on uncached input (-122,051 to +40,949 tokens), on
  output tokens (-40,512 to +10,536), on cost (-0.0910 to +0.0223 USD) and
  on wall time; on requests it is lower in 6, higher in 5 and equal in 1
  (-88 to +24). The means are below zero on every one of them and are not
  the finding: a single pair spans more than any mean.
- **L-N5 minus L-N0, the same native cell twice: the yardstick.** Lower in 6
  and higher in 6 on uncached input (-125,958 to +45,026), 5 / 6 / 1 on
  requests (-86 to +23), 5 / 7 on cost (-0.0934 to +0.0132 USD). Two cells
  the model cannot tell apart differ by as much as the matched pair does.
  This arm says nothing about the approval policy.
- **L-A3 minus L-A0, the `on-request` policy taken away through Aether: not
  a saving.** Without the policy and its text the solver made more requests
  in 10 pairs and fewer in 2 (-5 to +70), took more uncached input in 8 and
  less in 4 (-11,997 to +70,972), and cost more in 9 and less in 3 (-0.0069
  to +0.0551 USD). It passed more hidden checks in 7 pairs, fewer in 3 and
  the same in 2 as counted (-106 to +143), and more in 8 and fewer in 4 with
  the per-test count (-7 to +47). The count on requests is, with one on
  hidden checks below, the most one-sided of the 48 in the tables over both
  cases. It is one count picked out of 48 after the fact, on 12 pairs, and
  it points away from what was expected.
- **L-A3 minus L-N0, Aether without the policy against native: level.**
  Lower in 5 and higher in 7 on requests (-63 to +84), on uncached input
  (-100,121 to +101,282) and on cost (-0.0760 to +0.0774 USD).
- **L-N6 minus L-N0, the multi-agent feature off: not one-sided.** Lower in
  7 and higher in 5 on requests (-77 to +40), on uncached input (-75,703 to
  +71,264) and on cost.
- **L-N7 minus L-N0, the envelope: not one-sided.** Lower in 6 and higher in
  6 on uncached input (-102,186 to +42,911), 7 / 5 on requests (-62 to +25)
  and on cost.

Hidden checks, which earlier iterations left open:

- **L-A0 is below L-N0 in 9 of the 12 pairs as counted** (higher in 2, equal
  in 1; -134 to +118) and in 10 of 12 with the per-test count (-38 to +14),
  all 6 on `gridcalc`. Earlier sittings had 12 of 16 blocks and 6 of 10
  counted pairs.
- **So is every other arm of this sitting**, with the per-test count: L-N5
  below L-N0 in 7 of 12, L-N6 in 9, L-N7 in 7, L-A3 in 8. L-N0 ran high
  here, and 10 of 12 against a cell that is beaten 7 times by its own repeat
  is not a finding about Aether.
- **4 of the 6 L-A0 trials on `matchbook` were graded 0 by the time limit**,
  against 2 of 6 in L-N0, 2 in L-N5 and 3 in L-A3. The counted difference in
  hidden checks on that case is mostly those zeros: 35.7 checks in the mean
  as counted, 5.0 with the per-test count.

Candidates, ranked by what this sitting measured for each:

1. **How long a run goes, which no arm controls.** *Measured.* Per request
   the arms are level, an arm's spend follows its request count, and the
   request count of one cell run twice in one block differs by up to 86.
   The matched pair was above native on uncached input in 18 of the 26
   earlier pairs and is in 6 of these 12: 24 of 38 in all, 14 the other way.
2. **The `on-request` permission text shortening the run.** *Hypothesis,
   with one measured count behind it.* With the text the solver made fewer
   requests in 10 of 12 pairs and passed fewer hidden checks in 7 or 8 of
   12. If that holds, the text costs accuracy and saves tokens. It was
   expected to do the opposite, and 12 pairs do not show it either way.
3. **The fixed size of the first turn.** *Measured, and small.* Every
   request of L-A0 carries about 925 input tokens more than L-N0's in its
   prefix, served from the cache after the first request; about 1,485 of
   them are the permission text, so without it the prefix is about 560
   tokens below native. It does not move a trial's spend by a visible
   amount beside the request count.
4. **The multi-agent feature being off.** *Hypothesis; not one-sided where
   measured* (7 pairs lower, 5 higher).
5. **The envelope.** *Hypothesis; not one-sided where measured* (6 and 6).
6. **The app-server's longer skill listing and Aether's own tools.**
   *Hypothesis; no arm.* They are in L-A3 minus L-N0 with the two above,
   which is 5 lower and 7 higher. No Aether tool was called in any trial.

What cannot be said: that the approval policy is the cause of Luna
spending more through Aether; that removing it would lower spend; that
Aether costs Luna hidden checks.

### What iteration 4 points to

**No change to Aether is supported for spend.** The difference to explain
did not appear in this sitting, and the candidate that could be isolated
moved the other way. The three courses open for the approval policy, with
what each costs in what Aether does:

**1. Approval `never` when the agent's policy gates nothing.**

- Where: `crates/aether-engine/src/codex_app.rs`, `thread_params`, lines
  189 and 190. The first pair, for a bypass turn with network, would read
  `("never", "danger-full-access")` in place of `("on-request",
  "danger-full-access")`. That is the patch of the diagnostic binary.
- What it changes for the model: 5,782 characters of permission
  instructions, about 1,485 tokens, leave the first turn and every
  request's prefix.
- What this sitting measured for it: arm L-A3. More requests in 10 of 12
  pairs, higher cost in 9 of 12, more hidden checks in 7 or 8 of 12. It is
  not a saving. Whether it buys accuracy is the hypothesis above.
- **What it costs in function: Aether stops answering the operator's
  prompt rules.** Under `never` Codex refuses outright any command that an
  operator `decision="prompt"` rule covers, `gh pr merge` among them, with
  no request for Aether to answer. Under `on-request` the prompt reaches
  Aether as a `requestApproval` and is decided under the agent's own
  policy, which for a bypass agent grants it. That was introduced on
  purpose and is described above the function. With the change an agent
  whose policy gates nothing could no longer run those commands at all.
  No trial here ran such a command, so the trials do not show this cost;
  the tests do.
- How to test: the two tests that hold the present behaviour would have to
  change with it, `only_online_bypass_turns_move_from_never_to_on_request`
  (`crates/aether-engine/src/codex_app/tests/launch.rs`, lines 771 to 780)
  and `a_bypass_turn_with_network_lets_aether_grant_a_native_prompt`
  (`crates/aether-engine/src/codex_app/tests/permissions.rs`, lines 94 to
  100). The second one failing is the cost stated above. For spend and
  accuracy, the measurement under 3.

**2. A shorter instruction text under `on-request`.**

- Where: nowhere in Aether today. The text is Codex's own. Of its 6,144
  characters, 3,878 are the rules for asking to leave the sandbox and 2,266
  list the command prefixes the operator has approved before.
- Whether the app-server accepts a setting that shortens or replaces the
  text while keeping `on-request` was not established. *Hypothesis.* If it
  does, the setting would be passed from `args()` in the same file, beside
  the two that switch the multi-agent feature off (lines 118 and 120).
- What it costs in function: nothing in what Aether answers, as long as the
  policy stays `on-request`. A model that is not told how to ask may ask
  less well, and under full access it has nothing to ask for. Leaving out
  the operator's approved prefixes by starting Codex without the operator's
  rules would change which commands are allowed, and is not proposed.
- How to test: an arm on a binary that passes the setting, against L-A0 and
  L-A3 in the same blocks, with the first-turn characters read from the
  session as here.

**3. Leaving it, and stating the cost.** *This is what the data supports
now.*

- The measured cost is about 1,485 input tokens in each request's prefix,
  read from the cache after the first request, and no difference in spend
  that 12 pairs can tell from a second run of the same cell.
- The possible cost is in accuracy, not in tokens, and is the hypothesis to
  test next: L-A0 against L-A3 alone, at least 12 blocks per case, graded
  per test, so that a hanging test costs its own checks and not all of
  them (`evals/harness_bench_cases.py`, `grade`). The numbers to read are
  the count of blocks on requests and on hidden checks, set against a
  second L-A0 in the same block as the yardstick.
- Until that is run, the function `on-request` buys is kept and nothing is
  known to be paid for it in spend.

**Not proposed:** switching the multi-agent feature back on, or changing
the envelope. Neither contrast is one-sided.

## Iteration 5

Iteration 4 left Luna undecided: the counted pairs show Aether dearer, the
sitting that looked for the cause showed nothing, and the same native cell
run twice in one block differed by as much as the pair did. Iteration 5 asks
one question with a larger sample: **does GPT-6 Luna through Aether differ
from GPT-6 Luna in Codex by more than Codex differs from itself?** It changes
nothing in Aether: no file under `crates/` is touched by its commits. Its
trials are diagnostic like those before them: all are reported here, none is
counted, and the comparison and report tools refuse their manifests.

This part of the section, down to and including
[The rule for a claim](#the-rule-for-a-claim), was written and committed
before the first trial started. What follows it was written afterwards.

### The arms of iteration 5

GPT-6 Luna, requested effort `low`, protocol version 2, the cases
`matchbook` and `gridcalc`:

| Arm | Condition | What it is |
| --- | --- | --- |
| L-N0 | `native-luna-low` | the counted native cell |
| L-N0b | `diag-native-luna-low-repeat` | the identical native command a second time in the same block. A test holds its command line equal to L-N0's (`test_the_repeat_arm_is_the_counted_native_cell_a_second_time`). This is the yardstick |
| L-A0 | `aether-single-luna-low` | the counted Aether cell, on the release build |

Two contrasts are read, per case and over both: **L-A0 minus L-N0**, the
question, and **L-N0b minus L-N0**, the yardstick. A pair is the two trials
of one block.

### The plan of iteration 5

- **12 blocks per case, 72 trials, seeds 31 to 42.** A block is one trial of
  each of the three arms on one case, started together as one invocation
  with `--jobs 3`, `--timeout 1200` and `--diagnostic`. Four blocks run side
  by side, two of each case, so 12 trials run at once: per case one stream
  takes seeds 31 to 36 and the other 37 to 42.
- **The number is fixed here and is not changed by what the results look
  like.** Only the clock of the sitting can shorten it: a stream starts no
  new block after 01:32 UTC on 2026-10-11, and what was not run is said.
- Before each block the available memory must be 4 GB or more and the home
  volume must have 15 GB or more free; both are recorded. A guard ends a
  process started in a trial's workspace once it holds more than 4 GB and
  logs it with the trial id.
- The grader is the counted one. From this iteration a trial also records a
  test-by-test grade beside it
  ([protocol](../evals/harness_bench_protocol.md#tasks-and-grading)): when
  the grader's one process times out, each hidden test is run again in a
  process of its own. The counted grade is unchanged.

### The measures and the arithmetic

Eight measures per trial: API requests (from the solver's Codex session
log), uncached input tokens, cache read tokens, output tokens,
API-equivalent USD, wall seconds, hidden checks as counted, and hidden
checks test by test (the passed count of the second field).

For each contrast and measure, per case and over both:

- the two means and medians; the mean and the median of the pair
  differences; the pairs in which the first arm is lower, higher and equal;
  the smallest and largest pair difference;
- a 95 % interval for the mean pair difference by a paired bootstrap:
  10,000 resamples of the pairs with replacement, the 2.5th and 97.5th
  percentiles of the resampled means, seed 20261011. Over both cases the
  pairs are resampled within each case, so every resample holds each case
  as often as the sample does;
- the two-sided sign-flip permutation p-value of the mean pair difference:
  exact over all sign assignments for up to 16 pairs, otherwise 100,000
  random assignments with the same seed.

A pair in which either trial did not report a measure is left out of that
measure and counted as missing. The intervals and p-values are
**descriptive**. Eight measures are looked at in two contrasts and three
groupings, so one small p-value among them is not a finding.

### The rule for a claim

An Aether effect on a measure is claimed only if all three hold:

1. the 95 % interval of L-A0 minus L-N0 over both cases excludes zero;
2. the 95 % interval of the yardstick, L-N0b minus L-N0, on that measure
   over both cases includes zero;
3. the mean pair difference of L-A0 minus L-N0 has the same sign in
   `matchbook` and in `gridcalc` taken separately.

Otherwise the measure is reported as **not distinguishable from native at
this sample size**, with the smallest and largest effect the interval still
allows. The rule is applied as worded, to every measure, whichever side it
favours.

### How iteration 5 was run

- **All 12 blocks per case ran: 24 blocks, 72 trials, seeds 31 to 42**, on
  2026-10-11 from 00:22:17 to 01:19:20 UTC. The clock never shortened the
  plan; the last block started at 01:04:34, before the 01:32 limit.
- **One build and one benchmark for every trial.** Every manifest records
  the commit that adds the test-by-test grade (`aceccc7b`) with a clean
  tree, the release binary with SHA-256
  `251c0b08f359c8a679a76831dea1d892b0c2a0a2c34e51427323c8b25d4d4145`, built
  before the first trial and never installed, and codex-cli 0.162.1. That
  commit changes no file under `crates/`. The case digests were checked before the first trial
  and are the frozen ones (`matchbook` `7f719f2d…d630a`, `gridcalc`
  `df35116f…d4758`).
- **The two native arms ran the same command.** In all 24 blocks the
  recorded command line of L-N0b is that of L-N0: `codex exec --json
  --dangerously-bypass-approvals-and-sandbox -c mcp_servers={} --model
  gpt-6-luna -c model_reasoning_effort="low"`. Every trial resolved
  `gpt-6-luna`, and the effort was confirmed in all 72.
- **Four streams**, two per case, each one block after the other, so up to
  12 trials ran at once; fewer after the first stream ended at 00:45:54.
- **Host.** Before every block the available memory and the free disk were
  read and recorded. One block waited: `matchbook` seed 32 found 2,782 MB
  available at 00:24:58, waited 30 seconds and started with 9,395 MB. The
  lowest reading at a block start was 5,124 MB; the disk had 39 to 40 GB
  free throughout.
- **The memory guard fired once**, at 00:57:33 UTC, on trial `13aaa3daa5`
  (`gridcalc`, seed 41, L-N0): the solver's own test run, `python -m
  unittest discover -s tests -v`, held 4.7 GB and was ended. The trial went
  on and completed with 57 requests and 136 of 154 hidden checks. Its pair
  is kept in every contrast; the guard changed what that one native solver
  saw.
- **A second guard was added four minutes into the sitting**, at 00:26:22
  UTC, after the grader's one process held 8.4 GB on a hung solution, which
  the memory guard leaves alone by design. It would have ended the largest
  process of a trial workspace had the available memory fallen under 700
  MB. It never fired. It is not in the plan above and is said here for that
  reason.
- **Two solvers reached the 1,200 second timeout**, one native and one
  through Aether; nine trials were graded 0. All are listed under
  [Trials graded 0, timeouts and hung grades](#trials-graded-0-timeouts-and-hung-grades).
  No trial failed to start, and no manifest records a harness error.

| Case | Block (seed) | Stream | Start (UTC) | End | Exit | Available memory at start (MB) | Free disk (GB) | 1-minute load at start |
| --- | ---: | --- | --- | --- | ---: | ---: | ---: | ---: |
| `matchbook` | 31 | s1 | 00:22:17 | 00:24:58 | 0 | 8,469 | 40 | 0.76 |
| `matchbook` | 32 | s1 | 00:25:28 | 00:29:50 | 0 | 9,395 | 40 | 3.25 |
| `matchbook` | 33 | s1 | 00:29:50 | 00:32:28 | 0 | 8,063 | 40 | 1.83 |
| `matchbook` | 34 | s1 | 00:32:28 | 00:34:55 | 0 | 8,389 | 40 | 2.61 |
| `matchbook` | 35 | s1 | 00:34:55 | 00:38:17 | 0 | 5,124 | 39 | 5.49 |
| `matchbook` | 36 | s1 | 00:38:17 | 00:58:19 | 0 | 9,971 | 39 | 2.84 |
| `matchbook` | 37 | s2 | 00:22:17 | 00:27:26 | 0 | 8,470 | 40 | 0.76 |
| `matchbook` | 38 | s2 | 00:27:26 | 00:32:23 | 0 | 8,929 | 40 | 1.56 |
| `matchbook` | 39 | s2 | 00:32:23 | 00:35:23 | 0 | 8,587 | 40 | 2.40 |
| `matchbook` | 40 | s2 | 00:35:23 | 00:38:45 | 0 | 9,546 | 39 | 4.80 |
| `matchbook` | 41 | s2 | 00:38:45 | 00:43:10 | 0 | 9,605 | 39 | 2.03 |
| `matchbook` | 42 | s2 | 00:43:10 | 00:45:54 | 0 | 9,289 | 39 | 1.68 |
| `gridcalc` | 31 | s1 | 00:22:17 | 00:32:04 | 0 | 8,470 | 40 | 0.76 |
| `gridcalc` | 32 | s1 | 00:32:04 | 00:35:37 | 0 | 8,020 | 40 | 2.22 |
| `gridcalc` | 33 | s1 | 00:35:37 | 00:43:12 | 0 | 9,252 | 39 | 4.22 |
| `gridcalc` | 34 | s1 | 00:43:12 | 00:50:04 | 0 | 8,991 | 39 | 1.68 |
| `gridcalc` | 35 | s1 | 00:50:04 | 00:59:14 | 0 | 9,172 | 39 | 0.51 |
| `gridcalc` | 36 | s1 | 00:59:14 | 01:19:20 | 0 | 8,926 | 39 | 1.44 |
| `gridcalc` | 37 | s2 | 00:22:17 | 00:28:12 | 0 | 8,470 | 40 | 0.76 |
| `gridcalc` | 38 | s2 | 00:28:12 | 00:40:20 | 0 | 8,887 | 40 | 0.90 |
| `gridcalc` | 39 | s2 | 00:40:21 | 00:52:58 | 0 | 9,510 | 39 | 0.98 |
| `gridcalc` | 40 | s2 | 00:52:58 | 00:56:08 | 0 | 8,896 | 39 | 0.89 |
| `gridcalc` | 41 | s2 | 00:56:08 | 01:04:34 | 0 | 8,865 | 39 | 0.95 |
| `gridcalc` | 42 | s2 | 01:04:34 | 01:09:13 | 0 | 8,818 | 39 | 1.27 |

### Every trial of iteration 5

Hidden checks are the counted grade. The column beside it is the second
field as the trial recorded it. Requests read `unknown` for the two trials
that reached the timeout: the count at the stop is a lower bound and is
left out. No solution passed every hidden check, so every manifest row
reads `failed`; the workflow column says whether the solver finished.

| Case | Block (seed) | Arm | Trial | Workflow | Hidden checks | Test by test: passed / failed / hung / not run | Requests | Uncached input | Cache read | Output | USD | Wall seconds |
| --- | ---: | --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `matchbook` | 31 | L-N0 | `1b8be3d4a0` | completed | 82/119 | 82 / 37 / 0 / 0 | 27 | 42,344 | 749,824 | 10,067 | 0.0168 | 85.2 |
| `matchbook` | 31 | L-N0b | `2227f36a30` | completed | 100/119 | 100 / 19 / 0 / 0 | 44 | 57,782 | 1,329,152 | 12,812 | 0.0255 | 119.2 |
| `matchbook` | 31 | L-A0 | `0bb184ae85` | completed | 100/119 | 100 / 19 / 0 / 0 | 13 | 24,413 | 308,992 | 7,677 | 0.0094 | 158.7 |
| `matchbook` | 32 | L-N0 | `175dfdddd7` | completed | 85/119 | 85 / 34 / 0 / 0 | 37 | 49,668 | 1,103,616 | 11,252 | 0.0216 | 260.2 |
| `matchbook` | 32 | L-N0b | `132c68ec38` | completed | 90/119 | 90 / 29 / 0 / 0 | 9 | 19,199 | 184,064 | 6,346 | 0.0069 | 56.2 |
| `matchbook` | 32 | L-A0 | `af37d309ea` | completed | 111/119 | 111 / 8 / 0 / 0 | 29 | 75,010 | 986,880 | 17,137 | 0.0259 | 128.5 |
| `matchbook` | 33 | L-N0 | `bc1958bb98` | completed | 111/119 | 111 / 8 / 0 / 0 | 23 | 35,320 | 626,944 | 12,970 | 0.0163 | 113.0 |
| `matchbook` | 33 | L-N0b | `462801eea1` | completed | 107/119 | 107 / 12 / 0 / 0 | 11 | 20,092 | 251,136 | 7,019 | 0.0080 | 49.3 |
| `matchbook` | 33 | L-A0 | `53017957b0` | completed | 117/119 | 117 / 2 / 0 / 0 | 46 | 78,144 | 1,726,976 | 16,795 | 0.0335 | 155.9 |
| `matchbook` | 34 | L-N0 | `d7eb9f9f79` | completed | 115/119 | 115 / 4 / 0 / 0 | 40 | 71,682 | 1,520,640 | 17,475 | 0.0311 | 145.6 |
| `matchbook` | 34 | L-N0b | `03a294ec98` | completed | 114/119 | 114 / 5 / 0 / 0 | 7 | 16,515 | 143,616 | 6,339 | 0.0063 | 134.6 |
| `matchbook` | 34 | L-A0 | `5b9c491b57` | completed | 103/119 | 103 / 16 / 0 / 0 | 14 | 30,778 | 347,648 | 7,860 | 0.0105 | 59.7 |
| `matchbook` | 35 | L-N0 | `ab6d4333e9` | completed | 104/119 | 104 / 15 / 0 / 0 | 23 | 64,619 | 631,040 | 9,342 | 0.0174 | 200.4 |
| `matchbook` | 35 | L-N0b | `d320a32bbc` | completed | 0/119 | 84 / 33 / 2 / 0 | 16 | 29,555 | 409,600 | 8,988 | 0.0115 | 64.4 |
| `matchbook` | 35 | L-A0 | `a1ac19ddae` | completed | 0/119 | 96 / 20 / 3 / 0 | 21 | 33,015 | 553,728 | 9,100 | 0.0134 | 73.1 |
| `matchbook` | 36 | L-N0 | `71f0172a87` | timeout | 96/119 | 96 / 23 / 0 / 0 | unknown | unknown | unknown | unknown | unknown | 1,200.0 |
| `matchbook` | 36 | L-N0b | `fbc34c6354` | completed | 105/119 | 105 / 14 / 0 / 0 | 20 | 32,654 | 488,448 | 9,989 | 0.0131 | 75.6 |
| `matchbook` | 36 | L-A0 | `82768fcdda` | completed | 104/119 | 104 / 15 / 0 / 0 | 16 | 27,756 | 405,504 | 8,769 | 0.0112 | 184.2 |
| `matchbook` | 37 | L-N0 | `7796e5c615` | completed | 113/119 | 113 / 6 / 0 / 0 | 29 | 42,646 | 791,296 | 14,951 | 0.0197 | 124.6 |
| `matchbook` | 37 | L-N0b | `dd5d7b09b8` | completed | 118/119 | 118 / 1 / 0 / 0 | 15 | 31,996 | 356,608 | 10,318 | 0.0119 | 85.5 |
| `matchbook` | 37 | L-A0 | `801395300a` | completed | 0/119 | 45 / 5 / 23 / 46 | 8 | 35,861 | 148,480 | 6,372 | 0.0083 | 127.9 |
| `matchbook` | 38 | L-N0 | `df77917b80` | completed | 0/119 | 107 / 8 / 4 / 0 | 31 | 58,275 | 940,288 | 19,046 | 0.0248 | 166.5 |
| `matchbook` | 38 | L-N0b | `4e95e7eff9` | completed | 113/119 | 113 / 6 / 0 / 0 | 24 | 39,321 | 645,120 | 9,206 | 0.0150 | 77.0 |
| `matchbook` | 38 | L-A0 | `dbbf2a4705` | completed | 109/119 | 109 / 10 / 0 / 0 | 32 | 47,751 | 913,408 | 14,318 | 0.0211 | 295.5 |
| `matchbook` | 39 | L-N0 | `0786e66f37` | completed | 0/119 | 89 / 28 / 2 / 0 | 28 | 63,129 | 798,720 | 11,207 | 0.0199 | 101.7 |
| `matchbook` | 39 | L-N0b | `e28344b4f9` | completed | 0/119 | 92 / 26 / 1 / 0 | 25 | 40,173 | 641,792 | 9,808 | 0.0153 | 77.3 |
| `matchbook` | 39 | L-A0 | `c11e7b97e6` | completed | 117/119 | 117 / 2 / 0 / 0 | 26 | 47,717 | 756,224 | 12,805 | 0.0187 | 98.7 |
| `matchbook` | 40 | L-N0 | `40ca7be92d` | completed | 118/119 | 118 / 1 / 0 / 0 | 22 | 35,542 | 595,456 | 12,649 | 0.0158 | 102.3 |
| `matchbook` | 40 | L-N0b | `99dd857c0f` | completed | 113/119 | 113 / 6 / 0 / 0 | 24 | 39,083 | 633,856 | 9,324 | 0.0149 | 200.0 |
| `matchbook` | 40 | L-A0 | `d48aa29619` | completed | 0/119 | 0 / 119 / 0 / 0 | 5 | 16,037 | 88,832 | 5,303 | 0.0051 | 104.2 |
| `matchbook` | 41 | L-N0 | `88130c237d` | completed | 112/119 | 112 / 7 / 0 / 0 | 16 | 29,959 | 423,936 | 9,451 | 0.0120 | 73.0 |
| `matchbook` | 41 | L-N0b | `6b547c8d06` | completed | 99/119 | 99 / 20 / 0 / 0 | 28 | 46,274 | 773,120 | 12,605 | 0.0187 | 264.0 |
| `matchbook` | 41 | L-A0 | `7f04512fdf` | completed | 110/119 | 110 / 9 / 0 / 0 | 14 | 32,540 | 353,792 | 8,852 | 0.0112 | 58.9 |
| `matchbook` | 42 | L-N0 | `68c5b6daf5` | completed | 118/119 | 118 / 1 / 0 / 0 | 10 | 20,472 | 225,792 | 8,091 | 0.0084 | 162.4 |
| `matchbook` | 42 | L-N0b | `ee034f7dd7` | completed | 104/119 | 104 / 15 / 0 / 0 | 28 | 41,345 | 786,432 | 10,583 | 0.0173 | 92.1 |
| `matchbook` | 42 | L-A0 | `afa034e19e` | completed | 0/119 | 102 / 11 / 6 / 0 | 12 | 22,183 | 271,360 | 5,994 | 0.0079 | 48.9 |
| `gridcalc` | 31 | L-N0 | `a1db754734` | completed | 138/154 | 138 / 16 / 0 / 0 | 39 | 52,922 | 1,213,696 | 15,093 | 0.0250 | 118.7 |
| `gridcalc` | 31 | L-N0b | `e80c009ac6` | completed | 135/154 | 135 / 19 / 0 / 0 | 60 | 71,984 | 2,221,056 | 17,898 | 0.0384 | 194.7 |
| `gridcalc` | 31 | L-A0 | `392603ada7` | completed | 134/154 | 134 / 20 / 0 / 0 | 58 | 87,897 | 2,670,080 | 27,866 | 0.0494 | 585.6 |
| `gridcalc` | 32 | L-N0 | `99b4ae5dbe` | completed | 125/154 | 125 / 29 / 0 / 0 | 27 | 38,759 | 768,256 | 13,410 | 0.0183 | 107.0 |
| `gridcalc` | 32 | L-N0b | `fdbeb5b53c` | completed | 126/154 | 126 / 28 / 0 / 0 | 56 | 72,240 | 2,174,976 | 24,632 | 0.0413 | 211.5 |
| `gridcalc` | 32 | L-A0 | `8e99c44eee` | completed | 130/154 | 130 / 24 / 0 / 0 | 74 | 102,621 | 3,054,080 | 23,861 | 0.0527 | 206.4 |
| `gridcalc` | 33 | L-N0 | `1ceab36456` | completed | 136/154 | 136 / 18 / 0 / 0 | 42 | 58,952 | 1,424,896 | 21,778 | 0.0310 | 453.8 |
| `gridcalc` | 33 | L-N0b | `9884f3df16` | completed | 132/154 | 132 / 22 / 0 / 0 | 116 | 139,116 | 5,881,856 | 27,501 | 0.0865 | 277.9 |
| `gridcalc` | 33 | L-A0 | `79dcd0fb59` | completed | 133/154 | 133 / 21 / 0 / 0 | 30 | 44,898 | 888,320 | 13,121 | 0.0199 | 113.0 |
| `gridcalc` | 34 | L-N0 | `540de05d20` | completed | 135/154 | 135 / 19 / 0 / 0 | 43 | 66,844 | 1,470,720 | 20,675 | 0.0317 | 169.7 |
| `gridcalc` | 34 | L-N0b | `612f6f7a39` | completed | 130/154 | 130 / 24 / 0 / 0 | 25 | 42,255 | 763,648 | 14,242 | 0.0190 | 115.4 |
| `gridcalc` | 34 | L-A0 | `4aca425bd1` | completed | 122/154 | 122 / 32 / 0 / 0 | 45 | 95,980 | 1,645,312 | 19,378 | 0.0357 | 410.1 |
| `gridcalc` | 35 | L-N0 | `725f6a7b16` | completed | 149/154 | 149 / 5 / 0 / 0 | 44 | 73,143 | 1,631,232 | 27,109 | 0.0372 | 549.0 |
| `gridcalc` | 35 | L-N0b | `1543fa36b7` | completed | 139/154 | 139 / 15 / 0 / 0 | 72 | 99,100 | 3,124,224 | 25,562 | 0.0539 | 236.0 |
| `gridcalc` | 35 | L-A0 | `8622a6e482` | completed | 120/154 | 120 / 34 / 0 / 0 | 39 | 54,273 | 1,320,192 | 17,338 | 0.0273 | 139.7 |
| `gridcalc` | 36 | L-N0 | `1f2d2f90a6` | completed | 119/154 | 119 / 35 / 0 / 0 | 33 | 46,439 | 984,832 | 15,264 | 0.0221 | 317.7 |
| `gridcalc` | 36 | L-N0b | `3a01647652` | completed | 0/154 | 0 / 154 / 0 / 0 | 30 | 59,683 | 846,336 | 13,926 | 0.0214 | 113.0 |
| `gridcalc` | 36 | L-A0 | `24c3631fac` | timeout | 133/154 | 133 / 21 / 0 / 0 | unknown | 373,246 | 28,222,464 | 60,748 | 0.3499 | 1,200.0 |
| `gridcalc` | 37 | L-N0 | `0458c6a42b` | completed | 126/154 | 126 / 28 / 0 / 0 | 49 | 62,393 | 1,678,080 | 19,760 | 0.0329 | 155.9 |
| `gridcalc` | 37 | L-N0b | `c5800f0553` | completed | 126/154 | 126 / 28 / 0 / 0 | 24 | 43,271 | 702,464 | 17,654 | 0.0202 | 354.1 |
| `gridcalc` | 37 | L-A0 | `fbbf76037a` | completed | 117/154 | 117 / 37 / 0 / 0 | 37 | 54,125 | 1,256,192 | 18,083 | 0.0270 | 140.8 |
| `gridcalc` | 38 | L-N0 | `8cc5132784` | completed | 101/154 | 101 / 53 / 0 / 0 | 121 | 132,419 | 5,834,496 | 30,363 | 0.0868 | 726.9 |
| `gridcalc` | 38 | L-N0b | `fc27d340ab` | completed | 141/154 | 141 / 13 / 0 / 0 | 38 | 51,395 | 1,166,848 | 15,818 | 0.0247 | 134.2 |
| `gridcalc` | 38 | L-A0 | `d470ed1634` | completed | 127/154 | 127 / 27 / 0 / 0 | 29 | 63,112 | 1,054,464 | 17,347 | 0.0255 | 126.6 |
| `gridcalc` | 39 | L-N0 | `c1bb856e8d` | completed | 139/154 | 139 / 15 / 0 / 0 | 40 | 51,365 | 1,315,840 | 16,540 | 0.0266 | 135.6 |
| `gridcalc` | 39 | L-N0b | `4be29bf2c1` | completed | 125/154 | 125 / 29 / 0 / 0 | 41 | 61,555 | 1,428,224 | 17,014 | 0.0289 | 355.5 |
| `gridcalc` | 39 | L-A0 | `6d7953d443` | completed | 137/154 | 137 / 17 / 0 / 0 | 74 | 102,561 | 3,148,288 | 36,764 | 0.0601 | 756.2 |
| `gridcalc` | 40 | L-N0 | `f2d7c974be` | completed | 130/154 | 130 / 24 / 0 / 0 | 38 | 67,388 | 1,537,536 | 19,649 | 0.0319 | 157.2 |
| `gridcalc` | 40 | L-N0b | `7867853baf` | completed | 138/154 | 138 / 16 / 0 / 0 | 21 | 39,230 | 538,368 | 12,158 | 0.0154 | 90.1 |
| `gridcalc` | 40 | L-A0 | `4ff3598d78` | completed | 97/154 | 97 / 57 / 0 / 0 | 53 | 86,425 | 2,210,560 | 23,256 | 0.0424 | 188.1 |
| `gridcalc` | 41 | L-N0 | `13aaa3daa5` | completed | 136/154 | 136 / 18 / 0 / 0 | 57 | 77,439 | 2,132,736 | 17,845 | 0.0380 | 193.6 |
| `gridcalc` | 41 | L-N0b | `fd23010cff` | completed | 135/154 | 135 / 19 / 0 / 0 | 76 | 92,549 | 3,365,888 | 22,367 | 0.0541 | 504.6 |
| `gridcalc` | 41 | L-A0 | `e9c7097451` | completed | 115/154 | 115 / 39 / 0 / 0 | 17 | 31,787 | 423,680 | 10,508 | 0.0127 | 83.4 |
| `gridcalc` | 42 | L-N0 | `4f020a3903` | completed | 134/154 | 134 / 20 / 0 / 0 | 44 | 55,024 | 1,443,840 | 16,684 | 0.0283 | 138.3 |
| `gridcalc` | 42 | L-N0b | `4c6f52ff6c` | completed | 134/154 | 134 / 20 / 0 / 0 | 55 | 87,767 | 2,218,240 | 21,652 | 0.0418 | 172.0 |
| `gridcalc` | 42 | L-A0 | `55bf6191a7` | completed | 132/154 | 132 / 22 / 0 / 0 | 88 | 105,426 | 4,154,368 | 30,889 | 0.0675 | 277.3 |

### Trials graded 0, timeouts and hung grades

Every figure in this table is as the trial recorded it.

| Case | Block (seed) | Arm | Trial | Workflow | Counted | Test by test as recorded: passed / failed / hung / not run | Hung tests | Note |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| `matchbook` | 35 | L-A0 | `a1ac19ddae` | completed | 0/119 | 96 / 20 / 3 / 0 | 3: `test_random_flow_replays_to_the_same_engine`, `test_book_is_never_crossed_and_levels_match_open_orders`, `test_random_histories_replay_exactly` | the grader's one process timed out after 60 seconds |
| `matchbook` | 35 | L-N0b | `d320a32bbc` | completed | 0/119 | 84 / 33 / 2 / 0 | 2: `test_random_flow_replays_to_the_same_engine`, `test_tranches_stay_consistent_under_random_flow` | the grader's one process timed out after 60 seconds |
| `matchbook` | 36 | L-N0 | `71f0172a87` | timeout | 96/119 | 96 / 23 / 0 / 0 | none | the solver reached the timeout at 1,200 seconds with 604 requests made; its tokens and cost are unknown; its workspace was graded as it stood |
| `matchbook` | 37 | L-A0 | `801395300a` | completed | 0/119 | 45 / 5 / 23 / 46 | 23: `test_amend_to_a_crossing_price_trades_as_the_taker`, `test_amended_remainder_rests_at_the_new_price_after_trading`, `test_amend_to_a_crossing_price_trades_the_whole_remaining` and others | the grader's one process timed out after 60 seconds; the test-by-test grade reached its total limit with 46 tests not run |
| `matchbook` | 38 | L-N0 | `df77917b80` | completed | 0/119 | 107 / 8 / 4 / 0 | 4: `test_random_flow_replays_to_the_same_engine`, `test_book_is_never_crossed_and_levels_match_open_orders`, `test_filled_quantities_equal_the_trade_tape` and others | the grader's one process timed out after 60 seconds |
| `matchbook` | 39 | L-N0 | `0786e66f37` | completed | 0/119 | 89 / 28 / 2 / 0 | 2: `test_random_flow_replays_to_the_same_engine`, `test_tranches_stay_consistent_under_random_flow` | the grader's one process timed out after 60 seconds |
| `matchbook` | 39 | L-N0b | `e28344b4f9` | completed | 0/119 | 92 / 26 / 1 / 0 | 1: `test_replay_restores_tranches_and_queue_places` | the grader's one process timed out after 60 seconds |
| `matchbook` | 40 | L-A0 | `d48aa29619` | completed | 0/119 | 0 / 119 / 0 / 0 | none | every hidden check fails (AttributeError) |
| `matchbook` | 42 | L-A0 | `afa034e19e` | completed | 0/119 | 102 / 11 / 6 / 0 | 6: `test_random_flow_replays_to_the_same_engine`, `test_tranches_stay_consistent_under_random_flow`, `test_replay_restores_tranches_and_queue_places` and others | the grader's one process timed out after 60 seconds |
| `gridcalc` | 36 | L-A0 | `24c3631fac` | timeout | 133/154 | 133 / 21 / 0 / 0 | none | the solver reached the timeout at 1,200 seconds with 324 requests made; its tokens and cost up to the stop are recorded and are used; its workspace was graded as it stood; no terminal event within 1200s |
| `gridcalc` | 36 | L-N0b | `3a01647652` | completed | 0/154 | 0 / 154 / 0 / 0 | none | the solution does not import (IndentationError) |

- **The grader's one process timed out in 7 of 72 trials**, all on
  `matchbook`: 3 of L-A0, 2 of L-N0 and 2 of L-N0b. Each is counted as 0.
  Run one test per process, six of those seven solutions pass 84 to 107 of
  119. The seventh recorded 45 passed with 46 tests not run.
- **Two more trials are at 0 without a hang**: one L-A0 solution on
  `matchbook` fails every check, and one L-N0b solution on `gridcalc` does
  not import. Their test-by-test count is 0 as well.
- **Trials at 0 per arm: 4 of 24 for L-A0, 2 of 24 for L-N0, 3 of 24 for
  L-N0b.**
- **One test-by-test grade was cut by its total limit, and its recorded
  count is the one used.** Trial `801395300a` (`matchbook`, seed 37, L-A0)
  recorded 45 passed, 5 failed and 23 hung, and the 120 second total ran
  out with 46 tests not run. The plan's measure is "the passed count of the
  second field", so every table and figure of this iteration uses 45. It is
  a lower bound on what that solution passes. What the grade reads when
  finished is under
  [The cut grade, as planned and finished](#the-cut-grade-as-planned-and-finished).
- **The two timeouts are not recorded alike.** Codex reports tokens when a
  turn ends, so the native trial that was stopped (`71f0172a87`, 604
  requests made) has no tokens and no cost, and its pairs are left out of
  those measures. The Aether trial that was stopped (`24c3631fac`, 324
  requests made) has them from Aether's usage events up to the stop:
  373,246 uncached input tokens and 0.3499 USD, the dearest trial of the
  sitting. By the sentence written before the first trial it reported the
  measure and stays in. So the dearest native trial is missing from the
  token means and the dearest Aether trial is in them. The contrasts are
  given a second time without the two blocks under
  [Without the two blocks with a timeout](#without-the-two-blocks-with-a-timeout).
  Wall seconds (1,200 for both) and the grade of the workspace as it stood
  are used for both.
- **The counted grade is the one from before.** In the 65 trials whose
  grader did not time out, the second field equals the counted grade. For
  10 trials the grader of the commit before this change was run on the kept
  workspace and gave the recorded counted grade in 10 of 10.

#### The cut grade, as planned and finished

After the sitting the test-by-test grade of trial `801395300a` was run
again on the kept workspace, with the same 5 seconds per test and no total
limit: 75 passed, 6 failed, 38 hung. That finished grade is not the planned
measure and is used in no other table. Every row that holds the trial, with
the recorded count and with the finished one:

| L-A0 minus L-N0, hidden checks test by test | Count of `801395300a` | Pairs | Mean L-A0 | Median L-A0 | Mean pair difference | 95 % interval | Lower / higher / equal | Sign-flip p | Interval in % of the L-N0 mean |
| --- | --- | ---: | ---: | ---: | ---: | --- | --- | ---: | --- |
| `matchbook` | as planned: the recorded 45 | 12 | 92.8 | 103.5 | -11.3 | -36.7 to +8.2 | 6 / 6 / 0 | 0.444 | -35.2 % to +7.8 % |
| `matchbook` | finished after the sitting: 75 | 12 | 95.3 | 103.5 | -8.8 | -32.7 to +8.7 | 6 / 6 / 0 | 0.582 | -31.4 % to +8.3 % |
| both cases | as planned: the recorded 45 | 24 | 108.8 | 116.0 | -8.6 | -22.0 to +2.3 | 15 / 9 / 0 | 0.208 | -18.8 % to +2.0 % |
| both cases | finished after the sitting: 75 | 24 | 110.0 | 116.0 | -7.4 | -20.3 to +2.8 | 15 / 9 / 0 | 0.253 | -17.3 % to +2.4 % |
| both cases, without the 2 blocks with a timeout | as planned: the recorded 45 | 22 | 107.9 | 116.0 | -10.4 | -25.0 to +1.5 | 15 / 7 / 0 | 0.149 | -21.1 % to +1.3 % |
| both cases, without the 2 blocks with a timeout | finished after the sitting: 75 | 22 | 109.3 | 116.0 | -9.0 | -23.0 to +1.9 | 15 / 7 / 0 | 0.175 | -19.4 % to +1.6 % |
| `matchbook`, with iteration 4 | as planned: the recorded 45 | 18 | 96.1 | 103.5 | -9.2 | -26.4 to +4.2 | 10 / 8 / 0 | 0.330 | -25.1 % to +4.0 % |
| `matchbook`, with iteration 4 | finished after the sitting: 75 | 18 | 97.7 | 103.5 | -7.6 | -23.7 to +4.7 | 10 / 8 / 0 | 0.407 | -22.5 % to +4.4 % |
| both cases, with iteration 4 | as planned: the recorded 45 | 36 | 109.6 | 114.0 | -9.4 | -18.3 to -1.4 | 25 / 11 / 0 | 0.030 | -15.4 % to -1.2 % |
| both cases, with iteration 4 | finished after the sitting: 75 | 36 | 110.4 | 114.0 | -8.6 | -17.1 to -1.1 | 25 / 11 / 0 | 0.035 | -14.4 % to -0.9 % |

- **The finished grade moves every one of these differences towards zero,
  which is in Aether's favour**: over both cases by 1.2 checks in the mean,
  from -8.6 to -7.4.
- **The verdict under the rule is the same with either count.** Over both
  cases the interval includes zero with the recorded count (-22.0 to +2.3)
  and with the finished one (-20.3 to +2.8), so condition 1 fails and
  hidden checks test by test are not distinguishable from native. The rule
  is applied to the recorded count.
- **Over both cases with iteration 4, the interval excludes zero with
  either count**, below native. Those rows are not a planned sample and the
  rule is not applied to them.
- **This document first stated the finished figure as the result.** That
  departed from the plan in Aether's favour and was corrected on
  2026-10-11; no trial was run for the correction.

### One line per arm, iteration 5

A figure in square brackets is the number of trials that reported the
measure, where it is not all of them.

| Case | Arm | Trials | API requests: mean (median) | Uncached input tokens: mean (median) | Cache read tokens: mean (median) | Output tokens: mean (median) | API-equivalent USD: mean (median) | Wall seconds: mean (median) | Hidden checks, as counted: mean (median) | Hidden checks, test by test: mean (median) | Uncached input tokens per request: mean (median) | First-request input tokens: mean (median) |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `matchbook` | L-N0 | 12 | 26.0 (27.0) [11] | 46,696.0 (42,646.0) [11] | 764,322.9 (749,824.0) [11] | 12,409.2 (11,252.0) [11] | 0.0185 (0.0174) [11] | 227.9 (135.1) | 87.8 (107.5) | 104.2 (109.0) | 1,835.3 (1,792.0) [11] | 15,624.2 (15,624.0) |
| `matchbook` | L-N0b | 12 | 20.9 (22.0) | 34,499.1 (35,868.5) | 553,578.7 (561,152.0) | 9,444.8 (9,566.0) | 0.0137 (0.0140) | 107.9 (81.4) | 88.6 (104.5) | 103.2 (104.5) | 1,770.7 (1,645.5) | 15,623.5 (15,624.0) |
| `matchbook` | L-A0 | 12 | 19.7 (15.0) | 39,267.1 (32,777.5) | 571,818.7 (379,648.0) | 10,081.8 (8,810.5) | 0.0147 (0.0112) | 124.5 (116.1) | 72.6 (103.5) | 92.8 (103.5) | 2,238.2 (1,863.3) | 16,547.5 (16,547.5) |
| `gridcalc` | L-N0 | 12 | 48.1 (42.5) | 65,257.2 (60,672.5) | 1,786,346.7 (1,457,280.0) | 19,514.2 (18,747.0) | 0.0341 (0.0314) | 268.6 (163.5) | 130.7 (134.5) | 130.7 (134.5) | 1,404.5 (1,381.1) | 15,599.0 (15,599.0) |
| `gridcalc` | L-N0b | 12 | 51.2 (48.0) | 71,678.8 (66,769.5) | 2,036,010.7 (1,801,600.0) | 19,202.0 (17,776.0) | 0.0371 (0.0337) | 229.9 (203.1) | 121.8 (133.0) | 121.8 (133.0) | 1,507.0 (1,438.9) | 15,598.5 (15,598.0) |
| `gridcalc` | L-A0 | 12 | 49.5 (45.0) [11] | 100,195.9 (87,161.0) | 4,170,666.7 (1,927,936.0) | 24,929.9 (21,317.0) | 0.0642 (0.0391) | 352.3 (197.2) | 124.8 (128.5) | 124.8 (128.5) | 1,604.3 (1,496.6) [11] | 16,521.5 (16,521.0) |
| both | L-N0 | 24 | 37.5 (37.0) [23] | 56,380.1 (55,024.0) [23] | 1,297,552.7 (1,103,616.0) [23] | 16,116.1 (15,264.0) [23] | 0.0267 (0.0248) [23] | 248.3 (156.5) | 109.2 (118.0) | 117.4 (118.0) | 1,610.5 (1,535.7) [23] | 15,611.6 (15,612.5) |
| both | L-N0b | 24 | 36.0 (26.5) | 53,088.9 (42,763.0) | 1,294,794.7 (768,384.0) | 14,323.4 (12,708.5) | 0.0254 (0.0188) | 168.9 (126.7) | 105.2 (116.0) | 112.5 (116.0) | 1,638.8 (1,630.6) | 15,611.0 (15,611.5) |
| both | L-A0 | 24 | 33.9 (29.0) [23] | 69,731.5 (50,938.0) | 2,371,242.7 (950,144.0) | 17,505.9 (15,556.5) | 0.0394 (0.0233) | 238.4 (140.3) | 98.7 (116.0) | 108.8 (116.0) | 1,935.0 (1,734.8) [23] | 16,534.5 (16,533.5) |

### Per arm, every trial, iteration 5

In `matchbook`, the 45 of L-A0 in block 37 under hidden checks test by test
is the recorded count of the grade that was cut, a lower bound.

**API requests, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 27 | 37 | 23 | 40 | 23 | unknown | 29 | 31 | 28 | 22 | 16 | 10 | 26.0 (11) | 27.0 |
| L-N0b | 44 | 9 | 11 | 7 | 16 | 20 | 15 | 24 | 25 | 24 | 28 | 28 | 20.9 (12) | 22.0 |
| L-A0 | 13 | 29 | 46 | 14 | 21 | 16 | 8 | 32 | 26 | 5 | 14 | 12 | 19.7 (12) | 15.0 |

**Uncached input tokens, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 42,344 | 49,668 | 35,320 | 71,682 | 64,619 | unknown | 42,646 | 58,275 | 63,129 | 35,542 | 29,959 | 20,472 | 46,696.0 (11) | 42,646.0 |
| L-N0b | 57,782 | 19,199 | 20,092 | 16,515 | 29,555 | 32,654 | 31,996 | 39,321 | 40,173 | 39,083 | 46,274 | 41,345 | 34,499.1 (12) | 35,868.5 |
| L-A0 | 24,413 | 75,010 | 78,144 | 30,778 | 33,015 | 27,756 | 35,861 | 47,751 | 47,717 | 16,037 | 32,540 | 22,183 | 39,267.1 (12) | 32,777.5 |

**Cache read tokens, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 749,824 | 1,103,616 | 626,944 | 1,520,640 | 631,040 | unknown | 791,296 | 940,288 | 798,720 | 595,456 | 423,936 | 225,792 | 764,322.9 (11) | 749,824.0 |
| L-N0b | 1,329,152 | 184,064 | 251,136 | 143,616 | 409,600 | 488,448 | 356,608 | 645,120 | 641,792 | 633,856 | 773,120 | 786,432 | 553,578.7 (12) | 561,152.0 |
| L-A0 | 308,992 | 986,880 | 1,726,976 | 347,648 | 553,728 | 405,504 | 148,480 | 913,408 | 756,224 | 88,832 | 353,792 | 271,360 | 571,818.7 (12) | 379,648.0 |

**Output tokens, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 10,067 | 11,252 | 12,970 | 17,475 | 9,342 | unknown | 14,951 | 19,046 | 11,207 | 12,649 | 9,451 | 8,091 | 12,409.2 (11) | 11,252.0 |
| L-N0b | 12,812 | 6,346 | 7,019 | 6,339 | 8,988 | 9,989 | 10,318 | 9,206 | 9,808 | 9,324 | 12,605 | 10,583 | 9,444.8 (12) | 9,566.0 |
| L-A0 | 7,677 | 17,137 | 16,795 | 7,860 | 9,100 | 8,769 | 6,372 | 14,318 | 12,805 | 5,303 | 8,852 | 5,994 | 10,081.8 (12) | 8,810.5 |

**API-equivalent USD, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0.0168 | 0.0216 | 0.0163 | 0.0311 | 0.0174 | unknown | 0.0197 | 0.0248 | 0.0199 | 0.0158 | 0.0120 | 0.0084 | 0.0185 (11) | 0.0174 |
| L-N0b | 0.0255 | 0.0069 | 0.0080 | 0.0063 | 0.0115 | 0.0131 | 0.0119 | 0.0150 | 0.0153 | 0.0149 | 0.0187 | 0.0173 | 0.0137 (12) | 0.0140 |
| L-A0 | 0.0094 | 0.0259 | 0.0335 | 0.0105 | 0.0134 | 0.0112 | 0.0083 | 0.0211 | 0.0187 | 0.0051 | 0.0112 | 0.0079 | 0.0147 (12) | 0.0112 |

**Wall seconds, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 85.2 | 260.2 | 113.0 | 145.6 | 200.4 | 1,200.0 | 124.6 | 166.5 | 101.7 | 102.3 | 73.0 | 162.4 | 227.9 (12) | 135.1 |
| L-N0b | 119.2 | 56.2 | 49.3 | 134.6 | 64.4 | 75.6 | 85.5 | 77.0 | 77.3 | 200.0 | 264.0 | 92.1 | 107.9 (12) | 81.4 |
| L-A0 | 158.7 | 128.5 | 155.9 | 59.7 | 73.1 | 184.2 | 127.9 | 295.5 | 98.7 | 104.2 | 58.9 | 48.9 | 124.5 (12) | 116.1 |

**Hidden checks, as counted, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 82 | 85 | 111 | 115 | 104 | 96 | 113 | 0 | 0 | 118 | 112 | 118 | 87.8 (12) | 107.5 |
| L-N0b | 100 | 90 | 107 | 114 | 0 | 105 | 118 | 113 | 0 | 113 | 99 | 104 | 88.6 (12) | 104.5 |
| L-A0 | 100 | 111 | 117 | 103 | 0 | 104 | 0 | 109 | 117 | 0 | 110 | 0 | 72.6 (12) | 103.5 |

**Hidden checks, test by test, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 82 | 85 | 111 | 115 | 104 | 96 | 113 | 107 | 89 | 118 | 112 | 118 | 104.2 (12) | 109.0 |
| L-N0b | 100 | 90 | 107 | 114 | 84 | 105 | 118 | 113 | 92 | 113 | 99 | 104 | 103.2 (12) | 104.5 |
| L-A0 | 100 | 111 | 117 | 103 | 96 | 104 | 45 | 109 | 117 | 0 | 110 | 102 | 92.8 (12) | 103.5 |

**Uncached input tokens per request, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 1,568 | 1,342 | 1,536 | 1,792 | 2,810 | unknown | 1,471 | 1,880 | 2,255 | 1,616 | 1,872 | 2,047 | 1,835.3 (11) | 1,792.0 |
| L-N0b | 1,313 | 2,133 | 1,827 | 2,359 | 1,847 | 1,633 | 2,133 | 1,638 | 1,607 | 1,628 | 1,653 | 1,477 | 1,770.7 (12) | 1,645.5 |
| L-A0 | 1,878 | 2,587 | 1,699 | 2,198 | 1,572 | 1,735 | 4,483 | 1,492 | 1,835 | 3,207 | 2,324 | 1,849 | 2,238.2 (12) | 1,863.3 |

**First-request input tokens, `matchbook`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 15,630 | 15,620 | 15,622 | 15,626 | 15,626 | 15,624 | 15,624 | 15,622 | 15,624 | 15,624 | 15,622 | 15,626 | 15,624.2 (12) | 15,624.0 |
| L-N0b | 15,624 | 15,622 | 15,622 | 15,622 | 15,620 | 15,622 | 15,626 | 15,626 | 15,624 | 15,624 | 15,626 | 15,624 | 15,623.5 (12) | 15,624.0 |
| L-A0 | 16,547 | 16,545 | 16,548 | 16,553 | 16,549 | 16,546 | 16,540 | 16,550 | 16,555 | 16,542 | 16,548 | 16,547 | 16,547.5 (12) | 16,547.5 |

**API requests, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 39 | 27 | 42 | 43 | 44 | 33 | 49 | 121 | 40 | 38 | 57 | 44 | 48.1 (12) | 42.5 |
| L-N0b | 60 | 56 | 116 | 25 | 72 | 30 | 24 | 38 | 41 | 21 | 76 | 55 | 51.2 (12) | 48.0 |
| L-A0 | 58 | 74 | 30 | 45 | 39 | unknown | 37 | 29 | 74 | 53 | 17 | 88 | 49.5 (11) | 45.0 |

**Uncached input tokens, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 52,922 | 38,759 | 58,952 | 66,844 | 73,143 | 46,439 | 62,393 | 132,419 | 51,365 | 67,388 | 77,439 | 55,024 | 65,257.2 (12) | 60,672.5 |
| L-N0b | 71,984 | 72,240 | 139,116 | 42,255 | 99,100 | 59,683 | 43,271 | 51,395 | 61,555 | 39,230 | 92,549 | 87,767 | 71,678.8 (12) | 66,769.5 |
| L-A0 | 87,897 | 102,621 | 44,898 | 95,980 | 54,273 | 373,246 | 54,125 | 63,112 | 102,561 | 86,425 | 31,787 | 105,426 | 100,195.9 (12) | 87,161.0 |

**Cache read tokens, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 1,213,696 | 768,256 | 1,424,896 | 1,470,720 | 1,631,232 | 984,832 | 1,678,080 | 5,834,496 | 1,315,840 | 1,537,536 | 2,132,736 | 1,443,840 | 1,786,346.7 (12) | 1,457,280.0 |
| L-N0b | 2,221,056 | 2,174,976 | 5,881,856 | 763,648 | 3,124,224 | 846,336 | 702,464 | 1,166,848 | 1,428,224 | 538,368 | 3,365,888 | 2,218,240 | 2,036,010.7 (12) | 1,801,600.0 |
| L-A0 | 2,670,080 | 3,054,080 | 888,320 | 1,645,312 | 1,320,192 | 28,222,464 | 1,256,192 | 1,054,464 | 3,148,288 | 2,210,560 | 423,680 | 4,154,368 | 4,170,666.7 (12) | 1,927,936.0 |

**Output tokens, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 15,093 | 13,410 | 21,778 | 20,675 | 27,109 | 15,264 | 19,760 | 30,363 | 16,540 | 19,649 | 17,845 | 16,684 | 19,514.2 (12) | 18,747.0 |
| L-N0b | 17,898 | 24,632 | 27,501 | 14,242 | 25,562 | 13,926 | 17,654 | 15,818 | 17,014 | 12,158 | 22,367 | 21,652 | 19,202.0 (12) | 17,776.0 |
| L-A0 | 27,866 | 23,861 | 13,121 | 19,378 | 17,338 | 60,748 | 18,083 | 17,347 | 36,764 | 23,256 | 10,508 | 30,889 | 24,929.9 (12) | 21,317.0 |

**API-equivalent USD, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 0.0250 | 0.0183 | 0.0310 | 0.0317 | 0.0372 | 0.0221 | 0.0329 | 0.0868 | 0.0266 | 0.0319 | 0.0380 | 0.0283 | 0.0341 (12) | 0.0314 |
| L-N0b | 0.0384 | 0.0413 | 0.0865 | 0.0190 | 0.0539 | 0.0214 | 0.0202 | 0.0247 | 0.0289 | 0.0154 | 0.0541 | 0.0418 | 0.0371 (12) | 0.0337 |
| L-A0 | 0.0494 | 0.0527 | 0.0199 | 0.0357 | 0.0273 | 0.3499 | 0.0270 | 0.0255 | 0.0601 | 0.0424 | 0.0127 | 0.0675 | 0.0642 (12) | 0.0391 |

**Wall seconds, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 118.7 | 107.0 | 453.8 | 169.7 | 549.0 | 317.7 | 155.9 | 726.9 | 135.6 | 157.2 | 193.6 | 138.3 | 268.6 (12) | 163.5 |
| L-N0b | 194.7 | 211.5 | 277.9 | 115.4 | 236.0 | 113.0 | 354.1 | 134.2 | 355.5 | 90.1 | 504.6 | 172.0 | 229.9 (12) | 203.1 |
| L-A0 | 585.6 | 206.4 | 113.0 | 410.1 | 139.7 | 1,200.0 | 140.8 | 126.6 | 756.2 | 188.1 | 83.4 | 277.3 | 352.3 (12) | 197.2 |

**Hidden checks, as counted, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 138 | 125 | 136 | 135 | 149 | 119 | 126 | 101 | 139 | 130 | 136 | 134 | 130.7 (12) | 134.5 |
| L-N0b | 135 | 126 | 132 | 130 | 139 | 0 | 126 | 141 | 125 | 138 | 135 | 134 | 121.8 (12) | 133.0 |
| L-A0 | 134 | 130 | 133 | 122 | 120 | 133 | 117 | 127 | 137 | 97 | 115 | 132 | 124.8 (12) | 128.5 |

**Hidden checks, test by test, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 138 | 125 | 136 | 135 | 149 | 119 | 126 | 101 | 139 | 130 | 136 | 134 | 130.7 (12) | 134.5 |
| L-N0b | 135 | 126 | 132 | 130 | 139 | 0 | 126 | 141 | 125 | 138 | 135 | 134 | 121.8 (12) | 133.0 |
| L-A0 | 134 | 130 | 133 | 122 | 120 | 133 | 117 | 127 | 137 | 97 | 115 | 132 | 124.8 (12) | 128.5 |

**Uncached input tokens per request, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 1,357 | 1,436 | 1,404 | 1,555 | 1,662 | 1,407 | 1,273 | 1,094 | 1,284 | 1,773 | 1,359 | 1,251 | 1,404.5 (12) | 1,381.1 |
| L-N0b | 1,200 | 1,290 | 1,199 | 1,690 | 1,376 | 1,989 | 1,803 | 1,352 | 1,501 | 1,868 | 1,218 | 1,596 | 1,507.0 (12) | 1,438.9 |
| L-A0 | 1,515 | 1,387 | 1,497 | 2,133 | 1,392 | unknown | 1,463 | 2,176 | 1,386 | 1,631 | 1,870 | 1,198 | 1,604.3 (11) | 1,496.6 |

**First-request input tokens, `gridcalc`**

| Arm | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | Mean (n) | Median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L-N0 | 15,595 | 15,601 | 15,597 | 15,597 | 15,601 | 15,605 | 15,601 | 15,597 | 15,599 | 15,599 | 15,597 | 15,599 | 15,599.0 (12) | 15,599.0 |
| L-N0b | 15,597 | 15,599 | 15,599 | 15,601 | 15,599 | 15,597 | 15,597 | 15,597 | 15,601 | 15,597 | 15,595 | 15,603 | 15,598.5 (12) | 15,598.0 |
| L-A0 | 16,513 | 16,526 | 16,527 | 16,520 | 16,526 | 16,521 | 16,520 | 16,516 | 16,527 | 16,521 | 16,517 | 16,524 | 16,521.5 (12) | 16,521.0 |

### The two contrasts

Twelve pairs per case and 24 over both, fewer where a trial did not report
the measure. The interval and the p-value are as described under
[The measures and the arithmetic](#the-measures-and-the-arithmetic).

**L-A0 minus L-N0 (the question), `matchbook`**

| Measure | Pairs | Mean L-A0 | Mean L-N0 | Median L-A0 | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 11 | 20.0 | 26.0 | 14.0 | 27.0 | -6.0 | -13.2 to +1.9 | -2.0 | 8 / 3 / 0 | -26.0 | +23.0 | 0.175 |
| Uncached input tokens | 11 | 40,313.5 | 46,696.0 | 33,015.0 | 42,646.0 | -6,382.5 | -19,240.3 to +7,705.3 | -10,524.0 | 7 / 4 / 0 | -40,904.0 | +42,824.0 | 0.395 |
| Cache read tokens | 11 | 586,938.2 | 764,322.9 | 353,792.0 | 749,824.0 | -177,384.7 | -474,042.2 to +150,062.5 | -77,312.0 | 9 / 2 / 0 | -1,172,992.0 | +1,100,032.0 | 0.293 |
| Output tokens | 11 | 10,201.2 | 12,409.2 | 8,852.0 | 11,252.0 | -2,208.0 | -4,998.1 to +663.4 | -2,097.0 | 8 / 3 / 0 | -9,615.0 | +5,885.0 | 0.176 |
| API-equivalent USD | 11 | 0.0150 | 0.0185 | 0.0112 | 0.0174 | -0.0035 | -0.0088 to +0.0021 | -0.0037 | 9 / 2 / 0 | -0.0206 | +0.0172 | 0.261 |
| Wall seconds | 12 | 124.5 | 227.9 | 116.1 | 135.1 | -103.4 | -287.4 to +14.6 | -8.5 | 7 / 5 / 0 | -1,015.9 | +129.0 | 0.216 |
| Hidden checks, as counted | 12 | 72.6 | 87.8 | 103.5 | 107.5 | -15.2 | -61.0 to +29.7 | +2.0 | 6 / 6 / 0 | -118.0 | +117.0 | 0.624 |
| Hidden checks, test by test | 12 | 92.8 | 104.2 | 103.5 | 109.0 | -11.3 | -36.7 to +8.2 | +0.0 | 6 / 6 / 0 | -118.0 | +28.0 | 0.444 |

**L-A0 minus L-N0 (the question), `gridcalc`**

| Measure | Pairs | Mean L-A0 | Mean L-N0 | Median L-A0 | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 11 | 49.5 | 49.5 | 45.0 | 43.0 | +0.0 | -23.8 to +21.4 | +2.0 | 5 / 6 / 0 | -92.0 | +47.0 | 1.000 |
| Uncached input tokens | 12 | 100,195.9 | 65,257.2 | 87,161.0 | 60,672.5 | +34,938.7 | -8,909.8 to +96,906.0 | +24,086.5 | 5 / 7 / 0 | -69,307.0 | +326,807.0 | 0.260 |
| Cache read tokens | 12 | 4,170,666.7 | 1,786,346.7 | 1,927,936.0 | 1,457,280.0 | +2,384,320.0 | -748,096.0 to +7,320,213.3 | +423,808.0 | 5 / 7 / 0 | -4,780,032.0 | +27,237,632.0 | 0.433 |
| Output tokens | 12 | 24,929.9 | 19,514.2 | 21,317.0 | 18,747.0 | +5,415.8 | -2,587.5 to +15,049.3 | +1,155.0 | 6 / 6 / 0 | -13,016.0 | +45,484.0 | 0.300 |
| API-equivalent USD | 12 | 0.0642 | 0.0341 | 0.0391 | 0.0314 | +0.0300 | -0.0093 to +0.0902 | +0.0072 | 5 / 7 / 0 | -0.0612 | +0.3278 | 0.375 |
| Wall seconds | 12 | 352.3 | 268.6 | 197.2 | 163.5 | +83.6 | -141.0 to +325.4 | +65.1 | 5 / 7 / 0 | -600.3 | +882.3 | 0.513 |
| Hidden checks, as counted | 12 | 124.8 | 130.7 | 128.5 | 134.5 | -5.9 | -15.0 to +2.9 | -3.5 | 9 / 3 / 0 | -33.0 | +26.0 | 0.255 |
| Hidden checks, test by test | 12 | 124.8 | 130.7 | 128.5 | 134.5 | -5.9 | -15.0 to +2.9 | -3.5 | 9 / 3 / 0 | -33.0 | +26.0 | 0.255 |

**L-A0 minus L-N0 (the question), both cases**

| Measure | Pairs | Mean L-A0 | Mean L-N0 | Median L-A0 | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 22 | 34.7 | 37.7 | 29.5 | 37.5 | -3.0 | -15.7 to +8.4 | -2.0 | 13 / 9 / 0 | -92.0 | +47.0 | 0.664 |
| Uncached input tokens | 23 | 71,556.5 | 56,380.1 | 54,125.0 | 55,024.0 | +15,176.4 | -9,095.8 to +48,425.0 | -6,785.0 | 12 / 11 / 0 | -69,307.0 | +326,807.0 | 0.450 |
| Cache read tokens | 23 | 2,456,709.6 | 1,297,552.7 | 986,880.0 | 1,103,616.0 | +1,159,156.9 | -488,503.7 to +3,758,113.4 | -70,144.0 | 14 / 9 / 0 | -4,780,032.0 | +27,237,632.0 | 0.533 |
| Output tokens | 23 | 17,885.7 | 16,116.1 | 16,795.0 | 15,264.0 | +1,769.6 | -2,779.6 to +6,922.6 | -1,297.0 | 14 / 9 / 0 | -13,016.0 | +45,484.0 | 0.562 |
| API-equivalent USD | 23 | 0.0407 | 0.0267 | 0.0255 | 0.0248 | +0.0140 | -0.0069 to +0.0458 | -0.0012 | 14 / 9 / 0 | -0.0612 | +0.3278 | 0.525 |
| Wall seconds | 24 | 238.4 | 248.3 | 140.3 | 156.5 | -9.9 | -151.1 to +128.3 | -0.5 | 12 / 12 / 0 | -1,015.9 | +882.3 | 0.899 |
| Hidden checks, as counted | 24 | 98.7 | 109.2 | 116.0 | 118.0 | -10.6 | -33.6 to +12.3 | -2.5 | 15 / 9 / 0 | -118.0 | +117.0 | 0.383 |
| Hidden checks, test by test | 24 | 108.8 | 117.4 | 116.0 | 118.0 | -8.6 | -22.0 to +2.3 | -2.5 | 15 / 9 / 0 | -118.0 | +28.0 | 0.208 |

**L-N0b minus L-N0 (the yardstick), `matchbook`**

| Measure | Pairs | Mean L-N0b | Mean L-N0 | Median L-N0b | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 11 | 21.0 | 26.0 | 24.0 | 27.0 | -5.0 | -14.4 to +4.5 | -7.0 | 7 / 4 / 0 | -33.0 | +18.0 | 0.352 |
| Uncached input tokens | 11 | 34,666.8 | 46,696.0 | 39,083.0 | 42,646.0 | -12,029.2 | -25,458.3 to +1,570.2 | -15,228.0 | 7 / 4 / 0 | -55,167.0 | +20,873.0 | 0.132 |
| Cache read tokens | 11 | 559,499.6 | 764,322.9 | 633,856.0 | 749,824.0 | -204,823.3 | -544,372.4 to +125,114.2 | -221,440.0 | 7 / 4 / 0 | -1,377,024.0 | +579,328.0 | 0.290 |
| Output tokens | 11 | 9,395.3 | 12,409.2 | 9,324.0 | 11,252.0 | -3,013.9 | -5,759.5 to -311.1 | -3,325.0 | 8 / 3 / 0 | -11,136.0 | +3,154.0 | 0.069 |
| API-equivalent USD | 11 | 0.0138 | 0.0185 | 0.0149 | 0.0174 | -0.0048 | -0.0105 to +0.0010 | -0.0059 | 8 / 3 / 0 | -0.0249 | +0.0089 | 0.161 |
| Wall seconds | 12 | 107.9 | 227.9 | 81.4 | 135.1 | -120.0 | -323.6 to +17.2 | -51.4 | 9 / 3 / 0 | -1,124.5 | +191.0 | 0.197 |
| Hidden checks, as counted | 12 | 88.6 | 87.8 | 104.5 | 107.5 | +0.8 | -25.8 to +27.4 | -0.5 | 6 / 5 / 1 | -104.0 | +113.0 | 0.917 |
| Hidden checks, test by test | 12 | 103.2 | 104.2 | 104.5 | 109.0 | -0.9 | -6.9 to +5.0 | +1.0 | 6 / 6 / 0 | -20.0 | +18.0 | 0.792 |

**L-N0b minus L-N0 (the yardstick), `gridcalc`**

| Measure | Pairs | Mean L-N0b | Mean L-N0 | Median L-N0b | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 12 | 51.2 | 48.1 | 48.0 | 42.5 | +3.1 | -17.8 to +22.8 | +6.0 | 5 / 7 / 0 | -83.0 | +74.0 | 0.774 |
| Uncached input tokens | 12 | 71,678.8 | 65,257.2 | 66,769.5 | 60,672.5 | +6,421.5 | -15,468.6 to +27,596.8 | +14,177.0 | 4 / 8 / 0 | -81,024.0 | +80,164.0 | 0.590 |
| Cache read tokens | 12 | 2,036,010.7 | 1,786,346.7 | 1,801,600.0 | 1,457,280.0 | +249,664.0 | -928,981.3 to +1,361,962.7 | +443,392.0 | 5 / 7 / 0 | -4,667,648.0 | +4,456,960.0 | 0.663 |
| Output tokens | 12 | 19,202.0 | 19,514.2 | 17,776.0 | 18,747.0 | -312.2 | -4,045.2 to +3,389.5 | -432.0 | 6 / 6 / 0 | -14,545.0 | +11,222.0 | 0.879 |
| API-equivalent USD | 12 | 0.0371 | 0.0341 | 0.0337 | 0.0314 | +0.0030 | -0.0126 to +0.0176 | +0.0079 | 5 / 7 / 0 | -0.0621 | +0.0554 | 0.709 |
| Wall seconds | 12 | 229.9 | 268.6 | 203.1 | 163.5 | -38.7 | -177.7 to +91.3 | -10.3 | 6 / 6 / 0 | -592.6 | +311.0 | 0.624 |
| Hidden checks, as counted | 12 | 121.8 | 130.7 | 133.0 | 134.5 | -8.9 | -31.8 to +7.2 | -2.0 | 7 / 3 / 2 | -119.0 | +40.0 | 0.553 |
| Hidden checks, test by test | 12 | 121.8 | 130.7 | 133.0 | 134.5 | -8.9 | -31.8 to +7.2 | -2.0 | 7 / 3 / 2 | -119.0 | +40.0 | 0.553 |

**L-N0b minus L-N0 (the yardstick), both cases**

| Measure | Pairs | Mean L-N0b | Mean L-N0 | Median L-N0b | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 23 | 36.7 | 37.5 | 28.0 | 37.0 | -0.8 | -12.3 to +10.5 | -3.0 | 12 / 11 / 0 | -83.0 | +74.0 | 0.907 |
| Uncached input tokens | 23 | 53,977.4 | 56,380.1 | 43,271.0 | 55,024.0 | -2,402.7 | -15,619.9 to +10,301.7 | +3,541.0 | 11 / 12 / 0 | -81,024.0 | +80,164.0 | 0.743 |
| Cache read tokens | 23 | 1,329,853.2 | 1,297,552.7 | 773,120.0 | 1,103,616.0 | +32,300.5 | -588,410.4 to +640,222.6 | -138,496.0 | 12 / 11 / 0 | -4,667,648.0 | +4,456,960.0 | 0.920 |
| Output tokens | 23 | 14,511.8 | 16,116.1 | 12,812.0 | 15,264.0 | -1,604.3 | -3,961.3 to +740.1 | -1,399.0 | 14 / 9 / 0 | -14,545.0 | +11,222.0 | 0.215 |
| API-equivalent USD | 23 | 0.0260 | 0.0267 | 0.0190 | 0.0248 | -0.0007 | -0.0092 to +0.0074 | -0.0009 | 13 / 10 / 0 | -0.0621 | +0.0554 | 0.875 |
| Wall seconds | 24 | 168.9 | 248.3 | 126.7 | 156.5 | -79.3 | -199.9 to +21.2 | -46.7 | 15 / 9 / 0 | -1,124.5 | +311.0 | 0.211 |
| Hidden checks, as counted | 24 | 105.2 | 109.2 | 116.0 | 118.0 | -4.1 | -20.9 to +11.9 | -1.0 | 13 / 8 / 3 | -119.0 | +113.0 | 0.693 |
| Hidden checks, test by test | 24 | 112.5 | 117.4 | 116.0 | 118.0 | -4.9 | -16.8 to +3.9 | -1.0 | 13 / 9 / 2 | -119.0 | +40.0 | 0.513 |

### Without the two blocks with a timeout

The same contrasts with seed 36 of both cases left out: 11 pairs on
`gridcalc` and 22 over both. `matchbook` without its block is in the tables
above wherever they show 11 pairs. This was not planned before the first
trial. It is here because one trial carries the token means of `gridcalc`.

**L-A0 minus L-N0 (the question), `gridcalc`, without the 2 blocks with a timeout**

| Measure | Pairs | Mean L-A0 | Mean L-N0 | Median L-A0 | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 11 | 49.5 | 49.5 | 45.0 | 43.0 | +0.0 | -23.8 to +21.4 | +2.0 | 5 / 6 / 0 | -92.0 | +47.0 | 1.000 |
| Uncached input tokens | 11 | 75,373.2 | 66,968.0 | 86,425.0 | 62,393.0 | +8,405.2 | -15,683.3 to +32,418.7 | +19,037.0 | 5 / 6 / 0 | -69,307.0 | +63,862.0 | 0.520 |
| Cache read tokens | 11 | 1,984,139.6 | 1,859,211.6 | 1,645,312.0 | 1,470,720.0 | +124,928.0 | -1,126,050.9 to +1,238,458.2 | +174,592.0 | 5 / 6 / 0 | -4,780,032.0 | +2,710,528.0 | 0.865 |
| Output tokens | 11 | 21,673.7 | 19,900.5 | 19,378.0 | 19,649.0 | +1,773.2 | -4,404.5 to +8,271.0 | -1,297.0 | 6 / 5 / 0 | -13,016.0 | +20,224.0 | 0.601 |
| API-equivalent USD | 11 | 0.0382 | 0.0352 | 0.0357 | 0.0317 | +0.0030 | -0.0145 to +0.0194 | +0.0040 | 5 / 6 / 0 | -0.0612 | +0.0392 | 0.749 |
| Wall seconds | 11 | 275.2 | 264.1 | 188.1 | 157.2 | +11.0 | -197.0 to +219.8 | +30.9 | 5 / 6 / 0 | -600.3 | +620.6 | 0.923 |
| Hidden checks, as counted | 11 | 124.0 | 131.7 | 127.0 | 135.0 | -7.7 | -16.8 to +1.6 | -4.0 | 9 / 2 / 0 | -33.0 | +26.0 | 0.149 |
| Hidden checks, test by test | 11 | 124.0 | 131.7 | 127.0 | 135.0 | -7.7 | -16.8 to +1.6 | -4.0 | 9 / 2 / 0 | -33.0 | +26.0 | 0.149 |

**L-A0 minus L-N0 (the question), both cases, without the 2 blocks with a timeout**

| Measure | Pairs | Mean L-A0 | Mean L-N0 | Median L-A0 | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 22 | 34.7 | 37.7 | 29.5 | 37.5 | -3.0 | -15.7 to +8.4 | -2.0 | 13 / 9 / 0 | -92.0 | +47.0 | 0.664 |
| Uncached input tokens | 22 | 57,843.4 | 56,832.0 | 50,938.0 | 56,649.5 | +1,011.4 | -12,827.9 to +14,996.7 | -7,526.5 | 12 / 10 / 0 | -69,307.0 | +63,862.0 | 0.890 |
| Cache read tokens | 22 | 1,285,538.9 | 1,311,767.3 | 950,144.0 | 1,158,656.0 | -26,228.4 | -676,584.7 to +559,837.1 | -73,728.0 | 14 / 8 / 0 | -4,780,032.0 | +2,710,528.0 | 0.941 |
| Output tokens | 22 | 15,937.5 | 16,154.9 | 15,556.5 | 15,816.5 | -217.4 | -3,623.7 to +3,376.7 | -1,487.0 | 14 / 8 / 0 | -13,016.0 | +20,224.0 | 0.907 |
| API-equivalent USD | 22 | 0.0266 | 0.0269 | 0.0233 | 0.0249 | -0.0003 | -0.0095 to +0.0085 | -0.0024 | 14 / 8 / 0 | -0.0612 | +0.0392 | 0.955 |
| Wall seconds | 22 | 197.1 | 201.8 | 134.1 | 150.8 | -4.7 | -111.3 to +101.6 | -0.5 | 11 / 11 / 0 | -600.3 | +620.6 | 0.935 |
| Hidden checks, as counted | 22 | 96.9 | 109.4 | 116.0 | 118.0 | -12.5 | -37.1 to +12.7 | -3.5 | 15 / 7 / 0 | -118.0 | +117.0 | 0.332 |
| Hidden checks, test by test | 22 | 107.9 | 118.3 | 116.0 | 118.0 | -10.4 | -25.0 to +1.5 | -3.5 | 15 / 7 / 0 | -118.0 | +28.0 | 0.149 |

**L-N0b minus L-N0 (the yardstick), `gridcalc`, without the 2 blocks with a timeout**

| Measure | Pairs | Mean L-N0b | Mean L-N0 | Median L-N0b | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 11 | 53.1 | 49.5 | 55.0 | 43.0 | +3.6 | -19.1 to +25.4 | +11.0 | 4 / 7 / 0 | -83.0 | +74.0 | 0.763 |
| Uncached input tokens | 11 | 72,769.3 | 66,968.0 | 71,984.0 | 62,393.0 | +5,801.3 | -18,237.3 to +28,954.8 | +15,110.0 | 4 / 7 / 0 | -81,024.0 | +80,164.0 | 0.651 |
| Cache read tokens | 11 | 2,144,162.9 | 1,859,211.6 | 2,174,976.0 | 1,470,720.0 | +284,951.3 | -1,003,589.8 to +1,512,285.1 | +774,400.0 | 4 / 7 / 0 | -4,667,648.0 | +4,456,960.0 | 0.650 |
| Output tokens | 11 | 19,681.6 | 19,900.5 | 17,898.0 | 19,649.0 | -218.9 | -4,397.4 to +3,784.2 | +474.0 | 5 / 6 / 0 | -14,545.0 | +11,222.0 | 0.923 |
| API-equivalent USD | 11 | 0.0386 | 0.0352 | 0.0384 | 0.0317 | +0.0033 | -0.0139 to +0.0193 | +0.0134 | 4 / 7 / 0 | -0.0621 | +0.0554 | 0.701 |
| Wall seconds | 11 | 240.6 | 264.1 | 211.5 | 157.2 | -23.6 | -179.6 to +115.6 | +33.7 | 5 / 6 / 0 | -592.6 | +311.0 | 0.783 |
| Hidden checks, as counted | 11 | 132.8 | 131.7 | 134.0 | 135.0 | +1.1 | -5.4 to +9.9 | -1.0 | 6 / 3 / 2 | -14.0 | +40.0 | 0.918 |
| Hidden checks, test by test | 11 | 132.8 | 131.7 | 134.0 | 135.0 | +1.1 | -5.4 to +9.9 | -1.0 | 6 / 3 / 2 | -14.0 | +40.0 | 0.918 |

**L-N0b minus L-N0 (the yardstick), both cases, without the 2 blocks with a timeout**

| Measure | Pairs | Mean L-N0b | Mean L-N0 | Median L-N0b | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| API requests | 22 | 37.0 | 37.7 | 26.5 | 37.5 | -0.7 | -13.1 to +11.1 | -1.0 | 11 / 11 / 0 | -83.0 | +74.0 | 0.923 |
| Uncached input tokens | 22 | 53,718.0 | 56,832.0 | 42,763.0 | 56,649.5 | -3,114.0 | -17,225.9 to +10,245.5 | -3,554.5 | 11 / 11 / 0 | -81,024.0 | +80,164.0 | 0.683 |
| Cache read tokens | 22 | 1,351,831.3 | 1,311,767.3 | 768,384.0 | 1,158,656.0 | +40,064.0 | -625,047.3 to +671,627.6 | -59,264.0 | 11 / 11 / 0 | -4,667,648.0 | +4,456,960.0 | 0.904 |
| Output tokens | 22 | 14,538.5 | 16,154.9 | 12,708.5 | 15,816.5 | -1,616.4 | -4,143.4 to +826.8 | -1,473.0 | 13 / 9 / 0 | -14,545.0 | +11,222.0 | 0.234 |
| API-equivalent USD | 22 | 0.0262 | 0.0269 | 0.0188 | 0.0249 | -0.0007 | -0.0098 to +0.0078 | -0.0027 | 12 / 10 / 0 | -0.0621 | +0.0554 | 0.881 |
| Wall seconds | 22 | 175.7 | 201.8 | 134.4 | 150.8 | -26.1 | -109.4 to +50.6 | -31.7 | 13 / 9 / 0 | -592.6 | +311.0 | 0.556 |
| Hidden checks, as counted | 22 | 110.0 | 109.4 | 121.5 | 118.0 | +0.5 | -13.5 to +15.4 | -1.0 | 12 / 7 / 3 | -104.0 | +113.0 | 0.932 |
| Hidden checks, test by test | 22 | 118.0 | 118.3 | 121.5 | 118.0 | -0.4 | -5.0 to +5.0 | -1.0 | 12 / 8 / 2 | -20.0 | +40.0 | 0.916 |

### Per request

Two figures the rule does not cover, over both cases. The first request is
what the harness sends before the solver has done anything.

**L-A0 minus L-N0 (the question), both cases**

| Measure | Pairs | Mean L-A0 | Mean L-N0 | Median L-A0 | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Uncached input tokens per request | 22 | 1,944.1 | 1,619.8 | 1,767.0 | 1,545.1 | +324.4 | +8.5 to +681.4 | +160.8 | 8 / 14 / 0 | -1,237.4 | +3,012.1 | 0.081 |
| First-request input tokens | 24 | 16,534.5 | 15,611.6 | 16,533.5 | 15,612.5 | +922.9 | +921.2 to +924.6 | +923.0 | 0 / 24 / 0 | +916.0 | +931.0 | < 0.001 |

**L-N0b minus L-N0 (the yardstick), both cases**

| Measure | Pairs | Mean L-N0b | Mean L-N0 | Median L-N0b | Median L-N0 | Mean pair difference | 95 % interval | Median pair difference | Lower / higher / equal | Smallest | Largest | Sign-flip p |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Uncached input tokens per request | 23 | 1,639.1 | 1,610.5 | 1,628.5 | 1,535.7 | +28.5 | -145.4 to +198.0 | +12.9 | 11 / 12 / 0 | -962.3 | +790.8 | 0.759 |
| First-request input tokens | 24 | 15,611.0 | 15,611.6 | 15,611.5 | 15,612.5 | -0.6 | -1.9 to +0.8 | +0.0 | 11 / 9 / 4 | -8.0 | +4.0 | 0.479 |

- **Aether's first request is 923 input tokens larger, in 24 of 24 pairs**
  (916 to 931). The two native arms differ by less than one token in the
  mean. This is what Aether adds to every request of a Luna trial; iteration
  1 measured 939 and the counted runs under protocol version 2 measured 930.
- **Uncached input per request is 324 tokens higher through Aether in the
  mean** (interval +8.5 to +681.4; higher in 14 of 22 pairs; `matchbook`
  +449, `gridcalc` +200), and the native repeat differs from the native cell
  by 29 (-145.4 to +198.0). The rule was written for the eight measures and
  is not applied here. This is one interval among many with its lower end
  near zero, and it is against Aether.

### Beside iteration 4

Iteration 4 ran the matched pair in 6 blocks per case. Its yardstick was
L-N5, a second native cell that the model cannot tell from L-N0. Its
hidden checks test by test are from the saved output of its probe for the
12 trials whose grader timed out, and the counted grade for the other 60,
where the two are the same thing. The pooled rows put two sittings
together and resample the pairs within each sitting and case. They
are not a sample planned as one: iteration 4 was read before iteration 5
was planned, and the rule is not applied to them.

**the question: L-A0 minus L-N0 in iteration 5, L-A0 minus L-N0 in iteration 4, both cases**

| Measure | Sitting | Pairs | Mean pair difference | 95 % interval | Lower / higher / equal | Sign-flip p |
| --- | --- | ---: | ---: | --- | --- | ---: |
| API requests | iteration 4 | 12 | -12.6 | -31.2 to +2.9 | 6 / 5 / 1 | 0.215 |
| API requests | iteration 5 | 22 | -3.0 | -15.7 to +8.4 | 13 / 9 / 0 | 0.664 |
| API requests | **pooled over two sittings** | 34 | -6.4 | -16.7 to +2.8 | 19 / 14 / 1 | 0.236 |
| Uncached input tokens | iteration 4 | 12 | -16,194.0 | -42,971.5 to +7,007.8 | 6 / 6 / 0 | 0.261 |
| Uncached input tokens | iteration 5 | 23 | +15,176.4 | -9,095.8 to +48,425.0 | 12 / 11 / 0 | 0.450 |
| Uncached input tokens | **pooled over two sittings** | 35 | +4,420.8 | -14,061.1 to +27,900.9 | 18 / 17 / 0 | 0.784 |
| Cache read tokens | iteration 4 | 12 | -676,202.7 | -1,823,445.3 to +216,960.0 | 6 / 6 / 0 | 0.266 |
| Cache read tokens | iteration 5 | 23 | +1,159,156.9 | -488,503.7 to +3,758,113.4 | 14 / 9 / 0 | 0.533 |
| Cache read tokens | **pooled over two sittings** | 35 | +529,890.7 | -635,304.2 to +2,313,984.0 | 20 / 15 / 0 | 0.804 |
| Output tokens | iteration 4 | 12 | -3,242.0 | -11,506.1 to +3,438.3 | 6 / 6 / 0 | 0.524 |
| Output tokens | iteration 5 | 23 | +1,769.6 | -2,779.6 to +6,922.6 | 14 / 9 / 0 | 0.562 |
| Output tokens | **pooled over two sittings** | 35 | +51.3 | -4,007.9 to +4,239.7 | 20 / 15 / 0 | 0.983 |
| API-equivalent USD | iteration 4 | 12 | -0.0100 | -0.0281 to +0.0044 | 6 / 6 / 0 | 0.315 |
| API-equivalent USD | iteration 5 | 23 | +0.0140 | -0.0069 to +0.0458 | 14 / 9 / 0 | 0.525 |
| API-equivalent USD | **pooled over two sittings** | 35 | +0.0058 | -0.0095 to +0.0278 | 20 / 15 / 0 | 0.792 |
| Wall seconds | iteration 4 | 12 | -29.6 | -139.0 to +73.8 | 6 / 6 / 0 | 0.610 |
| Wall seconds | iteration 5 | 24 | -9.9 | -151.1 to +128.3 | 12 / 12 / 0 | 0.899 |
| Wall seconds | **pooled over two sittings** | 36 | -16.4 | -118.3 to +83.2 | 18 / 18 / 0 | 0.767 |
| Hidden checks, as counted | iteration 4 | 12 | -34.3 | -72.8 to +4.8 | 9 / 2 / 1 | 0.089 |
| Hidden checks, as counted | iteration 5 | 24 | -10.6 | -33.6 to +12.3 | 15 / 9 / 0 | 0.383 |
| Hidden checks, as counted | **pooled over two sittings** | 36 | -18.5 | -38.0 to +1.4 | 24 / 11 / 1 | 0.086 |
| Hidden checks, test by test | iteration 4 | 12 | -11.0 | -18.2 to -4.2 | 10 / 2 / 0 | 0.024 |
| Hidden checks, test by test | iteration 5 | 24 | -8.6 | -22.0 to +2.3 | 15 / 9 / 0 | 0.208 |
| Hidden checks, test by test | **pooled over two sittings** | 36 | -9.4 | -18.3 to -1.4 | 25 / 11 / 0 | 0.030 |

**the yardstick: L-N0b minus L-N0 in iteration 5, L-N5 minus L-N0 in iteration 4, both cases**

| Measure | Sitting | Pairs | Mean pair difference | 95 % interval | Lower / higher / equal | Sign-flip p |
| --- | --- | ---: | ---: | --- | --- | ---: |
| API requests | iteration 4 | 12 | -4.5 | -21.8 to +8.6 | 5 / 6 / 1 | 0.750 |
| API requests | iteration 5 | 23 | -0.8 | -12.3 to +10.5 | 12 / 11 / 0 | 0.907 |
| API requests | **pooled over two sittings** | 35 | -2.1 | -11.7 to +6.9 | 17 / 17 / 1 | 0.686 |
| Uncached input tokens | iteration 4 | 12 | -8,743.7 | -34,602.3 to +12,176.0 | 6 / 6 / 0 | 0.593 |
| Uncached input tokens | iteration 5 | 23 | -2,402.7 | -15,619.9 to +10,301.7 | 11 / 12 / 0 | 0.743 |
| Uncached input tokens | **pooled over two sittings** | 35 | -4,576.8 | -16,713.0 to +6,895.5 | 17 / 18 / 0 | 0.484 |
| Cache read tokens | iteration 4 | 12 | -465,962.7 | -1,567,018.7 to +339,690.7 | 5 / 7 / 0 | 0.542 |
| Cache read tokens | iteration 5 | 23 | +32,300.5 | -588,410.4 to +640,222.6 | 12 / 11 / 0 | 0.920 |
| Cache read tokens | **pooled over two sittings** | 35 | -138,532.6 | -686,804.1 to +362,956.8 | 17 / 18 / 0 | 0.641 |
| Output tokens | iteration 4 | 12 | -3,092.5 | -11,844.9 to +3,847.7 | 4 / 8 / 0 | 0.589 |
| Output tokens | iteration 5 | 23 | -1,604.3 | -3,961.3 to +740.1 | 14 / 9 / 0 | 0.215 |
| Output tokens | **pooled over two sittings** | 35 | -2,114.5 | -5,462.9 to +795.5 | 18 / 17 / 0 | 0.227 |
| API-equivalent USD | iteration 4 | 12 | -0.0071 | -0.0249 to +0.0063 | 5 / 7 / 0 | 0.568 |
| API-equivalent USD | iteration 5 | 23 | -0.0007 | -0.0092 to +0.0074 | 13 / 10 / 0 | 0.875 |
| API-equivalent USD | **pooled over two sittings** | 35 | -0.0029 | -0.0110 to +0.0044 | 18 / 17 / 0 | 0.508 |
| Wall seconds | iteration 4 | 12 | -34.5 | -138.7 to +72.2 | 5 / 7 / 0 | 0.553 |
| Wall seconds | iteration 5 | 24 | -79.3 | -199.9 to +21.2 | 15 / 9 / 0 | 0.211 |
| Wall seconds | **pooled over two sittings** | 36 | -64.4 | -154.5 to +13.5 | 20 / 16 / 0 | 0.153 |
| Hidden checks, as counted | iteration 4 | 12 | -7.5 | -46.2 to +30.7 | 7 / 5 / 0 | 0.696 |
| Hidden checks, as counted | iteration 5 | 24 | -4.1 | -20.9 to +11.9 | 13 / 8 / 3 | 0.693 |
| Hidden checks, as counted | **pooled over two sittings** | 36 | -5.2 | -22.2 to +11.2 | 20 / 13 / 3 | 0.552 |
| Hidden checks, test by test | iteration 4 | 12 | -7.5 | -17.3 to +2.0 | 7 / 5 / 0 | 0.228 |
| Hidden checks, test by test | iteration 5 | 24 | -4.9 | -16.8 to +3.9 | 13 / 9 / 2 | 0.513 |
| Hidden checks, test by test | **pooled over two sittings** | 36 | -5.8 | -14.4 to +1.0 | 20 / 14 / 2 | 0.174 |

- **Pooled, hidden checks test by test are lower through Aether**: -9.4
  (-18.3 to -1.4), lower in 25 of 36 pairs. The pooled yardstick is -5.8
  (-14.4 to +1.0), lower in 20 of 36.
- **By case, the pooled interval excludes zero on `gridcalc` only**: -9.6
  (-16.5 to -2.8) there, lower through Aether in 15 of 18 pairs (6 of 6 in
  iteration 4 and 9 of 12 in iteration 5); -9.2 (-26.4 to +4.2) on
  `matchbook`, lower in 10 of 18.
- **Pooled, no measure of spend or time has an interval that excludes
  zero**, in the question or in the yardstick.

### Verdict for Luna, iteration 5

**No Aether effect is claimed on any of the eight measures.** In 24 matched
pairs, Luna through Aether is not distinguishable from Luna in Codex at
this sample size. Applied as written, with hidden checks test by test as
the plan words them, the count each trial recorded:

| Measure | L-A0 minus L-N0, both cases: mean and 95 % interval | (1) excludes zero | Yardstick L-N0b minus L-N0: mean and 95 % interval | (2) includes zero | Mean in `matchbook` | Mean in `gridcalc` | (3) same sign | Verdict | Effect the interval still allows, in % of the L-N0 mean |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- | --- |
| API requests | -3.0 (-15.7 to +8.4) | no | -0.8 (-12.3 to +10.5) | yes | -6.0 | +0.0 | no | not distinguishable | -41.6 % to +22.3 % |
| Uncached input tokens | +15,176.4 (-9,095.8 to +48,425.0) | no | -2,402.7 (-15,619.9 to +10,301.7) | yes | -6,382.5 | +34,938.7 | no | not distinguishable | -16.1 % to +85.9 % |
| Cache read tokens | +1,159,156.9 (-488,503.7 to +3,758,113.4) | no | +32,300.5 (-588,410.4 to +640,222.6) | yes | -177,384.7 | +2,384,320.0 | no | not distinguishable | -37.6 % to +289.6 % |
| Output tokens | +1,769.6 (-2,779.6 to +6,922.6) | no | -1,604.3 (-3,961.3 to +740.1) | yes | -2,208.0 | +5,415.8 | no | not distinguishable | -17.2 % to +43.0 % |
| API-equivalent USD | +0.0140 (-0.0069 to +0.0458) | no | -0.0007 (-0.0092 to +0.0074) | yes | -0.0035 | +0.0300 | no | not distinguishable | -25.7 % to +171.9 % |
| Wall seconds | -9.9 (-151.1 to +128.3) | no | -79.3 (-199.9 to +21.2) | yes | -103.4 | +83.6 | no | not distinguishable | -60.9 % to +51.7 % |
| Hidden checks, as counted | -10.6 (-33.6 to +12.3) | no | -4.1 (-20.9 to +11.9) | yes | -15.2 | -5.9 | yes | not distinguishable | -30.8 % to +11.3 % |
| Hidden checks, test by test | -8.6 (-22.0 to +2.3) | no | -4.9 (-16.8 to +3.9) | yes | -11.3 | -5.9 | yes | not distinguishable | -18.8 % to +2.0 % |

- **Condition 1 fails for every measure**: each interval of L-A0 minus L-N0
  over both cases includes zero. Condition 2 holds for every measure.
  Condition 3 holds for the two hidden-check measures only.
- **One interval of the yardstick excludes zero**, on `matchbook` alone:
  the second run of the native command wrote 3,014 fewer output tokens
  than the first (-5,759.5 to -311.1, 11 pairs). The two commands are the
  same, so this is what one interval among 48 does by chance, and it is why
  the rule reads the yardstick beside the question.
- **Spend and time point in opposite directions in the two cases**: lower
  through Aether on `matchbook` and higher on `gridcalc` on uncached input,
  cache read, output tokens, cost and wall seconds. On `gridcalc` one
  stopped trial carries most of the mean: without the two blocks with a
  timeout the difference over both cases is +1,011 uncached input tokens
  (-12,828 to +14,997) and -0.0003 USD (-0.0095 to +0.0085).
- **Hidden checks lean below native in both cases, and the lean is not
  claimed.** Counted: -10.6 (-33.6 to +12.3), lower in 15 of 24 pairs. Test
  by test: -8.6 (-22.0 to +2.3), lower in 15 of 24. The native repeat leans
  the same way against the native cell: -4.1 and -4.9, lower in 13 of 24.
- **What the intervals still allow.** On cost, from 25.7 % below native to
  171.9 % above; without the stopped trial's block, from 35 % below to 32 %
  above. On uncached input, from 16.1 % below to 85.9 % above. On hidden
  checks test by test, from 18.8 % below native to 2.0 % above; as counted,
  from 30.8 % below to 11.3 % above. A sample of this size does not rule
  out an effect of that size in either direction.
- **The yardstick is wide.** The same native command run twice in one block
  differed by a median of 0.0127 USD, 48 % of the native mean cost, and by
  up to 0.0621 USD; in hidden checks by a median of 5 and by up to 119.
  The matched pair differed by a median of 0.0107 USD and of 16 checks as
  counted.

### What iteration 5 points to

**The loop found and fixed the Haiku cause. For Luna, 24 matched pairs
with a native repeat beside them do not show Aether apart from Codex on
any measure.** The counted difference of the second measurement, +0.0119
USD (+51.9 %) in the mean of 10 pairs, is of the size by which a mean of 10
pairs of the same native command moves: the yardstick's pair differences
have a standard deviation of 0.0216 USD, about 0.013 either way for a mean
of 10. That does not make the difference zero: the intervals above say what
is still possible.

- **No change to Aether is supported**, for spend or for accuracy.
- **What is measured and fixed in size** is the 923 input tokens Aether
  adds to each request. The two differences without an arm, the
  app-server's longer skill listing and Aether's own MCP tools and
  instructions, are inside those tokens and are still not separated. No
  claimed effect asks for it.
- **The one lean that repeats is hidden checks on `gridcalc`**: lower
  through Aether in 9 of 12 pairs here and in 6 of 6 in iteration 4. It is
  not claimed: in this sitting its interval includes zero (-15.0 to +2.9),
  and the pooled row is not a planned sample. It is the place a further
  sample should look. On the spread of the pair differences over both
  cases (a standard deviation of 31 checks test by test), an interval half
  as wide as this one needs about four times the pairs.
- **The test-by-test grade takes the hangs out of the comparison.** The
  yardstick's interval on hidden checks is -20.9 to +11.9 as counted and
  -16.8 to +3.9 test by test; on `matchbook` alone, -25.8 to +27.4 against
  -6.9 to +5.0. The counted grade stays the grade.

## After the loop: the counted test again

On 2026-10-11 the counted comparison was run again on the same binary, seeds 6
to 10 ([report](harness-lower-tier-results-2026-10-11.md)). Over all ten
version 2 runs, 20 pairs per model, Aether minus native in the mean with its
95 % interval: output tokens, Haiku -4,205 (-13,442 to +4,333) and Luna +4,887
(+1,451 to +8,458); cost, Haiku -0.0266 USD (-0.0868 to +0.0305) and Luna
+0.0127 USD (+0.0015 to +0.0263); hidden checks as counted, Haiku -0.2 pp
(-0.7 to +0.2) and Luna +3.2 pp (-16.1 to +23.0). These are counted trials;
none is merged with a diagnostic trial.

## Limits

- **Six blocks.** A mean here is a mean of 6, and a count of blocks is out
  of 6. No significance is claimed.
- **One case.** `matchbook` only. The counted Haiku runs read again above
  include `gridcalc`; nothing else here does.
- **One machine, shared, 43 minutes**, with the two batches overlapping.
- **H-A1 does not remove the connectors**, as stated under the arms.
- **The envelope is a fixture** with a fixed nonce and task id.
- **An edit is a file-editing tool call.** A file written from a shell
  command is not seen, so "before the first edit" is unknown for a trial
  that wrote all its files that way.
- **Test runs are commands that name `pytest` or `unittest`.** "Tests added,
  as stated" is read from the solver's last message and was stated in too
  few trials to tabulate; it is in the per-trial output.
- **Haiku dollars are the CLI's own figure** and swing more than the output
  tokens do, because a trial with more requests reads its conversation from
  cache more often (block 1: 0.2968 USD for H-N0's 97,499 output tokens and
  21 requests, 0.1081 for H-N1's 74,008 and 10). The token columns carry the
  contrasts.
- **Effort is requested.** As in the comparison, Claude Code does not report
  the effort it applied.
- **Tool definitions sent to Luna are not recorded** by Codex, so the Luna
  context table has no row for them.

For iteration 2, besides the above:

- **Six Haiku blocks and ten Luna blocks**, one case, one machine, one
  sitting. A count of blocks is out of 6 or 10, or out of 16 where the two
  iterations are put together.
- **Across the two iterations is across two sittings.** Only arms of one
  block share their load and their hour. The native Haiku cell moved by
  4,300 thinking tokens between the sittings with nothing changed.
- **No arm of iteration 2 carries the words "Effort low."**, because the
  binary no longer writes them for an engine. What the fix did through
  Aether is therefore read across sittings and not within a block.
- **H-N4 and H-A0 load the operator's own connectors.** Another account
  would load other servers, or none.
- **No call to an Aether tool was observed under `--strict-mcp-config`.**
  The server's tools are listed in every such trial; no solver called one.
- **A trial at 0 is usually a hang, not a solution that passes nothing**
  (five of the six in iteration 2). The grader's single limit for all 119
  hidden tests is part of the frozen protocol and was not changed. The
  per-test counts are a second look and are not the grade.
- **One Luna trial reached the timeout and loaded the machine** for 18
  minutes, during which a Haiku block waited. A guard against a repeat ran
  for the rest of the batches and did not fire.
- **The stop rule's 5 % bar is finer than these samples.** A mean of 6 or 10
  pairs whose single differences span more than the mean itself cannot
  establish a difference within 5 %. Part 2 of the stop rule is reported as
  the rule reads, and is not evidence of a cost.

For iteration 4, besides the above:

- **Six blocks per case, twelve pairs per contrast**, one machine, one
  sitting, twelve trials at once. A count is out of 6 or 12. No significance
  is claimed, and no difference is called real on a mean.
- **The approval policy has no native arm.** `codex exec` does not run under
  `on-request` with full access, so the policy is isolated only through
  Aether, on a binary patched for the purpose. L-N5 measures two native
  cells against each other and nothing else.
- **Two differences have no arm at all**: the app-server's longer skill
  listing and Aether's own MCP tools. They are only what is left over.
- **The parts are added as if they did not interact.** Two are measured
  natively and one through Aether.
- **The permission text holds the operator's own list of approved command
  prefixes.** Another machine's text would be shorter or longer.
- **What a solver did is read from its commands.** A file written from a
  shell command is not a file change, and "only reads" is a list of command
  names.
- **Reasoning tokens are what Codex reports**, per request; a trial with
  none reported is counted as 0.

For iteration 5, besides the above:

- **Twelve blocks per case, 24 pairs per contrast**, one machine, one
  sitting of 57 minutes, up to twelve trials at once. The intervals and
  p-values are descriptive, and 8 measures are read in 2 contrasts and 3
  groupings.
- **"Not distinguishable" is a statement about this sample.** It is not a
  finding that the two are equal; the intervals say how large an effect
  may still be.
- **The interval is a percentile bootstrap of 24 pairs** with a few very
  large pair differences. On such data it can be too narrow.
- **Two solvers were stopped at the timeout** and are recorded unlike each
  other, as said under
  [Trials graded 0, timeouts and hung grades](#trials-graded-0-timeouts-and-hung-grades).
- **The yardstick is the native cell against itself in the same block.** It
  measures how far two runs of one command differ on this machine in this
  sitting, and nothing about another day.
- **The test-by-test grade is not the counted grade.** It gives each hidden
  test 5 seconds and a process of its own, so a test that needs state left
  by another, or more time, reads differently.
- **One test-by-test grade was cut by its total limit.** Its recorded count
  is used, as planned, and is a lower bound: 46 of that trial's 119 tests
  were not run. A solution that hangs in more than about 23 tests cannot be
  graded in full under the limits.
- **The pooled rows join two sittings** that were not planned as one
  sample.
- **Two guards could end processes** in a trial's workspace; one did, once,
  in a native trial.

## Raw data and how to reproduce

Every figure above is in a table that gives each trial's value. The
manifests (as written and re-priced), the manifest of the aborted launch, the
host readings, the reader's per-trial output, and the script that printed the
tables from them are kept outside the repository and are not committed: they
hold local paths. No session transcript is committed either; the reader's
tests use a synthetic one.

Live runs spend model tokens on the logged-in Claude and Codex accounts.

```sh
cargo build --release
python3 -m unittest discover -s evals -p 'test_harness_bench*.py'   # no model calls

# One Haiku block and one Luna block. Repeat with seeds 1 to 6.
python3 evals/harness_bench.py --diagnostic --cases matchbook --runs 1 --seed 1 --jobs 5 --timeout 1200 \
  --conditions native-haiku-low,diag-native-haiku-low-envelope,diag-native-haiku-low-envelope-noeffort,aether-single-haiku-low,diag-aether-haiku-low-minimal \
  --binary target/release/aether --output-dir target/diag/haiku
python3 evals/harness_bench.py --diagnostic --cases matchbook --runs 1 --seed 1 --jobs 3 --timeout 1200 \
  --conditions native-luna-low,aether-single-luna-low,aether-solo-minimal-luna-low \
  --binary target/release/aether --output-dir target/diag/luna

# The figures a manifest does not hold, per trial.
python3 evals/harness_bench_transcript.py target/diag/haiku/harness-bench-*.json --output target/diag/haiku-transcripts.json
python3 evals/harness_bench_transcript.py target/diag/luna/harness-bench-*.json --output target/diag/luna-transcripts.json
```

`harness_bench_compare.py` and `harness_bench_report.py` refuse these
manifests; that is intended.

Iteration 2, from a build of the commit that changes the status line. The
first block above no longer reproduces iteration 1 exactly: the binary has
changed, and `aether-single-haiku-low` now starts without the connectors.

```sh
# One Haiku block and one Luna block. Haiku: seeds 11 to 16. Luna: seeds 11 to 20.
python3 evals/harness_bench.py --diagnostic --cases matchbook --runs 1 --seed 11 --jobs 5 --timeout 1200 \
  --conditions native-haiku-low,diag-native-haiku-low-nocoord,diag-native-haiku-low-nocoord-connectors,diag-aether-haiku-low-connectors,aether-single-haiku-low \
  --binary target/release/aether --output-dir target/diag/haiku
python3 evals/harness_bench.py --diagnostic --cases matchbook --runs 1 --seed 11 --jobs 3 --timeout 1200 \
  --conditions native-luna-low,aether-single-luna-low,aether-solo-minimal-luna-low \
  --binary target/release/aether --output-dir target/diag/luna
```

The manifests of iteration 2, the host readings, the guard's log, the
reader's per-trial output, the per-test counts of the trials at 0 and the
scripts that printed the tables are kept beside those of iteration 1,
outside the repository.

Iteration 4, Luna only. The release build is the one above; the diagnostic
binary is the same source with the one-line patch quoted under
[The arms of iteration 4](#the-arms-of-iteration-4), built into its own
target directory and never committed or installed.

```sh
git apply approval-never.patch
CARGO_TARGET_DIR=target/diag-never cargo build --release
git apply -R approval-never.patch && git diff --quiet -- crates

# One block of one case: both invocations started together. Repeat with seeds 21 to 26, and for gridcalc.
python3 evals/harness_bench.py --diagnostic --cases matchbook --runs 1 --seed 21 --jobs 5 --timeout 1200 \
  --conditions native-luna-low,diag-native-luna-low-onrequest,diag-native-luna-low-nomultiagent,diag-native-luna-low-envelope,aether-single-luna-low \
  --binary target/release/aether --output-dir target/diag4/matchbook/main &
python3 evals/harness_bench.py --diagnostic --cases matchbook --runs 1 --seed 21 --jobs 1 --timeout 1200 \
  --conditions diag-aether-luna-low-approval-never \
  --binary target/diag-never/release/aether --output-dir target/diag4/matchbook/never &
wait
```

The manifests of iteration 4, the host readings, the guard's log, the
reader's per-trial output, the per-test counts of the trials at 0, the patch
and the scripts that printed the tables are kept beside those of the earlier
iterations, outside the repository.

Iteration 5, Luna only, on the release build above.

```sh
# One block of one case. Repeat with seeds 31 to 42, and for gridcalc.
python3 evals/harness_bench.py --diagnostic --cases matchbook --runs 1 --seed 31 --jobs 3 --timeout 1200 \
  --conditions native-luna-low,diag-native-luna-low-repeat,aether-single-luna-low \
  --binary target/release/aether --output-dir target/diag5/matchbook/s1

# The requests and the first-request input of each trial.
python3 evals/harness_bench_transcript.py target/diag5/*/*/harness-bench-*.json --output target/diag5/transcripts.json
```

The manifests of iteration 5, the host readings, the guards' log, the
reader's per-trial output, the test-by-test grade finished after the
sitting, and the script that computed the intervals and p-values and
printed the tables, with its output, are kept beside those of the earlier
iterations, outside the repository. The script holds the bootstrap's seed.
The tables as corrected on 2026-10-11 to the recorded test-by-test count,
with the script that printed them from the same per-trial output, are kept
beside them.
