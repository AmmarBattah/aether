# Native CLI versus Aether: Haiku 5.5 and GPT-6 Luna, second sitting — 2026-10-11

This is the second sitting of the benchmark first reported in
[harness-bench-2026-10-10.md](harness-bench-2026-10-10.md): the same 72
trials, run again on the tree that has `main` merged in. It asks the same
question: does a model lose accuracy, tokens, cost or time when it runs as an
Aether agent instead of directly in its own CLI?

In short, from this sitting alone:

- **Claude Haiku 5.5.** Mean accuracy was 99.6 % on both sides, with 13 full
  passes of 18 natively and 12 through Aether. Aether passed fewer checks in 2
  pairs, more in 1, and the same number in 15. Haiku is close to the ceiling
  on these cases, so equal scores here are not evidence that nothing is lost.
  Through Aether, total tokens were 26.9 % lower (in 17 of 18 pairs),
  API-equivalent cost 29.4 % lower (in 17 of 18 pairs) and wall time 12.6 %
  lower (in 14 of 18 pairs).
- **GPT-6 Luna.** In the mean, Aether was behind on every token, cost and
  time measure. Through Aether, total tokens were 4.2 % higher, uncached
  input tokens 28.8 % higher, API-equivalent cost 11.4 % higher and wall time
  62.6 % higher, and one Aether trial timed out at 900 s. Full passes were 1
  of 18 through Aether against 2 of 18 natively. Mean accuracy was 95.4 %
  through Aether against 93.7 % natively; Aether passed fewer checks in 7
  pairs, more in 7, and the same number in 4.
- **Beside the first sitting.** For Haiku the two sittings agree in direction
  on every measure. For Luna they differ: in the first sitting Luna through
  Aether used 23.9 % fewer tokens, cost 14.6 % less and scored 4.3 points
  lower. This report does not say why. The sittings differ in several things
  at once, and three runs per case is a small sample.

## What is compared

Each pair uses the same model, the same requested effort (medium), and the
same CLI engine:

- **Claude Haiku 5.5** (`claude-haiku-5-5`): native Claude Code against one
  Aether agent on `claude-code/claude-haiku-5-5`.
- **GPT-6 Luna** (`gpt-6-luna`): native Codex against one Aether agent on
  `codex/gpt-6-luna`.

Six cases (`billing`, `semver`, `minilang`, `concurrent_v2`, `eventlog`,
`leases`), three runs per case and cell, 72 trials. Seed 393 and a 900 s
timeout, as in the first sitting. Each Aether trial is a single agent with no
delegation, so the comparison measures harness overhead, not multi-agent
coordination.

## Method

- **Tasks:** the six repair tasks of the first sitting, each graded by
  held-out checks that are never copied into a solver's workspace: `billing`
  (79 checks), `semver` (80), `minilang` (102), `concurrent_v2` (67),
  `eventlog` (49) and `leases` (28). Fixtures, hidden tests, objectives and
  prompts are byte for byte those of the first sitting.
- **Runs:** 3 runs per case and cell, 72 trials. Seed 393, the seeded
  run-major latin rotation order, a 900 s timeout. Every trial got a fresh Git
  workspace, and every Aether trial its own isolated daemon.
- **Tree and binary:** all trials ran from clean commit `6985d442`: the merge
  of `main` (`f86d8252`) into this branch (`bdff5c1a`) and the commit that
  holds the rules below. `git diff origin/main HEAD -- crates clients scripts`
  is empty, so the Aether binary is built from the Rust source of `main`. Its
  SHA-256 is
  `0028dffa8ef56c638d1bdd9d3b989109fa8be05c01d3a0cdd6eacd3fc29e7128`
  (`aether 0.1.1`). Claude Code 2.1.294 and Codex CLI 0.162.1. The hashes are
  recorded in the manifests.
- **Protocol version:** 2, recorded in both manifests. Both sides of the
  Claude pair start without the machine user's MCP servers.
- **Lanes and blocks:** the Haiku pair and the Luna pair ran as two lanes at
  once, one runner each. Within a lane the native and the Aether trial of a
  pair ran side by side in one block (`--jobs 2`), and a block ended before
  the next started. So up to four trials ran at once.
- **Memory guard:** both lanes ran under the guard of the earlier sittings. It
  ends any process started inside a trial workspace of this checkout once it
  holds more than 4 GB, and leaves the grader alone. Only its log path and the
  name of the runner script it waits for were changed.
- **Measures:**
  - **Accuracy:** the fraction of hidden checks passed per trial, averaged. A
    *full pass* is every check passed. The counted grade is the one-process
    grade of the first sitting. The runner now records a second grade, test
    by test, which is not counted; the two agree in every trial here.
  - **Tokens:** uncached input, output, cache reads and cache writes; total
    tokens is their sum. Claude Code caches nearly all input, so Haiku's input
    appears almost entirely in the cache columns. Codex reports no cache
    writes.
  - **Cost:** API-equivalent USD, which is not a subscription charge. Haiku's
    is Claude Code's reported dollar metadata on both sides. Luna's uses list
    prices from the pinned binary's price table: estimated from Codex receipts
    on the native side, priced by Aether's runtime on the Aether side.
  - **Time:** wall seconds from launch to finished workflow, or to the timeout.
- **What was checked in the records of all 72 trials:**
  - **Model.** Every trial resolved to its own model in its session metadata:
    36 trials `claude-haiku-5-5`, 36 trials `gpt-6-luna`. Luna's effort reads
    `medium` in the Codex session of all 36 Luna trials. Claude Code records
    no effort in its session, so Haiku's medium effort is requested on both
    sides and not confirmed.
  - **Tree and binary.** Both manifests record a clean tree at `6985d442`, the
    binary hash above, protocol version 2, and neither a pilot nor a
    diagnostic run. All 18 Aether trials of Haiku record that the runner
    added `--strict-mcp-config`.
  - **Violations.** No trial records a protocol, integrity or contract
    violation, and none was blocked. Every Aether trial had no descendant
    agent and a settled, quiet tree at the end.
  - **Tokens and cost.** Token and cost coverage is complete in all 72
    trials; no trial is unpriced and no figure is a lower bound.
  - **Grade.** The counted grade was made in all 72 trials. No grader hung,
    so the test-by-test grade equals the counted grade in every trial. No
    trial was graded 0; the lowest was 53 of 79 checks.
  - **Hidden checks.** No session log of any trial names the hidden tests,
    the fixture directories or the reference implementations.
- **The machine.** Before the start it had 6,126 MB of memory available, 39 GB
  of disk free and a one-minute load of 6.26. Other long work ran on it
  throughout. The one-minute load at the start or end of a trial ranged from
  0.98 to 8.84. Available memory, sampled every 20 s, stayed above 3,000 MB
  except in two samples at 14:39:45 and 14:40:05 UTC (1,825 MB and 2,129 MB),
  just before the [memory guard](#timeouts-failures-guard-firings-stray-writes-and-unknown-figures)
  fired.

## The same as the first sitting, and not the same

The same, with the evidence:

- **Cases.** The fixtures, hidden tests and public tests of the six cases are
  byte for byte those of the first sitting: `git diff --quiet 5bc4d8d HEAD`
  over each of the six fixture directories exits 0.
- **Objectives and prompts.** The objective and the built prompt of each of
  the six cases, compared between the first sitting's tree (`5bc4d8d`) and
  this one, are identical.
- **Native command lines.** The built native command of both models is
  identical in the two trees. The native Claude command already carried
  `--strict-mcp-config` in the first sitting.
- **Aether cells.** The agent parameters of both Aether cells (model, effort,
  no delegation) are identical in the two trees.
- **Matrix.** The same four cells, six cases, three runs, seed 393, 900 s
  timeout and seeded run-major latin rotation order. Two lanes at once, one
  per model, on the same machine.
- **Counted grade.** The one-process grade of all hidden checks of a case.

Not the same:

| What | First sitting | This sitting |
| --- | --- | --- |
| Protocol version | 1 (not recorded in a manifest then) | 2, recorded in both manifests |
| The Aether trial of Haiku | Claude Code started without `--strict-mcp-config`, so the machine user's MCP servers could load | the runner adds `--strict-mcp-config`, recorded in all 18 trials, so they do not load |
| Tree | `5bc4d8d` | `6985d442`, with `main` merged in: 33 commits that `5bc4d8d` lacks, 5 of them under `crates/` |
| Aether binary (SHA-256) | `fe7d3f2a8eb9…` | `0028dffa8ef5…` |
| Engine status line of an Aether turn | carried the effort words | no effort words (`f346e09b`); the first turn's status line reads "Turn 1." in the session logs of both Aether cells |
| Trials at once | the two lanes, one trial each: 2 | the two lanes, a pair each (`--jobs 2`): up to 4 |
| Claude Code | 2.1.295 | 2.1.294 |
| Codex CLI | not named in the first report | 0.162.1 |
| What the runner records | the counted grade | also the test-by-test grade, the block of each trial and the host load |
| Day and load | 2026-10-10 | 2026-10-11, beside other long work |

`--jobs 2` is the one deliberate difference. It puts the native and the Aether
trial of a pair side by side, so both see the same load. Everything else in
the table came with the merge or with the day. Protocol version 2 changes
nothing for Codex: the Luna pair differs from the first sitting in the Aether
binary, the trials at once and the day, and in no rule of the protocol.

## Rules, committed before the first trial

These rules were committed in `6985d442` at 13:51:32 UTC. The first trial
started at 13:51:44 UTC.

**Which trials count.** All 72 trials of this sitting are counted whole,
whatever they show. No trial is added, dropped or repeated on the strength of
a result. A trial that times out or fails is counted as it stands. A trial
ended by the memory guard is counted as it stands and listed.

**Which sitting leads.** The README and the pull request lead with this
sitting, because it is the one measured on the current `main`. The first
sitting stays in its own report as measured. The two sittings are never pooled
into one figure: they differ in protocol version and in the Aether binary.

**What a difference between the sittings means.** A difference between the two
sittings is not attributed to one change. Between them changed: the protocol
version (1 to 2), the Aether binary (the status line without the effort words,
and everything else merged since), the trials run at once, the day and the
load of the machine.

**The statistics.** They are computed per model over its 18 pairs. A pair is
the native trial and the Aether trial of the same case and run index. The
measures are hidden checks passed (in percentage points of the case's checks),
total tokens, uncached input tokens, output tokens, cache read tokens,
API-equivalent USD and wall seconds. For each measure the report gives:

- the mean and the median of each side;
- the mean and the median pair difference, Aether minus native;
- the mean pair difference relative to the native mean;
- the number of pairs in which Aether is lower, higher and equal;
- the smallest and the largest pair difference;
- a 95 % interval of the mean pair difference, by a paired bootstrap with
  10,000 resamples of whole pairs and the fixed seed 395 (percentile
  interval);
- the two-sided sign-flip p-value of the mean pair difference, exact over all
  2^18 sign assignments.

Hidden checks passed are also given per case, over the three pairs of each
case. A pair that lacks a measure on one side is left out of that measure and
counted as unknown.

These are descriptive. Several measures are looked at for each of two models,
so one interval among them that excludes zero is not by itself a finding.
Three runs per case is a small sample.

**Wall time.** Each lane runs the native and the Aether trial of a pair side
by side (`--jobs 2`), and the two lanes run at once. The machine is busy with
other long work during the sitting, and its load changes from minute to
minute. Wall time is compared within a pair only, never with the first
sitting's.

## Results

Means per trial. Each row is 18 trials: 3 runs of 6 cases.

| Model | Harness | Trials | Mean accuracy | Full passes | Finished workflows | Uncached input | Output | Cache reads | Cache writes | Total tokens | API-equiv. USD | Wall s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Native CLI | 18 | 99.6% | 13/18 | 18/18 | 35 | 61,467 | 1,129,754 | 77,314 | 1,268,569 | $0.0678 | 253 |
| Claude Haiku 5.5 | Aether | 18 | 99.6% | 12/18 | 18/18 | 27 | 48,175 | 808,196 | 70,735 | 927,132 | $0.0479 | 221 |
| Claude Haiku 5.5 | **Aether − native** |  | -0.1 pp | -1 | +0 | -22% | -22% | -28% | -9% | -27% | -29% | -13% |
| GPT-6 Luna | Native CLI | 18 | 93.7% | 2/18 | 18/18 | 28,150 | 6,078 | 292,537 | 0 | 326,765 | $0.0088 | 86 |
| GPT-6 Luna | Aether | 18 | 95.4% | 1/18 | 17/18 | 36,265 | 6,358 | 297,828 | 0 | 340,450 | $0.0098 | 140 |
| GPT-6 Luna | **Aether − native** |  | +1.7 pp | -1 | -1 | +29% | +5% | +2% | n/a | +4% | +11% | +63% |

Mean accuracy by case, with the checks passed in each of the three runs:

| Case | Checks | Claude Haiku 5.5 native | Claude Haiku 5.5 Aether | GPT-6 Luna native | GPT-6 Luna Aether |
| --- | ---: | ---: | ---: | ---: | ---: |
| `billing` | 79 | 98.7% (78, 78, 78) | 98.7% (78, 78, 78) | 86.9% (75, 53, 78) | 94.1% (77, 69, 77) |
| `concurrent_v2` | 67 | 99.0% (67, 66, 66) | 99.0% (66, 66, 67) | 98.5% (66, 66, 66) | 99.0% (67, 66, 66) |
| `eventlog` | 49 | 100.0% (49, 49, 49) | 100.0% (49, 49, 49) | 96.6% (47, 48, 47) | 96.6% (46, 48, 48) |
| `leases` | 28 | 100.0% (28, 28, 28) | 100.0% (28, 28, 28) | 94.0% (23, 28, 28) | 96.4% (27, 27, 27) |
| `minilang` | 102 | 100.0% (102, 102, 102) | 100.0% (102, 102, 102) | 95.1% (89, 101, 101) | 93.1% (100, 92, 93) |
| `semver` | 80 | 100.0% (80, 80, 80) | 99.6% (80, 80, 79) | 90.8% (72, 67, 79) | 92.9% (79, 65, 79) |

- **Haiku.** 25 of its 36 trials passed every check. The other 11 each missed
  exactly one check: all six `billing` trials (78 of 79 on both sides in every
  run), four of the six `concurrent_v2` trials, and one Aether trial of
  `semver`.
- **Luna.** 3 of its 36 trials passed every check: two native `leases` runs
  and one Aether `concurrent_v2` run. 17 trials missed exactly one check, 8
  natively and 9 through Aether. The lowest trials were native `billing` run 2
  (53 of 79), Aether `semver` run 2 (65 of 80) and native `semver` run 2 (67
  of 80).
- **Finished workflows.** 71 of 72. One Aether trial of Luna, `minilang` run
  1, ended at the 900 s timeout; its workspace still graded 100 of 102.

## The statistics

As [planned](#rules-committed-before-the-first-trial), per model over its 18
pairs. "Lower" means the Aether trial's figure is below its native partner's.
Hidden checks are in percentage points of each case's checks.

| Model | Measure | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed (percent) | 99.6 % | 99.6 % | 100.0 % | 100.0 % | -0.1 pp | +0.0 pp |  | 2 / 1 / 15 | -1.5 pp | +1.5 pp | -0.3 pp to +0.2 pp | 1.000 | 18 |
| Claude Haiku 5.5 | Total tokens | 1,268,569 | 927,132 | 1,233,942 | 837,908 | -341,437 | -303,538 | -26.9 % | 17 / 1 / 0 | -881,774 | +549,492 | -488,207 to -186,040 | 0.001 | 18 |
| Claude Haiku 5.5 | Uncached input tokens | 35 | 27 | 33 | 28 | -8 | -8 | -21.8 % | 16 / 1 / 1 | -22 | +12 | -11 to -4 | 0.001 | 18 |
| Claude Haiku 5.5 | Output tokens | 61,467 | 48,175 | 53,699 | 45,736 | -13,292 | -6,408 | -21.6 % | 16 / 2 / 0 | -120,991 | +5,809 | -27,389 to -4,786 | < 0.001 | 18 |
| Claude Haiku 5.5 | Cache read tokens | 1,129,754 | 808,196 | 1,087,774 | 736,786 | -321,559 | -267,316 | -28.5 % | 17 / 1 / 0 | -826,953 | +537,151 | -463,367 to -171,567 | 0.001 | 18 |
| Claude Haiku 5.5 | API-equivalent USD | 0.0678 | 0.0479 | 0.0543 | 0.0428 | -0.0200 | -0.0097 | -29.4 % | 17 / 1 / 0 | -0.1338 | +0.0096 | -0.0372 to -0.0074 | < 0.001 | 18 |
| Claude Haiku 5.5 | Wall seconds | 253.5 | 221.5 | 232.9 | 212.4 | -32.0 | -29.1 | -12.6 % | 14 / 4 / 0 | -332.6 | +103.8 | -76.6 to +1.6 | 0.104 | 18 |
| GPT-6 Luna | Hidden checks passed (percent) | 93.7 % | 95.4 % | 98.2 % | 97.5 % | +1.7 pp | +0.0 pp |  | 7 / 7 / 4 | -8.8 pp | +20.3 pp | -1.4 pp to +5.3 pp | 0.361 | 18 |
| GPT-6 Luna | Total tokens | 326,765 | 340,450 | 324,436 | 285,284 | +13,686 | -21,034 | +4.2 % | 13 / 5 / 0 | -169,882 | +498,075 | -41,933 to +85,578 | 0.780 | 18 |
| GPT-6 Luna | Uncached input tokens | 28,150 | 36,265 | 30,370 | 32,738 | +8,115 | +10,434 | +28.8 % | 6 / 12 / 0 | -8,600 | +27,886 | +3,117 to +13,235 | 0.007 | 18 |
| GPT-6 Luna | Output tokens | 6,078 | 6,358 | 6,216 | 6,535 | +280 | +14 | +4.6 % | 9 / 9 / 0 | -2,279 | +3,181 | -344 to +955 | 0.425 | 18 |
| GPT-6 Luna | Cache read tokens | 292,537 | 297,828 | 282,112 | 253,440 | +5,291 | -32,512 | +1.8 % | 13 / 5 / 0 | -160,000 | +468,224 | -47,374 to +73,685 | 0.914 | 18 |
| GPT-6 Luna | API-equivalent USD | 0.0088 | 0.0098 | 0.0090 | 0.0094 | +0.0010 | +0.0005 | +11.4 % | 7 / 11 / 0 | -0.0031 | +0.0089 | -0.0001 to +0.0023 | 0.122 | 18 |
| GPT-6 Luna | Wall seconds | 86.0 | 139.8 | 74.7 | 88.9 | +53.8 | +16.8 | +62.6 % | 7 / 11 / 0 | -144.9 | +841.6 | -15.5 to +160.6 | 0.324 | 18 |

Hidden checks per case, in checks, over the three pairs of each case. With
three pairs the smallest possible two-sided p-value is 0.250.

| Model | Case | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | `billing` (79 checks) | 78.0 | 78.0 | 78.0 | 78.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `concurrent_v2` (67 checks) | 66.3 | 66.3 | 66.0 | 66.0 | +0.0 | +0.0 | +0.0 % | 1 / 1 / 1 | -1.0 | +1.0 | -1.0 to +1.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `eventlog` (49 checks) | 49.0 | 49.0 | 49.0 | 49.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `leases` (28 checks) | 28.0 | 28.0 | 28.0 | 28.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `minilang` (102 checks) | 102.0 | 102.0 | 102.0 | 102.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `semver` (80 checks) | 80.0 | 79.7 | 80.0 | 80.0 | -0.3 | +0.0 | -0.4 % | 1 / 0 / 2 | -1.0 | +0.0 | -1.0 to +0.0 | 1.000 | 3 |
| GPT-6 Luna | `billing` (79 checks) | 68.7 | 74.3 | 75.0 | 77.0 | +5.7 | +2.0 | +8.3 % | 1 / 2 / 0 | -1.0 | +16.0 | -1.0 to +16.0 | 0.500 | 3 |
| GPT-6 Luna | `concurrent_v2` (67 checks) | 66.0 | 66.3 | 66.0 | 66.0 | +0.3 | +0.0 | +0.5 % | 0 / 1 / 2 | +0.0 | +1.0 | +0.0 to +1.0 | 1.000 | 3 |
| GPT-6 Luna | `eventlog` (49 checks) | 47.3 | 47.3 | 47.0 | 48.0 | +0.0 | +0.0 | +0.0 % | 1 / 1 / 1 | -1.0 | +1.0 | -1.0 to +1.0 | 1.000 | 3 |
| GPT-6 Luna | `leases` (28 checks) | 26.3 | 27.0 | 28.0 | 27.0 | +0.7 | -1.0 | +2.5 % | 2 / 1 / 0 | -1.0 | +4.0 | -1.0 to +4.0 | 1.000 | 3 |
| GPT-6 Luna | `minilang` (102 checks) | 97.0 | 95.0 | 101.0 | 93.0 | -2.0 | -8.0 | -2.1 % | 2 / 1 / 0 | -9.0 | +11.0 | -9.0 to +11.0 | 1.000 | 3 |
| GPT-6 Luna | `semver` (80 checks) | 72.7 | 74.3 | 72.0 | 79.0 | +1.7 | +0.0 | +2.3 % | 1 / 1 / 1 | -2.0 | +7.0 | -2.0 to +7.0 | 1.000 | 3 |

The same figures in words:

- **Where Aether is behind with Claude Haiku 5.5, in the mean:** hidden checks passed 99.6 % against 99.6 % (-0.1 pp; lower in 2 of 18 pairs, higher in 1, equal in 15; 95 % interval -0.3 pp to +0.2 pp, p 1.000).
- **Where Aether is ahead with Claude Haiku 5.5, in the mean:** total tokens 927,132 against 1,268,569 (-341,437, -26.9 %; lower in 17 of 18 pairs, higher in 1, equal in 0; 95 % interval -488,207 to -186,040, p 0.001); uncached input tokens 27 against 35 (-8, -21.8 %; lower in 16 of 18 pairs, higher in 1, equal in 1; 95 % interval -11 to -4, p 0.001); output tokens 48,175 against 61,467 (-13,292, -21.6 %; lower in 16 of 18 pairs, higher in 2, equal in 0; 95 % interval -27,389 to -4,786, p < 0.001); cache read tokens 808,196 against 1,129,754 (-321,559, -28.5 %; lower in 17 of 18 pairs, higher in 1, equal in 0; 95 % interval -463,367 to -171,567, p 0.001); API-equivalent USD 0.0479 against 0.0678 (-0.0200, -29.4 %; lower in 17 of 18 pairs, higher in 1, equal in 0; 95 % interval -0.0372 to -0.0074, p < 0.001); wall seconds 221.5 against 253.5 (-32.0, -12.6 %; lower in 14 of 18 pairs, higher in 4, equal in 0; 95 % interval -76.6 to +1.6, p 0.104).
- **Where Aether is behind with GPT-6 Luna, in the mean:** total tokens 340,450 against 326,765 (+13,686, +4.2 %; lower in 13 of 18 pairs, higher in 5, equal in 0; 95 % interval -41,933 to +85,578, p 0.780); uncached input tokens 36,265 against 28,150 (+8,115, +28.8 %; lower in 6 of 18 pairs, higher in 12, equal in 0; 95 % interval +3,117 to +13,235, p 0.007); output tokens 6,358 against 6,078 (+280, +4.6 %; lower in 9 of 18 pairs, higher in 9, equal in 0; 95 % interval -344 to +955, p 0.425); cache read tokens 297,828 against 292,537 (+5,291, +1.8 %; lower in 13 of 18 pairs, higher in 5, equal in 0; 95 % interval -47,374 to +73,685, p 0.914); API-equivalent USD 0.0098 against 0.0088 (+0.0010, +11.4 %; lower in 7 of 18 pairs, higher in 11, equal in 0; 95 % interval -0.0001 to +0.0023, p 0.122); wall seconds 139.8 against 86.0 (+53.8, +62.6 %; lower in 7 of 18 pairs, higher in 11, equal in 0; 95 % interval -15.5 to +160.6, p 0.324).
- **Where Aether is ahead with GPT-6 Luna, in the mean:** hidden checks passed 95.4 % against 93.7 % (+1.7 pp; lower in 7 of 18 pairs, higher in 7, equal in 4; 95 % interval -1.4 pp to +5.3 pp, p 0.361).

What the intervals say, measure by measure:

- **Haiku, intervals that exclude zero:** total tokens, uncached input tokens,
  output tokens, cache read tokens and API-equivalent USD, all lower through
  Aether. Uncached input is 27 against 35 tokens per trial, which is of no
  practical size: Claude Code caches nearly all of its input.
- **Haiku, intervals that include zero:** hidden checks (−0.3 pp to +0.2 pp)
  and wall seconds (−76.6 s to +1.6 s). Haiku's mean accuracy is 99.62 %
  natively and 99.55 % through Aether; both round to 99.6 %.
- **Luna, the interval that excludes zero:** uncached input tokens, higher
  through Aether by 8,115 tokens per trial (+3,117 to +13,235), in 12 of 18
  pairs.
- **Luna, intervals that include zero:** hidden checks, total tokens, output
  tokens, cache read tokens, API-equivalent USD and wall seconds.
- **Luna's means and medians point in different directions on tokens.** The
  mean pair difference in total tokens is +13,686, but the median is −21,034
  and Aether used fewer tokens in 13 of 18 pairs. The pair with the timeout
  alone has a difference of +498,075 tokens and +841.6 s. Without that one
  pair, which is not a planned figure, Aether's means over the other 17 pairs
  are 4.5 % lower in total tokens, 6.2 % higher in cost, 8.5 % higher in wall
  time and 1.2 pp higher in accuracy. The pair counts in every headline figure.
- **Per case.** With three pairs per case no per-case difference can be told
  from chance, and none is claimed.

The figures in a row are each rounded on their own, so a rounded difference
can differ from the difference of two rounded means by one in the last place.

## Beside the first sitting

The first sitting is reported in
[harness-bench-2026-10-10.md](harness-bench-2026-10-10.md). Its manifests are
not kept, so its statistics here are recomputed from that report's "Every
trial" table, in which cost is rounded to four decimals and wall time to whole
seconds. The recomputed means match the first report's, except that Haiku's
cost through Aether is $0.0396 from the rounded rows against the $0.0395 it
printed. The two sittings are shown side by side and are never pooled.

Means per trial, the mean pair difference (Aether minus native), the pairs in
which Aether is lower, higher and equal, the 95 % interval of the mean pair
difference and the sign-flip p-value, for each sitting:

| Model | Measure | First sitting: native | First sitting: Aether | First sitting: Aether minus native | First sitting: relative to native | First sitting: lower / higher / equal | First sitting: 95 % interval | First sitting: p | This sitting: native | This sitting: Aether | This sitting: Aether minus native | This sitting: relative to native | This sitting: lower / higher / equal | This sitting: 95 % interval | This sitting: p |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed (percent) | 99.6 % | 99.6 % | -0.1 pp |  | 1 / 0 / 17 | -0.2 pp to +0.0 pp | 1.000 | 99.6 % | 99.6 % | -0.1 pp |  | 2 / 1 / 15 | -0.3 pp to +0.2 pp | 1.000 |
| Claude Haiku 5.5 | Total tokens | 1,026,479 | 782,820 | -243,659 | -23.7 % | 15 / 3 / 0 | -428,144 to -75,170 | 0.012 | 1,268,569 | 927,132 | -341,437 | -26.9 % | 17 / 1 / 0 | -488,207 to -186,040 | 0.001 |
| Claude Haiku 5.5 | Uncached input tokens | 28 | 25 | -3 | -11.5 % | 13 / 4 / 1 | -7 to +1 | 0.170 | 35 | 27 | -8 | -21.8 % | 16 / 1 / 1 | -11 to -4 | 0.001 |
| Claude Haiku 5.5 | Output tokens | 52,924 | 39,819 | -13,104 | -24.8 % | 17 / 1 / 0 | -18,141 to -8,051 | < 0.001 | 61,467 | 48,175 | -13,292 | -21.6 % | 16 / 2 / 0 | -27,389 to -4,786 | < 0.001 |
| Claude Haiku 5.5 | Cache read tokens | 897,620 | 678,727 | -218,893 | -24.4 % | 15 / 3 / 0 | -397,015 to -56,724 | 0.019 | 1,129,754 | 808,196 | -321,559 | -28.5 % | 17 / 1 / 0 | -463,367 to -171,567 | 0.001 |
| Claude Haiku 5.5 | API-equivalent USD | 0.0622 | 0.0396 | -0.0227 | -36.4 % | 16 / 2 / 0 | -0.0396 to -0.0086 | < 0.001 | 0.0678 | 0.0479 | -0.0200 | -29.4 % | 17 / 1 / 0 | -0.0372 to -0.0074 | < 0.001 |
| Claude Haiku 5.5 | Wall seconds | 226.4 | 170.6 | -55.9 | -24.7 % | 17 / 1 / 0 | -78.1 to -34.7 | < 0.001 | 253.5 | 221.5 | -32.0 | -12.6 % | 14 / 4 / 0 | -76.6 to +1.6 | 0.104 |
| GPT-6 Luna | Hidden checks passed (percent) | 97.5 % | 93.2 % | -4.3 pp |  | 8 / 5 / 5 | -14.8 pp to +2.0 pp | 0.637 | 93.7 % | 95.4 % | +1.7 pp |  | 7 / 7 / 4 | -1.4 pp to +5.3 pp | 0.361 |
| GPT-6 Luna | Total tokens | 398,842 | 303,439 | -95,404 | -23.9 % | 15 / 3 / 0 | -157,549 to -43,199 | 0.002 | 326,765 | 340,450 | +13,686 | +4.2 % | 13 / 5 / 0 | -41,933 to +85,578 | 0.780 |
| GPT-6 Luna | Uncached input tokens | 31,943 | 31,221 | -722 | -2.3 % | 10 / 8 / 0 | -7,209 to +5,192 | 0.836 | 28,150 | 36,265 | +8,115 | +28.8 % | 6 / 12 / 0 | +3,117 to +13,235 | 0.007 |
| GPT-6 Luna | Output tokens | 7,149 | 6,148 | -1,000 | -14.0 % | 12 / 6 / 0 | -1,810 to -93 | 0.039 | 6,078 | 6,358 | +280 | +4.6 % | 9 / 9 / 0 | -344 to +955 | 0.425 |
| GPT-6 Luna | Cache read tokens | 359,751 | 266,069 | -93,682 | -26.0 % | 15 / 3 / 0 | -150,471 to -44,629 | 0.001 | 292,537 | 297,828 | +5,291 | +1.8 % | 13 / 5 / 0 | -47,374 to +73,685 | 0.914 |
| GPT-6 Luna | API-equivalent USD | 0.0104 | 0.0089 | -0.0015 | -14.6 % | 14 / 4 / 0 | -0.0029 to -0.0003 | 0.029 | 0.0088 | 0.0098 | +0.0010 | +11.4 % | 7 / 11 / 0 | -0.0001 to +0.0023 | 0.122 |
| GPT-6 Luna | Wall seconds | 101.3 | 97.9 | -3.3 | -3.3 % | 8 / 10 / 0 | -36.7 to +27.2 | 0.847 | 86.0 | 139.8 | +53.8 | +62.6 % | 7 / 11 / 0 | -15.5 to +160.6 | 0.324 |

Where the two sittings agree:

- **Haiku's accuracy.** 99.6 % on both sides in both sittings, with 13 full
  passes natively and 12 through Aether both times. Aether was lower in 1 pair
  and higher in none then, lower in 2 and higher in 1 now.
- **Haiku's tokens and cost.** Lower through Aether in both sittings, with
  intervals that exclude zero both times: total tokens 23.7 % lower then and
  26.9 % lower now, cost 36.4 % lower then and 29.4 % lower now.
- **Luna's full passes.** Fewer through Aether in both sittings: 3 against 6
  then, 1 against 2 now.

Where they differ:

- **Haiku's wall time.** 24.7 % lower through Aether then, with an interval
  that excluded zero (−78.1 s to −34.7 s). 12.6 % lower now, with an interval
  that includes zero (−76.6 s to +1.6 s). The pairs ran one after the other
  then and side by side now.
- **Luna's accuracy.** 4.3 pp lower through Aether then, 1.7 pp higher now.
  Both intervals include zero. Most of the first sitting's gap was one run
  (`leases` run 2, 3 of 28).
- **Luna's tokens and cost.** Then, total tokens were 23.9 % lower through
  Aether and cost 14.6 % lower, with intervals that excluded zero. Now total
  tokens are 4.2 % higher and cost 11.4 % higher, with intervals that include
  zero. Uncached input was 2.3 % lower then and is 28.8 % higher now, with an
  interval that excludes zero.
- **Luna's wall time.** 3.3 % lower through Aether then, 62.6 % higher now,
  of which one timeout pair carries 841.6 s of the 968.7 s summed over the 18
  pairs. Both intervals include zero.

The native side moved too, although its command lines did not change. Native
Luna's mean accuracy went from 97.5 % to 93.7 % and its full passes from 6 to
2; its total tokens went from 398,842 to 326,765 per trial. Native Haiku's
total tokens went from 1,026,479 to 1,268,569 per trial. So the two sittings
differ by more than the harness, and by the rule above no difference between
them is attributed to one change.

### What became of the three things the first report named

1. **Haiku's native runs with prompts above 105k tokens.** The first report
   named three native runs whose largest prompt exceeded 105k tokens, and no
   Aether run above 87k. This sitting has two such native runs: `minilang` run
   1 (122,385 tokens; $0.1866, 2,139,207 total tokens) and `semver` run 2
   (111,196 tokens; $0.1393, 2,157,326 total tokens). They are the two most
   expensive trials of the sitting. No Aether trial exceeded 105k, but the
   largest Aether prompt was 103,299 tokens (`minilang` run 3), well above the
   first sitting's 87k. Without those two pairs, which is not a planned
   figure, Aether's means over the other 16 pairs are 24.5 % lower in total
   tokens, 16.2 % lower in cost and 15.7 % lower in wall time. The largest
   prompt is read here from the session logs as the input, cache read and
   cache write tokens of one request; how the first report measured it is not
   recorded.
2. **Luna's `leases` run with the garbled path.** It did not happen again as a
   write. Two Luna trials through Aether named a wrong absolute path in an
   `apply_patch` call: `744453503a` (`billing` run 1) a path in the machine
   user's home directory outside the repository, and `b013491656` (`semver`
   run 3) its own trial directory with part of the path doubled. Both calls
   carried an empty hunk and were refused; nothing was written or deleted. The
   trials went on and graded 77 of 79 and 79 of 80. No native Luna trial named
   such a path. Luna's three `leases` runs through Aether each passed 27 of
   28.
3. **Luna's lower full-pass count through Aether.** It is lower again, 1
   against 2, but both sides are far below the first sitting's 3 against 6.
   On `leases` all three Aether runs missed one check, as two of three did in
   the first sitting, while two of three native runs passed everything. On
   `eventlog` no run passed everything on either side this time.

The first sitting's own statistics, by the same method and seed:

| Model | Measure | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed (percent) | 99.6 % | 99.6 % | 100.0 % | 100.0 % | -0.1 pp | +0.0 pp |  | 1 / 0 / 17 | -1.5 pp | +0.0 pp | -0.2 pp to +0.0 pp | 1.000 | 18 |
| Claude Haiku 5.5 | Total tokens | 1,026,479 | 782,820 | 900,757 | 785,973 | -243,659 | -224,494 | -23.7 % | 15 / 3 / 0 | -1,325,748 | +559,023 | -428,144 to -75,170 | 0.012 | 18 |
| Claude Haiku 5.5 | Uncached input tokens | 28 | 25 | 26 | 23 | -3 | -4 | -11.5 % | 13 / 4 / 1 | -26 | +18 | -7 to +1 | 0.170 | 18 |
| Claude Haiku 5.5 | Output tokens | 52,924 | 39,819 | 52,228 | 40,962 | -13,104 | -10,931 | -24.8 % | 17 / 1 / 0 | -37,915 | +11,832 | -18,141 to -8,051 | < 0.001 | 18 |
| Claude Haiku 5.5 | Cache read tokens | 897,620 | 678,727 | 770,450 | 667,012 | -218,893 | -194,827 | -24.4 % | 15 / 3 / 0 | -1,267,874 | +564,746 | -397,015 to -56,724 | 0.019 | 18 |
| Claude Haiku 5.5 | API-equivalent USD | 0.0622 | 0.0396 | 0.0498 | 0.0429 | -0.0227 | -0.0085 | -36.4 % | 16 / 2 / 0 | -0.1131 | +0.0080 | -0.0396 to -0.0086 | < 0.001 | 18 |
| Claude Haiku 5.5 | Wall seconds | 226.4 | 170.6 | 227.0 | 181.5 | -55.9 | -49.0 | -24.7 % | 17 / 1 / 0 | -168.0 | +36.0 | -78.1 to -34.7 | < 0.001 | 18 |
| GPT-6 Luna | Hidden checks passed (percent) | 97.5 % | 93.2 % | 98.6 % | 98.0 % | -4.3 pp | +0.0 pp |  | 8 / 5 / 5 | -85.7 pp | +13.9 pp | -14.8 pp to +2.0 pp | 0.637 | 18 |
| GPT-6 Luna | Total tokens | 398,842 | 303,439 | 313,262 | 307,566 | -95,404 | -100,358 | -23.9 % | 15 / 3 / 0 | -472,099 | +109,686 | -157,549 to -43,199 | 0.002 | 18 |
| GPT-6 Luna | Uncached input tokens | 31,943 | 31,221 | 29,506 | 31,976 | -722 | -508 | -2.3 % | 10 / 8 / 0 | -39,434 | +22,992 | -7,209 to +5,192 | 0.836 | 18 |
| GPT-6 Luna | Output tokens | 7,149 | 6,148 | 6,432 | 6,116 | -1,000 | -1,262 | -14.0 % | 12 / 6 / 0 | -3,502 | +3,875 | -1,810 to -93 | 0.039 | 18 |
| GPT-6 Luna | Cache read tokens | 359,751 | 266,069 | 282,112 | 265,344 | -93,682 | -89,216 | -26.0 % | 15 / 3 / 0 | -431,104 | +98,304 | -150,471 to -44,629 | 0.001 | 18 |
| GPT-6 Luna | API-equivalent USD | 0.0104 | 0.0089 | 0.0088 | 0.0089 | -0.0015 | -0.0010 | -14.6 % | 14 / 4 / 0 | -0.0091 | +0.0031 | -0.0029 to -0.0003 | 0.029 | 18 |
| GPT-6 Luna | Wall seconds | 101.3 | 97.9 | 76.0 | 84.5 | -3.3 | +4.0 | -3.3 % | 8 / 10 / 0 | -133.0 | +131.0 | -36.7 to +27.2 | 0.847 | 18 |

Its hidden checks per case, in checks:

| Model | Case | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | `billing` (79 checks) | 78.0 | 78.0 | 78.0 | 78.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `concurrent_v2` (67 checks) | 66.7 | 66.3 | 67.0 | 66.0 | -0.3 | +0.0 | -0.5 % | 1 / 0 / 2 | -1.0 | +0.0 | -1.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `eventlog` (49 checks) | 49.0 | 49.0 | 49.0 | 49.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `leases` (28 checks) | 28.0 | 28.0 | 28.0 | 28.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `minilang` (102 checks) | 102.0 | 102.0 | 102.0 | 102.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| Claude Haiku 5.5 | `semver` (80 checks) | 79.7 | 79.7 | 80.0 | 80.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 3 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 3 |
| GPT-6 Luna | `billing` (79 checks) | 74.0 | 77.7 | 77.0 | 78.0 | +3.7 | +0.0 | +5.0 % | 0 / 1 / 2 | +0.0 | +11.0 | +0.0 to +11.0 | 1.000 | 3 |
| GPT-6 Luna | `concurrent_v2` (67 checks) | 65.3 | 66.3 | 66.0 | 66.0 | +1.0 | +1.0 | +1.5 % | 0 / 2 / 1 | +0.0 | +2.0 | +0.0 to +2.0 | 0.500 | 3 |
| GPT-6 Luna | `eventlog` (49 checks) | 48.0 | 48.0 | 49.0 | 48.0 | +0.0 | -1.0 | +0.0 % | 2 / 1 / 0 | -1.0 | +2.0 | -1.0 to +2.0 | 1.000 | 3 |
| GPT-6 Luna | `leases` (28 checks) | 27.7 | 19.0 | 28.0 | 27.0 | -8.7 | -1.0 | -31.3 % | 3 / 0 / 0 | -24.0 | -1.0 | -24.0 to -1.0 | 0.250 | 3 |
| GPT-6 Luna | `minilang` (102 checks) | 100.7 | 101.3 | 102.0 | 102.0 | +0.7 | +0.0 | +0.7 % | 1 / 1 / 1 | -2.0 | +4.0 | -2.0 to +4.0 | 1.000 | 3 |
| GPT-6 Luna | `semver` (80 checks) | 78.7 | 77.3 | 79.0 | 77.0 | -1.3 | -1.0 | -1.7 % | 2 / 0 / 1 | -3.0 | +0.0 | -3.0 to +0.0 | 0.500 | 3 |

## Reading

- **Haiku: accuracy could not separate the harnesses; tokens and cost were
  lower through Aether.** 15 of 18 pairs passed the same number of checks, and
  25 of 36 trials passed all of them. At that ceiling equal scores are not
  evidence that nothing is lost: the interval of the mean pair difference runs
  from −0.3 pp to +0.2 pp. Total tokens were 26.9 % lower through Aether and
  cost 29.4 % lower, each in 17 of 18 pairs, with intervals that exclude zero.
  Wall time was 12.6 % lower, in 14 of 18 pairs, with an interval that
  includes zero.
- **Part of Haiku's cost gap is again two native runs with very large
  prompts.** Without those two pairs the cost gap is 16.2 % instead of
  29.4 %.
- **Luna: Aether was behind on cost and time, and not clearly different on
  accuracy.** In the mean, Luna through Aether used 4.2 % more total tokens,
  cost 11.4 % more and took 62.6 % longer; those three intervals include
  zero. It used 28.8 % more uncached input tokens, in 12 of 18 pairs, with an
  interval that excludes zero. Mean accuracy was 1.7 pp higher through Aether,
  with 7 pairs each way and an interval from −1.4 pp to +5.3 pp, so no
  accuracy difference is claimed in either direction. Full passes were 1
  against 2.
- **One Aether trial of Luna timed out.** Its session stopped producing
  records 111 s after the start and was cancelled at 900 s. No native trial
  timed out. One timeout in 18 trials says little about how often this
  happens.
- **The first sitting's Luna result did not repeat.** Luna through Aether was
  cheaper then and is dearer now; it scored lower then and higher now. With
  18 pairs per sitting and both sides moving between the sittings, neither
  sitting settles the Luna pair.
- **No harness reached 100 %.** Every cell missed checks on at least one case.

The benchmark measures these differences and does not explain them. Nothing
here shows that the merge changed a result: the sittings differ in the
protocol version, the Aether binary, the trials at once, the day and the load,
and the native side moved as well.

## Timeouts, failures, guard firings, stray writes and unknown figures

Every trial below is counted as it stands.

**Timeouts: one.** Trial `e3ffe1a772`, Luna through Aether, `minilang` run 1.
It started at 14:00:53 UTC. The last record in its Codex session log is at
14:02:44 UTC, a reasoning item two seconds after a file change. After that the
session log and the trial's isolated daemon log hold nothing, and no tool
process of the trial was running. The runner cancelled it at 900 s with "no
terminal event within 900s". The cause is not known. Its workspace graded 100
of 102 checks, and its token and cost figures are complete (839,411 total
tokens, $0.0185). Its native partner finished in 58 s with 89 of 102.

**Failures: none.** No trial failed to start, lost its grade, or lost a token
or cost figure. The runner marks a trial `failed` when it does not pass every
check; 44 of the 72 trials did not, and one of those is the timeout above.

**Memory guard firings: one.** Trial `7812248082`, Haiku through Aether,
`minilang` run 2. At 14:40:08 UTC the guard ended a `python3` process of
4,424,568 kB that the solver had started in its workspace: a test script of
the solver's own. The solver saw exit code 137, removed from its script the
line that fed the interpreter a chain of 150 exponentiations, and went on. The
trial finished at 321 s and graded 102 of 102. The guard touched no other
process.

**Trials graded 0: none.** The lowest grade is 53 of 79.

**Writes outside a trial's workspace.** After the lanes ended, `git status`
of the checkout outside `target/` was clean. The session log of every trial
was then searched for file-editing calls and for absolute paths in shell
commands that are not under the trial's workspace:

| Trial | Cell | Case, run | What it did | What became of it |
| --- | --- | --- | --- | --- |
| `19e0f31078` | Haiku native | `billing` 3 | wrote a test file to `workspace/tests/` at the root of the benchmark checkout instead of its own workspace | the solver noticed, removed that directory itself and wrote the file in its workspace; nothing was left; 78 of 79 |
| `6eac871e48` | Haiku Aether | `billing` 2 | wrote one test script under `/tmp` | 1 file moved into the trial's artifacts |
| `37eccf7c71` | Haiku Aether | `minilang` 1 | wrote four test scripts under `/tmp` | 4 files moved into the trial's artifacts |
| `7812248082` | Haiku Aether | `minilang` 2 | wrote two test scripts under `/tmp`; one of them is the process the guard ended | 2 files moved into the trial's artifacts |
| `da8fd0db84` | Haiku Aether | `minilang` 3 | wrote three test scripts under `/tmp` | 3 files moved into the trial's artifacts |
| `509def375b` | Haiku Aether | `concurrent_v2` 2 | wrote a test script under `/tmp` | not there after the lanes |
| `d7c66da397`, `a0bfc97d69` | Haiku native | `concurrent_v2` 1 and 3 | each wrote a test script under `/tmp` | not there after the lanes |
| `febe6e4e1d`, `4a5f16c358` | Haiku native | `minilang` 1 and 2 | each wrote two test scripts under `/tmp` | not there after the lanes |
| `97d13e24e6` | Haiku native | `semver` 2 | wrote a test script under `/tmp` | not there after the lanes |
| `f3f48487df` | Luna native | `minilang` 1 | wrote a script under `/tmp` that edits its workspace, and ran it | 1 file moved into the trial's artifacts |
| `744453503a` | Luna Aether | `billing` 1 | named a path in the machine user's home directory in an `apply_patch` with an empty hunk | the call was refused; nothing was written; 77 of 79 |
| `b013491656` | Luna Aether | `semver` 3 | named its trial directory with part of the path doubled in an `apply_patch` with an empty hunk | the call was refused; nothing was written; 79 of 80 |

The eleven files found were moved into a `stray-writes/` directory of their
trial, and the two directories the solvers had made for them under `/tmp`
were removed once empty. All of them are test or helper scripts of the
solver's own; none holds a hidden check. Haiku wrote such scripts on both
sides, five trials natively and five through Aether; Luna did once, natively.
Two more Haiku native trials (`9a7bee2f20`, `0a7285866b`) changed directory to
`/tmp` to run a check from the command line and wrote nothing there.

**Unknown figures.**

- The cause of the timeout.
- Whether Haiku ran at medium effort: it is requested on both sides and not
  recorded by Claude Code.
- The Codex CLI version of the first sitting, and how the first report
  measured a largest prompt.
- No token or cost figure of this sitting is unknown, unpriced or a lower
  bound.

## Limits

- **Sample.** Three runs per case is a small sample: 18 pairs per model. The
  intervals and p-values are descriptive. Seven measures are looked at for
  each of two models, so one interval that excludes zero is not by itself a
  finding. The same native cell moved by 3.8 pp of accuracy between the two
  sittings.
- **Ceiling.** 25 of 36 Haiku trials passed every check. Accuracy could
  hardly separate the harnesses for Haiku, and its near-equal scores are not
  evidence that nothing is lost.
- **Two sittings, several changes.** The sittings differ in protocol version,
  Aether binary, trials at once, CLI version, day and load. They are not a
  controlled comparison of any one of these.
- **Haiku tokens:** both sides come from Claude process receipts, so they are
  comparable.
- **Luna tokens:** the two sides come from different sources. Native counts
  come from Codex rollout receipts. Aether counts come from its durable
  counters, which label themselves as possibly omitting helper models; these
  trials had no descendants.
- **Cost.** Dollars are API list-price equivalents, not subscription charges.
- **Wall time.** Up to four trials ran at once on a machine busy with other
  work. Wall time is comparable within a pair only, not with the first
  sitting's and not as an absolute latency.
- **Writes outside a workspace.** Both sides run without a sandbox, so either
  can write anywhere the machine user can. The search reads the paths of
  file-editing calls and the absolute paths in shell commands. A write through
  a relative path or from inside a script would not be seen, and `/tmp` is
  shared with other work on the machine, so it was not listed whole.
- **A solver can see the checkout.** A trial's workspace lies under the
  benchmark checkout's `target/` directory. One native Haiku trial listed the
  checkout's root. No session log names the hidden tests.
- **First sitting's statistics.** They are recomputed from a table with
  rounded cost and wall time, not from its manifests.

## Reproduce

```sh
python3 -m unittest discover -s evals -p 'test_harness_bench*.py'
python3 evals/harness_bench.py --runs 3 --seed 393 --timeout 900 --jobs 2 \
  --cases billing,semver,minilang,concurrent_v2,eventlog,leases \
  --conditions native-haiku-medium,aether-single-haiku-medium \
  --binary target/release/aether --output-dir target/hb395-s2/claude-lane
# Same command with native-luna-medium,aether-single-luna-medium and
# target/hb395-s2/codex-lane for the Luna lane, started at the same time.
python3 evals/harness_bench_report.py target/hb395-s2/*-lane/harness-bench-*.json \
  --output-dir target/hb395-s2/report
python3 evals/harness_bench_compare.py target/hb395-s2/*-lane/harness-bench-*.json --trials
```

The runner checks that each daemon socket path is shorter than 100 bytes. From
a deeply nested checkout, run it through a short symlink to the repository,
and invoke it by that absolute path. The report tool writes `summary.md` (by
condition and by case), `summary.json`, `results.csv` and `index.html`; the
compare tool prints each pair and, with `--trials`, each trial with its
test-by-test grade. The manifests and per-trial artifacts are ignored local
files under `target/hb395-s2/`. The statistics come from a script kept with
those files; it draws its 10,000 resamples with seed 395, and they can be
recomputed from the "Every trial" table below up to its rounding. Live runs
spend model tokens: this matrix cost $2.42 API-equivalent over 3.5 hours of
solve time, in 80 minutes of wall time.

## Every trial

The Trial column is the runner's id of the trial; Block is the block of its
lane in which it ran, beside its partner.

| Model | Harness | Case | Run | Trial | Block | Checks | Workflow | Uncached input | Output | Cache reads | Cache writes | Total tokens | API-equiv. USD | Wall s |
| --- | --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Native CLI | `billing` | 1 | `eb8f6b8b6e` | 3 | 78/79 | completed | 42 | 54,449 | 1,417,602 | 80,514 | 1,552,607 | $0.0575 | 227 |
| Claude Haiku 5.5 | Aether | `billing` | 1 | `13791f3dbe` | 3 | 78/79 | completed | 24 | 44,637 | 647,834 | 66,211 | 758,706 | $0.0420 | 179 |
| Claude Haiku 5.5 | Native CLI | `billing` | 2 | `e9ba5fdfe4` | 8 | 78/79 | completed | 30 | 48,975 | 828,461 | 71,474 | 948,940 | $0.0471 | 196 |
| Claude Haiku 5.5 | Aether | `billing` | 2 | `6eac871e48` | 8 | 78/79 | completed | 26 | 43,286 | 654,422 | 66,031 | 763,765 | $0.0414 | 181 |
| Claude Haiku 5.5 | Native CLI | `billing` | 3 | `19e0f31078` | 13 | 78/79 | completed | 40 | 48,933 | 1,216,561 | 72,658 | 1,338,192 | $0.0512 | 198 |
| Claude Haiku 5.5 | Aether | `billing` | 3 | `6342bbea0a` | 13 | 78/79 | completed | 18 | 44,203 | 459,665 | 66,416 | 570,302 | $0.0400 | 171 |
| Claude Haiku 5.5 | Native CLI | `concurrent_v2` | 1 | `d7c66da397` | 2 | 67/67 | completed | 32 | 61,095 | 1,098,270 | 82,977 | 1,242,374 | $0.0581 | 267 |
| Claude Haiku 5.5 | Aether | `concurrent_v2` | 1 | `b6d51af7ac` | 2 | 66/67 | completed | 32 | 53,343 | 1,006,380 | 75,104 | 1,134,859 | $0.0518 | 229 |
| Claude Haiku 5.5 | Native CLI | `concurrent_v2` | 2 | `9a7bee2f20` | 7 | 66/67 | completed | 32 | 51,969 | 977,520 | 72,969 | 1,102,490 | $0.0504 | 218 |
| Claude Haiku 5.5 | Aether | `concurrent_v2` | 2 | `509def375b` | 7 | 66/67 | completed | 44 | 57,778 | 1,514,671 | 79,489 | 1,651,982 | $0.0599 | 248 |
| Claude Haiku 5.5 | Native CLI | `concurrent_v2` | 3 | `a0bfc97d69` | 18 | 66/67 | completed | 34 | 60,628 | 1,141,211 | 82,437 | 1,284,310 | $0.0582 | 263 |
| Claude Haiku 5.5 | Aether | `concurrent_v2` | 3 | `2d19536ea9` | 18 | 67/67 | completed | 26 | 46,835 | 689,588 | 66,081 | 802,530 | $0.0435 | 198 |
| Claude Haiku 5.5 | Native CLI | `eventlog` | 1 | `a7720c6b94` | 1 | 49/49 | completed | 30 | 42,990 | 801,274 | 62,847 | 907,141 | $0.0421 | 187 |
| Claude Haiku 5.5 | Aether | `eventlog` | 1 | `723432e855` | 1 | 49/49 | completed | 14 | 35,862 | 254,949 | 62,577 | 353,402 | $0.0330 | 151 |
| Claude Haiku 5.5 | Native CLI | `eventlog` | 2 | `bcc9b96a65` | 12 | 49/49 | completed | 20 | 36,319 | 458,070 | 54,148 | 548,557 | $0.0336 | 149 |
| Claude Haiku 5.5 | Aether | `eventlog` | 2 | `15d8942590` | 12 | 49/49 | completed | 14 | 35,380 | 266,318 | 52,784 | 354,496 | $0.0309 | 145 |
| Claude Haiku 5.5 | Native CLI | `eventlog` | 3 | `d7c21b2044` | 17 | 49/49 | completed | 22 | 39,136 | 532,783 | 57,462 | 629,403 | $0.0364 | 167 |
| Claude Haiku 5.5 | Aether | `eventlog` | 3 | `c8b3f9efcb` | 17 | 49/49 | completed | 14 | 28,558 | 244,503 | 45,045 | 318,120 | $0.0257 | 119 |
| Claude Haiku 5.5 | Native CLI | `leases` | 1 | `9a9ad9bc1b` | 5 | 28/28 | completed | 34 | 42,397 | 896,902 | 60,568 | 999,901 | $0.0423 | 196 |
| Claude Haiku 5.5 | Aether | `leases` | 1 | `9c685b8d94` | 5 | 28/28 | completed | 30 | 37,083 | 713,216 | 58,305 | 808,634 | $0.0373 | 226 |
| Claude Haiku 5.5 | Native CLI | `leases` | 2 | `56850e6f6b` | 10 | 28/28 | completed | 32 | 52,949 | 950,346 | 71,709 | 1,075,036 | $0.0503 | 239 |
| Claude Haiku 5.5 | Aether | `leases` | 2 | `859914bdef` | 10 | 28/28 | completed | 22 | 43,501 | 558,824 | 63,361 | 665,708 | $0.0400 | 178 |
| Claude Haiku 5.5 | Native CLI | `leases` | 3 | `872c8b1f88` | 15 | 28/28 | completed | 36 | 46,606 | 1,051,275 | 65,057 | 1,162,974 | $0.0468 | 212 |
| Claude Haiku 5.5 | Aether | `leases` | 3 | `2d48c4e78e` | 15 | 28/28 | completed | 28 | 43,281 | 760,356 | 63,516 | 867,181 | $0.0420 | 180 |
| Claude Haiku 5.5 | Native CLI | `minilang` | 1 | `febe6e4e1d` | 6 | 102/102 | completed | 46 | 73,887 | 1,953,371 | 111,903 | 2,139,207 | $0.1866 | 338 |
| Claude Haiku 5.5 | Aether | `minilang` | 1 | `37eccf7c71` | 6 | 102/102 | completed | 34 | 51,064 | 1,126,418 | 79,917 | 1,257,433 | $0.0528 | 442 |
| Claude Haiku 5.5 | Native CLI | `minilang` | 2 | `4a5f16c358` | 11 | 102/102 | completed | 34 | 64,346 | 1,220,317 | 90,758 | 1,375,455 | $0.0790 | 277 |
| Claude Haiku 5.5 | Aether | `minilang` | 2 | `7812248082` | 11 | 102/102 | completed | 28 | 59,662 | 973,964 | 87,868 | 1,121,522 | $0.0571 | 321 |
| Claude Haiku 5.5 | Native CLI | `minilang` | 3 | `0a7285866b` | 16 | 102/102 | completed | 48 | 183,332 | 1,633,713 | 85,354 | 1,902,447 | $0.1251 | 601 |
| Claude Haiku 5.5 | Aether | `minilang` | 3 | `da8fd0db84` | 16 | 102/102 | completed | 38 | 62,341 | 1,422,215 | 92,317 | 1,576,911 | $0.0920 | 268 |
| Claude Haiku 5.5 | Native CLI | `semver` | 1 | `e00b86d2be` | 4 | 80/80 | completed | 32 | 62,062 | 1,087,699 | 84,059 | 1,233,852 | $0.0587 | 258 |
| Claude Haiku 5.5 | Aether | `semver` | 1 | `2384a87d3b` | 4 | 80/80 | completed | 28 | 62,162 | 958,533 | 84,473 | 1,105,196 | $0.0576 | 256 |
| Claude Haiku 5.5 | Native CLI | `semver` | 2 | `97d13e24e6` | 9 | 80/80 | completed | 48 | 74,213 | 1,982,351 | 100,714 | 2,157,326 | $0.1393 | 313 |
| Claude Haiku 5.5 | Aether | `semver` | 2 | `68cf8830ad` | 9 | 80/80 | completed | 38 | 58,935 | 1,290,550 | 81,214 | 1,430,737 | $0.0586 | 248 |
| Claude Haiku 5.5 | Native CLI | `semver` | 3 | `5d124086e7` | 14 | 80/80 | completed | 32 | 62,117 | 1,087,848 | 84,035 | 1,234,032 | $0.0587 | 257 |
| Claude Haiku 5.5 | Aether | `semver` | 3 | `930978894f` | 14 | 79/80 | completed | 30 | 59,244 | 1,005,113 | 82,513 | 1,146,900 | $0.0562 | 245 |
| GPT-6 Luna | Native CLI | `billing` | 1 | `a1a4c9f225` | 3 | 75/79 | completed | 36,842 | 7,968 | 487,168 | 0 | 531,978 | $0.0125 | 90 |
| GPT-6 Luna | Aether | `billing` | 1 | `744453503a` | 3 | 77/79 | completed | 34,873 | 7,290 | 393,728 | 0 | 435,891 | $0.0111 | 77 |
| GPT-6 Luna | Native CLI | `billing` | 2 | `550ad5a669` | 8 | 53/79 | completed | 32,585 | 6,152 | 268,800 | 0 | 307,537 | $0.0090 | 70 |
| GPT-6 Luna | Aether | `billing` | 2 | `0ae109023c` | 8 | 69/79 | completed | 49,076 | 5,848 | 228,352 | 0 | 283,276 | $0.0101 | 58 |
| GPT-6 Luna | Native CLI | `billing` | 3 | `e5a9768bdb` | 13 | 78/79 | completed | 24,017 | 6,315 | 320,768 | 0 | 351,100 | $0.0088 | 102 |
| GPT-6 Luna | Aether | `billing` | 3 | `36ce9ada12` | 13 | 77/79 | completed | 32,841 | 6,903 | 295,424 | 0 | 335,168 | $0.0097 | 150 |
| GPT-6 Luna | Native CLI | `concurrent_v2` | 1 | `8fee4eb6b2` | 2 | 66/67 | completed | 31,838 | 6,351 | 308,224 | 0 | 346,413 | $0.0094 | 74 |
| GPT-6 Luna | Aether | `concurrent_v2` | 1 | `54270a7a93` | 2 | 67/67 | completed | 26,412 | 7,401 | 247,808 | 0 | 281,621 | $0.0088 | 74 |
| GPT-6 Luna | Native CLI | `concurrent_v2` | 2 | `03b532bac8` | 7 | 66/67 | completed | 33,395 | 6,281 | 256,768 | 0 | 296,444 | $0.0090 | 136 |
| GPT-6 Luna | Aether | `concurrent_v2` | 2 | `44c15b8505` | 7 | 66/67 | completed | 32,635 | 6,094 | 324,608 | 0 | 363,337 | $0.0096 | 67 |
| GPT-6 Luna | Native CLI | `concurrent_v2` | 3 | `d2ca91304e` | 18 | 66/67 | completed | 16,079 | 5,134 | 102,912 | 0 | 124,125 | $0.0052 | 53 |
| GPT-6 Luna | Aether | `concurrent_v2` | 3 | `87d54754f7` | 18 | 66/67 | completed | 29,409 | 7,636 | 248,832 | 0 | 285,877 | $0.0092 | 165 |
| GPT-6 Luna | Native CLI | `eventlog` | 1 | `5b402a5675` | 1 | 47/49 | completed | 22,781 | 4,102 | 194,816 | 0 | 221,699 | $0.0063 | 52 |
| GPT-6 Luna | Aether | `eventlog` | 1 | `573de8cab7` | 1 | 46/49 | completed | 38,443 | 4,169 | 161,280 | 0 | 203,892 | $0.0075 | 95 |
| GPT-6 Luna | Native CLI | `eventlog` | 2 | `8c2df683b1` | 12 | 48/49 | completed | 19,121 | 6,037 | 221,184 | 0 | 246,342 | $0.0071 | 198 |
| GPT-6 Luna | Aether | `eventlog` | 2 | `5fa00a104f` | 12 | 48/49 | completed | 31,913 | 4,749 | 177,664 | 0 | 214,326 | $0.0073 | 53 |
| GPT-6 Luna | Native CLI | `eventlog` | 3 | `87825ae20c` | 17 | 47/49 | completed | 17,057 | 4,080 | 139,264 | 0 | 160,401 | $0.0051 | 44 |
| GPT-6 Luna | Aether | `eventlog` | 3 | `e6d17f6f5c` | 17 | 48/49 | completed | 13,366 | 3,869 | 161,536 | 0 | 178,771 | $0.0049 | 86 |
| GPT-6 Luna | Native CLI | `leases` | 1 | `d86f0eead3` | 5 | 23/28 | completed | 14,920 | 3,700 | 114,944 | 0 | 133,564 | $0.0045 | 43 |
| GPT-6 Luna | Aether | `leases` | 1 | `0131bdb879` | 5 | 27/28 | completed | 32,538 | 3,839 | 83,456 | 0 | 119,833 | $0.0060 | 107 |
| GPT-6 Luna | Native CLI | `leases` | 2 | `c4a9e839c3` | 10 | 28/28 | completed | 31,400 | 5,339 | 191,744 | 0 | 228,483 | $0.0077 | 61 |
| GPT-6 Luna | Aether | `leases` | 2 | `1502e6d116` | 10 | 27/28 | completed | 24,084 | 5,224 | 165,632 | 0 | 194,940 | $0.0067 | 66 |
| GPT-6 Luna | Native CLI | `leases` | 3 | `841940d22e` | 15 | 28/28 | completed | 29,341 | 4,775 | 170,496 | 0 | 204,612 | $0.0070 | 54 |
| GPT-6 Luna | Aether | `leases` | 3 | `c908eb26fd` | 15 | 27/28 | completed | 29,632 | 4,838 | 160,512 | 0 | 194,982 | $0.0070 | 52 |
| GPT-6 Luna | Native CLI | `minilang` | 1 | `f3f48487df` | 6 | 89/102 | completed | 40,777 | 5,135 | 295,424 | 0 | 341,336 | $0.0096 | 58 |
| GPT-6 Luna | Aether | `minilang` | 1 | `e3ffe1a772` | 6 | 100/102 | timeout | 67,447 | 8,316 | 763,648 | 0 | 839,411 | $0.0185 | 900 |
| GPT-6 Luna | Native CLI | `minilang` | 2 | `5ead5a82c4` | 11 | 101/102 | completed | 39,606 | 9,146 | 590,336 | 0 | 639,088 | $0.0144 | 96 |
| GPT-6 Luna | Aether | `minilang` | 2 | `f0be7751d1` | 11 | 92/102 | completed | 51,651 | 6,867 | 541,440 | 0 | 599,958 | $0.0140 | 156 |
| GPT-6 Luna | Native CLI | `minilang` | 3 | `274f4b2efb` | 16 | 101/102 | completed | 31,502 | 6,849 | 476,160 | 0 | 514,511 | $0.0113 | 78 |
| GPT-6 Luna | Aether | `minilang` | 3 | `b1177216db` | 16 | 93/102 | completed | 40,261 | 9,845 | 407,808 | 0 | 457,914 | $0.0130 | 96 |
| GPT-6 Luna | Native CLI | `semver` | 1 | `c49f8cdfdc` | 4 | 72/80 | completed | 29,040 | 7,485 | 418,048 | 0 | 454,573 | $0.0108 | 177 |
| GPT-6 Luna | Aether | `semver` | 1 | `27de2b61b4` | 4 | 79/80 | completed | 20,440 | 6,203 | 258,048 | 0 | 284,691 | $0.0077 | 64 |
| GPT-6 Luna | Native CLI | `semver` | 2 | `564cf6e406` | 9 | 67/80 | completed | 32,376 | 7,466 | 387,840 | 0 | 427,682 | $0.0108 | 84 |
| GPT-6 Luna | Aether | `semver` | 2 | `7d1587e9b3` | 9 | 65/80 | completed | 45,832 | 7,432 | 311,808 | 0 | 365,072 | $0.0114 | 159 |
| GPT-6 Luna | Native CLI | `semver` | 3 | `678e7af6a4` | 14 | 79/80 | completed | 24,025 | 7,084 | 320,768 | 0 | 351,877 | $0.0092 | 76 |
| GPT-6 Luna | Aether | `semver` | 3 | `b013491656` | 14 | 79/80 | completed | 51,911 | 7,921 | 429,312 | 0 | 489,144 | $0.0134 | 92 |

## Every pair's hidden checks

For checking the pair counts by hand: the native and the Aether trial of the
same case and run index.

| Model | Case | Run | Checks | Native | Aether | Aether minus native | Aether is |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | `billing` | 1 | 79 | 78 | 78 | +0 | equal |
| Claude Haiku 5.5 | `billing` | 2 | 79 | 78 | 78 | +0 | equal |
| Claude Haiku 5.5 | `billing` | 3 | 79 | 78 | 78 | +0 | equal |
| Claude Haiku 5.5 | `concurrent_v2` | 1 | 67 | 67 | 66 | -1 | lower |
| Claude Haiku 5.5 | `concurrent_v2` | 2 | 67 | 66 | 66 | +0 | equal |
| Claude Haiku 5.5 | `concurrent_v2` | 3 | 67 | 66 | 67 | +1 | higher |
| Claude Haiku 5.5 | `eventlog` | 1 | 49 | 49 | 49 | +0 | equal |
| Claude Haiku 5.5 | `eventlog` | 2 | 49 | 49 | 49 | +0 | equal |
| Claude Haiku 5.5 | `eventlog` | 3 | 49 | 49 | 49 | +0 | equal |
| Claude Haiku 5.5 | `leases` | 1 | 28 | 28 | 28 | +0 | equal |
| Claude Haiku 5.5 | `leases` | 2 | 28 | 28 | 28 | +0 | equal |
| Claude Haiku 5.5 | `leases` | 3 | 28 | 28 | 28 | +0 | equal |
| Claude Haiku 5.5 | `minilang` | 1 | 102 | 102 | 102 | +0 | equal |
| Claude Haiku 5.5 | `minilang` | 2 | 102 | 102 | 102 | +0 | equal |
| Claude Haiku 5.5 | `minilang` | 3 | 102 | 102 | 102 | +0 | equal |
| Claude Haiku 5.5 | `semver` | 1 | 80 | 80 | 80 | +0 | equal |
| Claude Haiku 5.5 | `semver` | 2 | 80 | 80 | 80 | +0 | equal |
| Claude Haiku 5.5 | `semver` | 3 | 80 | 80 | 79 | -1 | lower |
| GPT-6 Luna | `billing` | 1 | 79 | 75 | 77 | +2 | higher |
| GPT-6 Luna | `billing` | 2 | 79 | 53 | 69 | +16 | higher |
| GPT-6 Luna | `billing` | 3 | 79 | 78 | 77 | -1 | lower |
| GPT-6 Luna | `concurrent_v2` | 1 | 67 | 66 | 67 | +1 | higher |
| GPT-6 Luna | `concurrent_v2` | 2 | 67 | 66 | 66 | +0 | equal |
| GPT-6 Luna | `concurrent_v2` | 3 | 67 | 66 | 66 | +0 | equal |
| GPT-6 Luna | `eventlog` | 1 | 49 | 47 | 46 | -1 | lower |
| GPT-6 Luna | `eventlog` | 2 | 49 | 48 | 48 | +0 | equal |
| GPT-6 Luna | `eventlog` | 3 | 49 | 47 | 48 | +1 | higher |
| GPT-6 Luna | `leases` | 1 | 28 | 23 | 27 | +4 | higher |
| GPT-6 Luna | `leases` | 2 | 28 | 28 | 27 | -1 | lower |
| GPT-6 Luna | `leases` | 3 | 28 | 28 | 27 | -1 | lower |
| GPT-6 Luna | `minilang` | 1 | 102 | 89 | 100 | +11 | higher |
| GPT-6 Luna | `minilang` | 2 | 102 | 101 | 92 | -9 | lower |
| GPT-6 Luna | `minilang` | 3 | 102 | 101 | 93 | -8 | lower |
| GPT-6 Luna | `semver` | 1 | 80 | 72 | 79 | +7 | higher |
| GPT-6 Luna | `semver` | 2 | 80 | 67 | 65 | -2 | lower |
| GPT-6 Luna | `semver` | 3 | 80 | 79 | 79 | +0 | equal |
