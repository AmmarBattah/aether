# Lower-tier harness comparison, the counted test again: Claude Haiku 5.5 and GPT-6 Luna, 2026-10-11

Does running a cheap model through Aether cost accuracy, tokens, money or time
against running it in its native harness? This report gives five more counted
runs of the matched comparison of the
[second measurement](harness-lower-tier-results-2026-10-10-protocol-2.md),
40 trials, under the same protocol version 2, on the same Aether binary,
with the same settings, and it puts the two sittings together: 10 runs,
80 trials. The [first measurement](harness-lower-tier-results-2026-10-10.md),
under protocol version 1, stands as it was written and is in no figure here.
The number of runs, the rule for counting them, the rule for which table the
README shows and the statistics were committed before the first run started
([the plan](#the-plan-committed-before-the-first-run)); no run was added,
dropped or repeated. It is a small sample from one machine. The intervals and
p-values below are descriptive, and no significance is claimed.

## Summary

### Runs 6 to 10, this sitting

Means per trial. Each row is 10 trials: 5 runs of 2 cases.

| Model, requested effort | Harness | Workflows finished | Hidden checks passed | Uncached input tokens | Output tokens | Cache read tokens | API-equivalent USD | Wall seconds |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5, low | Claude Code | 10/10 | 99.6 % | 598 | 121,513 | 2,315,255 | 0.3346 | 520.5 |
| Claude Haiku 5.5, low | Aether | 10/10 | 99.5 % | 644 | 120,912 | 2,429,522 | 0.3515 | 509.4 |
| | Aether minus native, 10 pairs | | -0.1 pp | +46 | -601 | +114,267 | +0.0169 | -11.0 |
| GPT-6 Luna, low | Codex | 10/10 | 70.1 % | 52,583 | 13,901 | 889,856 | 0.0211 | 168.0 |
| GPT-6 Luna, low | Aether | 10/10 | 77.2 % | 74,892 | 19,086 | 1,755,648 | 0.0346 | 270.9 |
| | Aether minus native, 10 pairs | | +7.1 pp | +22,309 | +5,185 | +865,792 | +0.0135 | +102.9 |

Hidden checks passed per case, mean of 5 runs with every run's count, in run
order 6 to 10:

| Model | Harness | `matchbook` (119 checks) | `gridcalc` (154 checks) |
| --- | --- | --- | --- |
| Claude Haiku 5.5 | Claude Code | 99.3 %: 117, 117, 119, 119, 119 | 99.9 %: 154, 154, 154, 153, 154 |
| Claude Haiku 5.5 | Aether | 99.0 %: 119, 119, 117, 117, 117 | 100.0 %: 154, 154, 154, 154, 154 |
| GPT-6 Luna | Codex | 57.5 %: 0, 114, 117, 0, 111 | 82.7 %: 138, 117, 110, 136, 136 |
| GPT-6 Luna | Aether | 71.8 %: 89, 107, 114, 117, 0 | 82.7 %: 140, 120, 126, 118, 133 |

A pair is one case in one run, both sides started in the same block. "Lower"
means the Aether trial's figure is below its native partner's. The interval
is the 95 % paired-bootstrap interval of the mean pair difference and p the
two-sided sign-flip p-value, both as [planned](#statistics).

- **Where Aether is behind with Haiku, in the mean:** hidden checks passed
  99.5 % against 99.6 % (-0.1 pp; lower in 3 of 10 pairs, higher in 3, equal
  in 4; 95 % interval -0.8 pp to +0.6 pp, p 1.000); uncached input tokens 644
  against 598 (+46, +7.7 %; lower in 5 of 10 pairs, higher in 5, equal in 0;
  95 % interval -786 to +927, p 0.824); cache read tokens 2,429,522 against
  2,315,255 (+114,267, +4.9 %; lower in 5 of 10 pairs, higher in 5, equal in
  0; 95 % interval -595,743 to +905,388, p 0.834); API-equivalent USD 0.3515
  against 0.3346 (+0.0169, +5.0 %; lower in 5 of 10 pairs, higher in 5, equal
  in 0; 95 % interval -0.0530 to +0.0855, p 0.656).
- **Where Aether is ahead with Haiku, in the mean:** output tokens 120,912
  against 121,513 (-601, -0.5 %; lower in 5 of 10 pairs, higher in 5, equal in
  0; 95 % interval -13,065 to +10,123, p 0.930); wall seconds 509.4 against
  520.5 (-11.0, -2.1 %; lower in 5 of 10 pairs, higher in 5, equal in 0; 95 %
  interval -84.4 to +47.7, p 0.807).
- **Where Aether is behind with Luna, in the mean:** uncached input tokens
  74,892 against 52,583 (+22,309, +42.4 %; lower in 4 of 10 pairs, higher in
  6, equal in 0; 95 % interval -859 to +52,699, p 0.141); output tokens 19,086
  against 13,901 (+5,185, +37.3 %; lower in 3 of 10 pairs, higher in 7, equal
  in 0; 95 % interval +399 to +10,860, p 0.078); cache read tokens 1,755,648
  against 889,856 (+865,792, +97.3 %; lower in 5 of 10 pairs, higher in 5,
  equal in 0; 95 % interval -220,211 to +2,555,110, p 0.367); API-equivalent
  USD 0.0346 against 0.0211 (+0.0135, +63.9 %; lower in 3 of 10 pairs, higher
  in 7, equal in 0; 95 % interval -0.0013 to +0.0360, p 0.227); wall seconds
  270.9 against 168.0 (+102.9, +61.2 %; lower in 3 of 10 pairs, higher in 7,
  equal in 0; 95 % interval -21.5 to +275.8, p 0.236).
- **Where Aether is ahead with Luna, in the mean:** hidden checks passed 77.2
  % against 70.1 % (+7.1 pp; lower in 5 of 10 pairs, higher in 5, equal in 0;
  95 % interval -21.6 pp to +37.6 pp, p 0.791).

### All ten runs under protocol version 2, two sittings

Means per trial. Each row is 20 trials: runs 1 to 5 of the second
measurement (2026-10-10) and runs 6 to 10 of this sitting (2026-10-11). The
same binary, command lines, prompts, cases and CLI versions in both
([the checks](#is-it-the-same-test-as-the-second-measurement)); a different
day.

| Model, requested effort | Harness | Workflows finished | Hidden checks passed | Uncached input tokens | Output tokens | Cache read tokens | API-equivalent USD | Wall seconds |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5, low | Claude Code | 20/20 | 99.8 % | 426 | 120,865 | 2,476,166 | 0.3428 | 528.3 |
| Claude Haiku 5.5, low | Aether | 20/20 | 99.6 % | 337 | 116,659 | 2,045,135 | 0.3162 | 490.2 |
| | Aether minus native, 20 pairs | | -0.2 pp | -89 | -4,205 | -431,030 | -0.0266 | -38.1 |
| GPT-6 Luna, low | Codex | 20/20 | 73.4 % | 51,775 | 14,061 | 983,590 | 0.0220 | 202.6 |
| GPT-6 Luna, low | Aether | 20/20 | 76.6 % | 74,106 | 18,948 | 1,786,816 | 0.0348 | 249.2 |
| | Aether minus native, 20 pairs | | +3.2 pp | +22,330 | +4,887 | +803,226 | +0.0127 | +46.6 |

- **Where Aether is behind with Haiku, in the mean:** hidden checks passed
  99.6 % against 99.8 % (-0.2 pp; lower in 5 of 20 pairs, higher in 3, equal
  in 12; 95 % interval -0.7 pp to +0.2 pp, p 0.453).
- **Where Aether is ahead with Haiku, in the mean:** uncached input tokens 337
  against 426 (-89, -20.9 %; lower in 13 of 20 pairs, higher in 7, equal in 0;
  95 % interval -506 to +352, p 0.759); output tokens 116,659 against 120,865
  (-4,205, -3.5 %; lower in 10 of 20 pairs, higher in 10, equal in 0; 95 %
  interval -13,442 to +4,333, p 0.392); cache read tokens 2,045,135 against
  2,476,166 (-431,030, -17.4 %; lower in 13 of 20 pairs, higher in 7, equal in
  0; 95 % interval -1,033,039 to +157,542, p 0.184); API-equivalent USD 0.3162
  against 0.3428 (-0.0266, -7.8 %; lower in 11 of 20 pairs, higher in 9, equal
  in 0; 95 % interval -0.0868 to +0.0305, p 0.400); wall seconds 490.2 against
  528.3 (-38.1, -7.2 %; lower in 11 of 20 pairs, higher in 9, equal in 0; 95 %
  interval -95.2 to +10.5, p 0.204).
- **Where Aether is behind with Luna, in the mean:** uncached input tokens
  74,106 against 51,775 (+22,330, +43.1 %; lower in 6 of 20 pairs, higher in
  14, equal in 0; 95 % interval +5,266 to +41,869, p 0.021); output tokens
  18,948 against 14,061 (+4,887, +34.8 %; lower in 6 of 20 pairs, higher in
  14, equal in 0; 95 % interval +1,451 to +8,458, p 0.013); cache read tokens
  1,786,816 against 983,590 (+803,226, +81.7 %; lower in 9 of 20 pairs, higher
  in 11, equal in 0; 95 % interval -5,210 to +1,801,229, p 0.096);
  API-equivalent USD 0.0348 against 0.0220 (+0.0127, +57.7 %; lower in 6 of 20
  pairs, higher in 14, equal in 0; 95 % interval +0.0015 to +0.0263, p 0.047);
  wall seconds 249.2 against 202.6 (+46.6, +23.0 %; lower in 9 of 20 pairs,
  higher in 11, equal in 0; 95 % interval -32.6 to +146.7, p 0.399).
- **Where Aether is ahead with Luna, in the mean:** hidden checks passed 76.6
  % against 73.4 % (+3.2 pp; lower in 11 of 20 pairs, higher in 9, equal in 0;
  95 % interval -16.1 pp to +23.0 pp, p 0.841).

The two sittings are set side by side, with the interval and p-value of each
measure over all 10 runs, under
[All ten runs under protocol version 2](#all-ten-runs-under-protocol-version-2).

### What these numbers show, and what they do not

- **Luna: tokens, cost and time.** Over all ten runs, Aether minus Codex in
  the mean: Aether is behind on uncached input tokens +22,330, +43.1 % of the
  native mean (95 % interval +5,266 to +41,869; lower in 6 of 20 pairs, higher
  in 14; p 0.021); output tokens +4,887, +34.8 % of the native mean (95 %
  interval +1,451 to +8,458; lower in 6 of 20 pairs, higher in 14; p 0.013);
  cache read tokens +803,226, +81.7 % of the native mean (95 % interval -5,210
  to +1,801,229; lower in 9 of 20 pairs, higher in 11; p 0.096);
  API-equivalent USD +0.0127 USD, +57.7 % of the native mean (95 % interval
  +0.0015 to +0.0263 USD; lower in 6 of 20 pairs, higher in 14; p 0.047); wall
  seconds +46.6 s, +23.0 % of the native mean (95 % interval -32.6 to +146.7
  s; lower in 9 of 20 pairs, higher in 11; p 0.399). The interval excludes
  zero for uncached input tokens, output tokens and API-equivalent USD. It
  includes zero for cache read tokens and wall seconds. In runs 6 to 10 alone
  it excludes zero for output tokens.
- **Luna: hidden checks.** Over all ten runs Aether passed 76.6 % as counted
  against 73.4 % in Codex: a mean pair difference of +3.2 pp (95 % interval
  -16.1 to +23.0 pp; lower in 11 of 20 pairs, higher in 9; p 0.841). The
  counted figure holds 5 trials graded 0 because the solution hung in the
  grader, 3 in Codex and 2 through Aether; one such 0 in place of a full pass
  moves a mean of 20 trials by 5.0 points. One of the three in Codex is the
  trial of run 9 in which the memory guard ended two processes the solver had
  started. Graded test by test, which is the second figure and not the counted
  one, it is 85.5 % against 86.6 %: a mean pair difference of -1.0 pp (95 %
  interval -4.1 to +2.4 pp; lower in 12 of 20 pairs, higher in 8; p 0.547).
  Per case, over all ten runs: on `gridcalc` Aether passed 79.5 % against 81.4
  % (lower in 6 of 10 runs, higher in 4); on `matchbook`, where all five hung
  grades fell, 73.6 % against 65.4 % as counted (lower in 5 of 10, higher in
  5) and 91.5 % against 91.7 % test by test. 1 of the 40 Luna trials passed
  every check. The interval of the test-by-test figure still allows Aether to
  be 4.1 points behind or 2.4 points ahead in the mean, so these runs are not
  evidence that nothing is lost.
- **Haiku: tokens, cost and time.** Over all ten runs, Aether minus Claude
  Code in the mean: Aether is ahead on uncached input tokens -89, -20.9 % of
  the native mean (95 % interval -506 to +352; lower in 13 of 20 pairs, higher
  in 7; p 0.759); output tokens -4,205, -3.5 % of the native mean (95 %
  interval -13,442 to +4,333; lower in 10 of 20 pairs, higher in 10; p 0.392);
  cache read tokens -431,030, -17.4 % of the native mean (95 % interval
  -1,033,039 to +157,542; lower in 13 of 20 pairs, higher in 7; p 0.184);
  API-equivalent USD -0.0266 USD, -7.8 % of the native mean (95 % interval
  -0.0868 to +0.0305 USD; lower in 11 of 20 pairs, higher in 9; p 0.400); wall
  seconds -38.1 s, -7.2 % of the native mean (95 % interval -95.2 to +10.5 s;
  lower in 11 of 20 pairs, higher in 9; p 0.204). No interval of these five
  excludes zero. In runs 6 to 10 alone none of the five intervals excludes
  zero.
- **Haiku: hidden checks.** Over all ten runs Aether passed 99.6 % as counted
  against 99.8 % in Claude Code: a mean pair difference of -0.2 pp (95 %
  interval -0.7 to +0.2 pp; lower in 5 of 20 pairs, higher in 3, equal in 12;
  p 0.453). No grade hung, so the test-by-test figure equals the counted one.
  32 of the 40 Haiku trials passed every check. The interval still allows
  Aether to be 0.7 points behind or 0.2 points ahead in the mean, so these
  runs are not evidence that nothing is lost.
- **The two sittings.** The mean difference has the same sign in both on 7 of
  the 12 counted model-and-measure figures; it changes sign on Haiku's
  uncached input tokens, Haiku's cache read tokens, Haiku's API-equivalent
  USD, Luna's hidden checks passed as counted and Luna's wall seconds. The
  table under [All ten runs under protocol version
  2](#all-ten-runs-under-protocol-version-2) sets them side by side.
- **What they do not show.** Ten runs of two cases on one machine is a small
  sample, and the same cell moves by tens of checks from run to run. The
  intervals and p-values are descriptive: seven measures for each of two
  models, in three groupings, so an interval that excludes zero is not by
  itself a finding, and no significance is claimed. Nothing here says why a
  figure differs; the [diagnosis](harness-overhead-diagnosis-2026-10-10.md)
  looked for causes with trials of its own, none of which is counted here.

## The plan, committed before the first run

### What is run

Five complete matched runs, seeds 6 to 10, one runner invocation per run,
launched one after another:

```sh
python3 evals/harness_bench.py --cases matchbook,gridcalc --runs 1 --seed <n> --jobs 8 --timeout 1200 \
  --conditions native-haiku-low,aether-single-haiku-low,native-luna-low,aether-single-luna-low \
  --binary target/release/aether --output-dir target/harness-393-v2b/run-<n>
```

Each run is 8 trials: two cases (`matchbook`, `gridcalc`), two models (Claude
Haiku 5.5 and GPT-6 Luna, both at requested effort low), each model once in
its native CLI and once through one Aether agent. The settings are those of
the second measurement; only the seeds and the output directory differ.

### Counting rule

Rules 1 to 6 are the rules of the
[first measurement](harness-lower-tier-results-2026-10-10.md), as the second
report quotes them:

1. A counted run is one runner invocation, `--runs 1 --seed n`: the complete
   matched set of 8 trials (two cases, four cells), with the same `--jobs` and
   `--timeout` in every run.
2. A run counts when its manifest holds all 8 trial rows. Every one of its
   trials then enters the tables as it stands. No trial is rerun or dropped.
3. A trial that reaches the timeout is graded on the files it left and stays
   in the tables with its workflow shown as not finished.
4. A trial lost to an environment fault (a provider rate or usage limit, the
   machine out of memory, a daemon that did not start) stays in the tables as
   a failed trial of a kept run, graded on what it left, and is listed with
   its evidence. The run is not left out because of it.
5. A run is left out whole only when the runner did not write all 8 rows, for
   example because it had to be stopped for the clock. Its partial rows are
   not counted, and this report says so.
6. The smoke and the calibration pilot are never counted.

Added for this measurement:

7. Runs 6 to 10 of protocol version 2 are counted whole, whatever they show.
   No run is added, dropped or repeated on the strength of a result. A trial
   ended by the memory guard is counted as it stands and listed.
8. Five runs are planned, with seeds 6 to 10. The number is fixed here, before
   the first run, and by the clock alone: a run is started only before 10:06
   UTC on 2026-10-11, minute 88 of the sitting. If that limit keeps the fifth
   run from starting, four runs are counted and this report says so. No result
   is looked at to decide it.
9. A run whose manifest has been saved is never made again.
10. Neither counted case is smoked or piloted before run 6.
11. A counted trial graded 0 because the grader ran out of time keeps its 0
    in every counted figure. Its test-by-test grade is reported beside the
    counted figure and never in place of it.

Separately, the report tool keeps its own frozen rule: a trial the runner
classifies as `blocked` (the host refused a tool call and the agent idled) is
left out of the means and counted in its own row.

### Headline rule

If the checks of point 3 show the same Rust source, the same counted command
lines and prompts and the same CLI versions as the second measurement, the
README's table gives all ten version 2 runs together (20 trials per cell, 80
trials), marked as two sittings, with the new sitting's own figures beside it
in this report. If any of them differs, the README's table gives the five new
runs alone, and the ten together appear only in this report with the
difference named.

"The checks of point 3" are the four checks under
[Is it the same test as the second measurement?](#is-it-the-same-test-as-the-second-measurement)
below. Three of them can be made before a run and were. The fourth, the
command line each Aether cell starts its CLI with, is built by the Aether
binary and is recorded only while a trial runs; before the first run it is
held by the binary being byte for byte the same file. After the runs the
recorded command lines of the new trials are compared with the second
measurement's, the result is reported here, and a difference there sends the
README to the second branch of this rule.

### Statistics

For each model pair (Aether minus its native partner; a pair is one case in
one run, both sides started in the same block) and each of seven measures:

1. hidden checks passed, as counted (in percentage points of the case's
   checks; per case also in checks);
2. uncached input tokens;
3. output tokens;
4. cache read tokens;
5. API-equivalent USD;
6. wall seconds;
7. hidden checks passed, test by test (the second, uncounted grade: it
   equals the counted grade unless the grader's one process timed out).

the report gives: the two means and the two medians; the mean and the median
pair difference; the mean difference relative to the native mean; the pairs in
which Aether is lower, higher and equal; the smallest and the largest pair
difference; a 95 % interval for the mean pair difference; and a two-sided
p-value.

- **Interval.** A paired bootstrap: the pairs are drawn again with
  replacement, as whole pairs, 10,000 times; the interval runs from the 2.5th
  to the 97.5th percentile of the 10,000 means. Random seed 20261011.
- **p-value.** The sign-flip permutation test of the mean pair difference,
  two-sided, exact: every assignment of signs to the pair differences is
  enumerated (1,024 for 10 pairs, 1,048,576 for 20), and p is the share whose
  absolute mean is at least the observed one.
- **Three groupings.** The five new runs (10 pairs per model), the second
  measurement (10 pairs per model, from its saved manifests), and all ten
  version 2 runs (20 pairs per model). Hidden checks per case too (5, 5 and
  10 pairs).
- **Test by test, for the second measurement.** Its manifests hold no
  test-by-test grade. For its two trials graded 0 by a hung grader the
  per-test output saved with that measurement is used (107 and 110 of 119);
  for every other trial of it the counted grade is used.
- A pair with an unknown figure on either side is left out of that measure
  and counted as missing.

These are descriptive. Seven measures are looked at for each of two models, so
one interval among them that excludes zero is not by itself a finding. No
significance is claimed on five runs; over ten runs an interval is reported as
it falls.

Counted and diagnostic trials are never merged into one figure.

### Host rules

- Before each run: 4 GB or more of memory available and 15 GB or more free on
  the home file system; both are recorded with the load average. If memory is
  short the run waits; `--jobs` is not lowered.
- A memory guard runs beside every run. It ends a process started inside a
  trial workspace of this checkout once that process holds more than 4 GB,
  and logs it with the trial id. It leaves the grader alone and touches
  nothing outside this checkout's trial workspaces.
- Each run's manifest is copied out of the working tree as soon as the run
  ends, before the next starts.

## Is it the same test as the second measurement?

Checked before the first run, at commit `f967288b` (the head the binary was
built from) against `2a7c3023`, the commit every manifest of the second
measurement records:

| Check | How | Result |
| --- | --- | --- |
| Rust source | `git diff --quiet 2a7c3023 HEAD -- crates`, and the same for `clients`, `Cargo.toml`, `Cargo.lock` | no difference |
| Aether binary | `sha256sum target/release/aether` after `cargo build --release` at `f967288b` | `251c0b08f359c8a679a76831dea1d892b0c2a0a2c34e51427323c8b25d4d4145`, the digest the second measurement's manifests record; `aether 0.1.1` |
| Fixtures | `git diff --quiet 2a7c3023 HEAD -- evals/fixtures`; the two case digests | no difference; `matchbook` `7f719f2d3e0b7c98228f9bdf9a922aea93b605cbec9cb3b46535fe66ab8d630a`, `gridcalc` `df35116f6492541fbdc8078c26355647483ff321332b91bb7ab3c9c975dd4758`, as frozen |
| Counted command lines and prompts | for each of the 40 trials of the second measurement's manifests: its recorded treatment, its recorded task text and (native cells) its recorded command line against what the current code builds for that cell and case | 40 trials compared, 0 differ |
| The benchmark code that starts a trial | `git diff --quiet 2a7c3023 HEAD` on `evals/harness_bench_config.py`, `harness_bench_receipts.py`, `harness_bench_process.py`, `harness_bench_wait.py`, `harness_bench_usage.py`, `harness_bench_pricing.py`, `harness_bench_sessions.py`, `harness_bench_blocks.py`, `aether_client.py` | no difference |
| CLI versions | `claude --version`, `codex --version`, and the digest of each executable | `2.1.294 (Claude Code)` and `codex-cli 0.162.1`; digests `27122ca7…f262` and `628e3548…92e1`, as the second measurement's manifests record |

The command lines the current code builds for the two native cells:

```text
claude -p --output-format stream-json --verbose --dangerously-skip-permissions --strict-mcp-config --model claude-haiku-5-5 --effort low <task text>
codex exec --json --dangerously-bypass-approvals-and-sandbox -c mcp_servers={} --model gpt-6-luna -c model_reasoning_effort="low" <task text>
```

The two Aether cells create one agent on `claude-code/claude-haiku-5-5` and
on `codex/gpt-6-luna` at effort low with the same task text; the Aether binary
builds the CLI command line.

What is not the same, known before the first run:

- **The benchmark code is later.** Since `2a7c3023` the runner gained
  diagnostic arms (no counted cell uses one) and a second, uncounted field per
  trial, `grade_by_test`. When the counted grader's one process times out, the
  runner now runs each hidden test in a process of its own afterwards. A
  trial's wall seconds end before grading, so this adds nothing to that
  trial's figures; it can add load to trials of the same block that are still
  running. In the second measurement the same per-test look was made after
  the sitting.
- **One check added to the uncounted grade** (commit `91c2409d`, made before
  the first run): a test-by-test count that is not an integer within its
  total is recorded as unknown. The counted grade is unchanged.
- **The sitting.** These runs start about eleven hours after the second
  measurement's on the next UTC day. Provider load, cache state and the other
  work on the machine are not held equal between the sittings.
- **The seeds.** 6 to 10 where the second measurement used 1 to 5. The seed
  sets the order in which a run's trials are launched.

### After the runs

The checks were made again on the 5 manifests of this sitting, which record
what each trial was started with. All 40 trials: the recorded treatment, the
recorded task text and, for the 20 native trials, the recorded command line
equal what the code builds, and through that the second measurement's; 0 of 40
differ. Every manifest records the binary digest `251c0b08…4145`, commit
`0ca5dee6` with a clean tree, `claude` 2.1.294 and `codex-cli` 0.162.1 with
the executables' digests of the second measurement. Every receipt of the 10
Aether Haiku trials records that `--strict-mcp-config` was added, as under
version 2 then.

The two native command lines as recorded, without the task text:

```text
claude -p --output-format stream-json --verbose --dangerously-skip-permissions --strict-mcp-config --model claude-haiku-5-5 --effort low
codex exec --json --dangerously-bypass-approvals-and-sandbox -c mcp_servers={} --model gpt-6-luna -c model_reasoning_effort="low"
```

What each Aether cell's CLI was started with is not in a manifest. It was read
from the session logs instead: for each of the 8 cell and case groups, the
sizes of what the session was given before its first response (system prompt,
always-loaded tools, deferred tools, first user message for Claude Code; every
block of the first turn's context for Codex) are the same in this sitting as
in the second measurement. The first request's input tokens per group:

| Cell | Case | First-request input tokens, runs 1 to 5 | Runs 6 to 10 |
| --- | --- | ---: | ---: |
| `aether-single-haiku-low` | `gridcalc` | 23288 to 23300 | 23276 to 23284 |
| `aether-single-haiku-low` | `matchbook` | 23316 to 23334 | 23309 to 23324 |
| `aether-single-luna-low` | `gridcalc` | 16524 to 16537 | 16507 to 16520 |
| `aether-single-luna-low` | `matchbook` | 16549 to 16559 | 16539 to 16548 |
| `native-haiku-low` | `gridcalc` | 23052 to 23060 | 23046 to 23056 |
| `native-haiku-low` | `matchbook` | 23088 to 23094 | 23080 to 23088 |
| `native-luna-low` | `gridcalc` | 15597 to 15603 | 15591 to 15597 |
| `native-luna-low` | `matchbook` | 15622 to 15626 | 15616 to 15624 |

**So the first branch of the headline rule applies:** the same Rust source and
binary, the same counted command lines and prompts and the same CLI versions
as the second measurement. The README's table gives all ten version 2 runs
together, marked as two sittings, and this report gives the new sitting's own
figures beside it.

What does differ, and is named so that it is not hidden: the path of the
checkout. Every trial's workspace path contains it, and the CLI's environment
block and Aether's first message both print that path. It has the same number
of characters as in the second measurement and other characters, so the sizes
above are equal in characters while the first request's token count sits a few
tokens beside the second measurement's range (the table), on both sides of a
pair. Inside one sitting the count moves in the same way from trial to trial
with the trial's own directory name. That the path accounts for all of it was
not checked token by token.

## What was compared

Two matched pairs, each the installed native CLI against one Aether agent that
drives the same CLI:

| Pair | Native cell | Aether cell | Model on both sides | Requested effort |
| --- | --- | --- | --- | --- |
| `haiku-low` | `native-haiku-low`: `claude -p --model claude-haiku-5-5 --effort low` | `aether-single-haiku-low`: one agent on `claude-code/claude-haiku-5-5` | Claude Haiku 5.5 | low |
| `luna-low` | `native-luna-low`: `codex exec --model gpt-6-luna -c model_reasoning_effort="low"` | `aether-single-luna-low`: one agent on `codex/gpt-6-luna` | GPT-6 Luna | low |

Both sides of a pair get the same task text, the same requested effort and a
fresh copy of the same starter repository. The solver never sees the hidden
checks: they are applied to the files it leaves. No other model was used as a
solver. The Aether cells are a single agent told not to delegate, so this
measures what the Aether layer adds or costs around one native session, not a
team of agents. Under protocol version 2 neither side of the Haiku pair loads
the machine user's MCP connectors; the Aether side loads Aether's own server.

The two cases are the frozen cases of the first and second measurements (see
the [protocol](../evals/harness_bench_protocol.md)):

| Case | Hidden checks | Unmodified starter passes | Frozen digest |
| --- | ---: | ---: | --- |
| `matchbook` (limit order book with a journal and iceberg orders) | 119 | 23 | `7f719f2d3e0b7c98228f9bdf9a922aea93b605cbec9cb3b46535fe66ab8d630a` |
| `gridcalc` (spreadsheet engine with formulas and structural edits) | 154 | 5 | `df35116f6492541fbdc8078c26355647483ff321332b91bb7ab3c9c975dd4758` |

The digests were recomputed with `fixture_digest()` before the first trial
and equal the frozen values. Every manifest records the hash of each starter
and hidden-check file, and every trial rechecks them before and after it
runs.

## How it was run

| | |
| --- | --- |
| Aether binary | `aether 0.1.1`, `cargo build --release`, SHA-256 `251c0b08f359c8a679a76831dea1d892b0c2a0a2c34e51427323c8b25d4d4145`, the same file in all 5 runs, not installed. It is byte for byte the binary of the second measurement. Every Aether trial ran on its own isolated daemon |
| Built from | commit `f967288b8cd0606b32d7681d5209a6a973907bd5`; no file under `crates/` differs from the second measurement's commit |
| Benchmark code | every manifest records commit `0ca5dee615d1c167f675ac4c9cb913e76d3d5380` with a clean tree and `protocol_version` 2. That commit is `f967288b` plus the one check on the uncounted grade (`91c2409d`) and the plan of this report |
| Native CLIs | `claude` 2.1.294 (Claude Code), `codex-cli` 0.162.1: the versions and the executables of the second measurement |
| Cells | `native-haiku-low`, `aether-single-haiku-low`, `native-luna-low`, `aether-single-luna-low` |
| Cases | `matchbook`, `gridcalc` |
| Runs | 5, one runner invocation each, `--runs 1`, seeds 6 to 10, launched one after another by one script. Each is the matched set of 8 trials. Five were planned before the first, on rule 8 of the counting rule |
| Trials at once | `--jobs 8`: all 8 trials of a run in one block, so both sides of every pair ran under the same load |
| Timeout | `--timeout 1200` seconds for every trial on both sides |
| Memory guard | the script of the diagnosis's fifth iteration, changed in two lines: where it logs and which script's lifetime it follows. Every 2 seconds it ends any process above 4 GB of resident memory whose working directory is inside one of this checkout's trial workspaces, and logs it with the trial id. It leaves the grader alone |
| Machine | one Linux host with 12 hardware threads and 15 GB of memory, shared with other work |

Per run (times in UTC on 2026-10-11; load average is 1, 5 and 15 minutes;
available memory was sampled every 20 seconds):

| Run | Seed | `--jobs` | `--timeout` (s) | Start (UTC) | End (UTC) | Load average at start | Load average at end | Available memory at start (MB) | Free disk at start (GB) | Lowest available memory (MB) | Runner exit | Manifest |
| --- | ---: | ---: | ---: | --- | --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| 6 | 6 | 8 | 1200 | 08:45:24 | 09:00:19 | 1.55 2.66 1.73 | 0.61 0.96 1.30 | 7491 | 39 | 6340 | 0 | `run-6/harness-bench-2026-10-11T084525Z.json` |
| 7 | 7 | 8 | 1200 | 09:00:19 | 09:10:54 | 0.61 0.96 1.30 | 0.86 1.03 1.17 | 7500 | 39 | 6292 | 0 | `run-7/harness-bench-2026-10-11T090020Z.json` |
| 8 | 8 | 8 | 1200 | 09:10:54 | 09:22:49 | 0.86 1.03 1.17 | 0.69 0.81 0.97 | 7495 | 39 | 6144 | 0 | `run-8/harness-bench-2026-10-11T091055Z.json` |
| 9 | 9 | 8 | 1200 | 09:22:49 | 09:36:56 | 0.69 0.81 0.97 | 1.03 1.12 1.07 | 7301 | 39 | 5015 | 0 | `run-9/harness-bench-2026-10-11T092250Z.json` |
| 10 | 10 | 8 | 1200 | 09:36:56 | 09:46:55 | 1.03 1.12 1.07 | 1.35 1.10 1.04 | 7789 | 39 | 6829 | 0 | `run-10/harness-bench-2026-10-11T093657Z.json` |

Before each run the script waited for at least 4,000 MB of available memory
and 15 GB of free disk and recorded both; `--jobs` was never lowered. Each
manifest was copied out of the working tree as soon as its run ended and
before the next run started. The plan was committed at 08:45:16 UTC (commit
`0ca5dee6`); run 6 started at 08:45:24.

The memory guard ran from 08:45:27 UTC until 09:48:28 UTC, when it was stopped
by hand after the last run had ended. It ended 2 processes, listed under
[Timeouts, failures, guard firings and unknown
figures](#timeouts-failures-guard-firings-and-unknown-figures). Lowest
available memory per run: 6,340 MB in run 6 (at 08:46:04 UTC), 6,292 MB in run
7 (at 09:01:59 UTC), 6,144 MB in run 8 (at 09:11:34 UTC), 5,015 MB in run 9
(at 09:24:09 UTC), 6,829 MB in run 10 (at 09:37:36 UTC).

One slip of the sitting, named here: while run 7 was in flight, a test of the
tooling that assembles this report wrote over the working copy of this file
for about 20 seconds; it was restored from the commit at once. No file a
trial reads was touched. Every manifest records the tree as clean at commit
`0ca5dee6` when its run started.

## Results per model pair, runs 6 to 10

Printed by `evals/harness_bench_compare.py --trials` from the five saved
manifests of this sitting, unedited.

## Native and Aether, matched model and effort

### haiku-low

- Native: `claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Aether: `claude-code/claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Claude all-model process receipts
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 10 | 10 | |
| Workflows finished | 10/10 | 10/10 | |
| Hidden checks passed, mean | 99.6 % (10/10) | 99.5 % (10/10) | -0.1 pp (10 pairs) |
| Hidden checks per run | gridcalc: 154/154, 154/154, 154/154, 153/154, 154/154; matchbook: 117/119, 117/119, 119/119, 119/119, 119/119 | gridcalc: 154/154, 154/154, 154/154, 154/154, 154/154; matchbook: 119/119, 119/119, 117/119, 117/119, 117/119 | |
| Uncached input tokens, mean | 598 (10/10) | 644 (10/10) | +46 (10 pairs) |
| Output tokens, mean | 121,513 (10/10) | 120,912 (10/10) | -601 (10 pairs) |
| Cache read tokens, mean | 2,315,255 (10/10) | 2,429,522 (10/10) | +114,267 (10 pairs) |
| Cache write tokens, mean | 151,102 (10/10) | 149,443 (10/10) | -1,659 (10 pairs) |
| API-equivalent USD, mean | 0.3346 (billed; 10/10 priced) | 0.3515 (billed; 10/10 priced) | +0.0169 (10 pairs) |
| Wall seconds, mean (min to max) | 520.5 (309.2 to 893.3; 10/10) | 509.4 (275.5 to 844.8; 10/10) | -11.0 (10 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

10 native and 10 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

### luna-low

- Native: `gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Aether: `codex/gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Aether durable (may omit helpers)
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 10 | 10 | |
| Workflows finished | 10/10 | 10/10 | |
| Hidden checks passed, mean | 70.1 % (10/10) | 77.2 % (10/10) | +7.1 pp (10 pairs) |
| Hidden checks per run | gridcalc: 138/154, 117/154, 110/154, 136/154, 136/154; matchbook: 0/119, 114/119, 117/119, 0/119, 111/119 | gridcalc: 140/154, 120/154, 126/154, 118/154, 133/154; matchbook: 89/119, 107/119, 114/119, 117/119, 0/119 | |
| Uncached input tokens, mean | 52,583 (10/10) | 74,892 (10/10) | +22,309 (10 pairs) |
| Output tokens, mean | 13,901 (10/10) | 19,086 (10/10) | +5,185 (10 pairs) |
| Cache read tokens, mean | 889,856 (10/10) | 1,755,648 (10/10) | +865,792 (10 pairs) |
| Cache write tokens, mean | 0 (10/10) | 0 (10/10) | +0 (10 pairs) |
| API-equivalent USD, mean | 0.0211 (estimated; 10/10 priced) | 0.0346 (billed; 10/10 priced) | +0.0135 (10 pairs) |
| Wall seconds, mean (min to max) | 168.0 (70.5 to 338.1; 10/10) | 270.9 (100.1 to 882.7; 10/10) | +102.9 (10 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

10 native and 10 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

Dollar values are API-equivalent (reported metadata or list-price estimates), not subscription charges. A mean covers only the trials that measured it: a token or cost figure counts as measured only when every unit of its accounting scope reported it at a terminal point. A pair is one case and run within one manifest; trials from different manifests are never paired, so run both sides of a pair in the same block.

## Every trial

Manifests are listed in the order given, each one a separate runner invocation; the Run column is the run within its manifest.

| Manifest | Case | Run | Cell | Workflow | Hidden checks | Hidden checks test by test (not counted) | Uncached input | Output | Cache read | Cache write | USD | Wall seconds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| run-6/harness-bench-2026-10-11T084525Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 154/154 | 2,659 | 190,573 | 3,809,360 | 238,464 | 0.6233 | 893.3 |
| run-6/harness-bench-2026-10-11T084525Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 154/154 | 38 | 147,556 | 2,502,591 | 183,373 | 0.4415 | 618.2 |
| run-6/harness-bench-2026-10-11T084525Z.json | gridcalc | 1 | native-luna-low | completed | 138/154 | 138/154 | 71,353 | 15,047 | 1,240,576 | 0 | 0.0271 | 338.1 |
| run-6/harness-bench-2026-10-11T084525Z.json | gridcalc | 1 | aether-single-luna-low | completed | 140/154 | 140/154 | 69,264 | 26,466 | 973,312 | 0 | 0.0299 | 528.2 |
| run-6/harness-bench-2026-10-11T084525Z.json | matchbook | 1 | native-haiku-low | completed | 117/119 | 117/119 | 26 | 84,181 | 1,072,180 | 109,160 | 0.1575 | 354.8 |
| run-6/harness-bench-2026-10-11T084525Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 119/119 | 24 | 96,046 | 1,018,743 | 130,456 | 0.2560 | 399.0 |
| run-6/harness-bench-2026-10-11T084525Z.json | matchbook | 1 | native-luna-low | completed | 0/119 | 94/119 (1 hung) | 41,043 | 9,860 | 556,800 | 0 | 0.0146 | 91.5 |
| run-6/harness-bench-2026-10-11T084525Z.json | matchbook | 1 | aether-single-luna-low | completed | 89/119 | 89/119 | 182,511 | 35,795 | 8,532,736 | 0 | 0.1215 | 882.7 |
| run-7/harness-bench-2026-10-11T090020Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 154/154 | 44 | 144,620 | 2,996,622 | 172,514 | 0.4637 | 633.7 |
| run-7/harness-bench-2026-10-11T090020Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 154/154 | 36 | 140,734 | 2,314,045 | 166,420 | 0.4029 | 583.8 |
| run-7/harness-bench-2026-10-11T090020Z.json | gridcalc | 1 | native-luna-low | completed | 117/154 | 117/154 | 44,297 | 13,722 | 779,264 | 0 | 0.0191 | 120.9 |
| run-7/harness-bench-2026-10-11T090020Z.json | gridcalc | 1 | aether-single-luna-low | completed | 120/154 | 120/154 | 66,566 | 18,480 | 763,392 | 0 | 0.0235 | 168.2 |
| run-7/harness-bench-2026-10-11T090020Z.json | matchbook | 1 | native-haiku-low | completed | 117/119 | 117/119 | 26 | 88,523 | 1,092,889 | 111,131 | 0.1792 | 369.7 |
| run-7/harness-bench-2026-10-11T090020Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 119/119 | 28 | 75,638 | 1,140,390 | 100,666 | 0.1329 | 313.9 |
| run-7/harness-bench-2026-10-11T090020Z.json | matchbook | 1 | native-luna-low | completed | 114/119 | 114/119 | 65,514 | 16,784 | 1,864,192 | 0 | 0.0336 | 158.3 |
| run-7/harness-bench-2026-10-11T090020Z.json | matchbook | 1 | aether-single-luna-low | completed | 107/119 | 107/119 | 50,127 | 16,606 | 811,776 | 0 | 0.0214 | 252.5 |
| run-8/harness-bench-2026-10-11T091055Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 154/154 | 44 | 149,700 | 3,055,736 | 177,035 | 0.4746 | 627.0 |
| run-8/harness-bench-2026-10-11T091055Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 154/154 | 54 | 170,460 | 4,252,888 | 198,312 | 0.5665 | 712.7 |
| run-8/harness-bench-2026-10-11T091055Z.json | gridcalc | 1 | native-luna-low | completed | 110/154 | 110/154 | 58,213 | 15,580 | 1,224,704 | 0 | 0.0259 | 153.5 |
| run-8/harness-bench-2026-10-11T091055Z.json | gridcalc | 1 | aether-single-luna-low | completed | 126/154 | 126/154 | 88,549 | 17,449 | 1,224,192 | 0 | 0.0298 | 159.7 |
| run-8/harness-bench-2026-10-11T091055Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 119/119 | 20 | 73,965 | 696,453 | 95,911 | 0.0912 | 309.2 |
| run-8/harness-bench-2026-10-11T091055Z.json | matchbook | 1 | aether-single-haiku-low | completed | 117/119 | 117/119 | 28 | 99,147 | 1,328,169 | 127,428 | 0.2707 | 425.9 |
| run-8/harness-bench-2026-10-11T091055Z.json | matchbook | 1 | native-luna-low | completed | 117/119 | 117/119 | 72,330 | 14,444 | 731,392 | 0 | 0.0218 | 150.5 |
| run-8/harness-bench-2026-10-11T091055Z.json | matchbook | 1 | aether-single-luna-low | completed | 114/119 | 114/119 | 52,405 | 12,848 | 872,448 | 0 | 0.0204 | 131.7 |
| run-9/harness-bench-2026-10-11T092250Z.json | gridcalc | 1 | native-haiku-low | completed | 153/154 | 153/154 | 3,053 | 188,342 | 4,800,255 | 237,404 | 0.6610 | 774.9 |
| run-9/harness-bench-2026-10-11T092250Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 154/154 | 6,145 | 190,866 | 7,605,826 | 227,247 | 0.8146 | 844.8 |
| run-9/harness-bench-2026-10-11T092250Z.json | gridcalc | 1 | native-luna-low | completed | 136/154 | 136/154 | 46,951 | 14,639 | 835,328 | 0 | 0.0204 | 305.6 |
| run-9/harness-bench-2026-10-11T092250Z.json | gridcalc | 1 | aether-single-luna-low | completed | 118/154 | 118/154 | 56,352 | 16,881 | 940,800 | 0 | 0.0235 | 145.8 |
| run-9/harness-bench-2026-10-11T092250Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 119/119 | 30 | 78,964 | 1,152,714 | 101,334 | 0.1241 | 326.1 |
| run-9/harness-bench-2026-10-11T092250Z.json | matchbook | 1 | aether-single-haiku-low | completed | 117/119 | 117/119 | 16 | 68,087 | 486,735 | 92,753 | 0.0790 | 275.5 |
| run-9/harness-bench-2026-10-11T092250Z.json | matchbook | 1 | native-luna-low | completed | 0/119 | 109/119 (2 hung) | 51,273 | 17,362 | 816,896 | 0 | 0.0220 | 187.4 |
| run-9/harness-bench-2026-10-11T092250Z.json | matchbook | 1 | aether-single-luna-low | completed | 117/119 | 117/119 | 45,509 | 10,955 | 515,840 | 0 | 0.0152 | 100.1 |
| run-10/harness-bench-2026-10-11T093657Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 154/154 | 50 | 141,178 | 3,509,673 | 169,118 | 0.4692 | 597.9 |
| run-10/harness-bench-2026-10-11T093657Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 154/154 | 32 | 135,950 | 1,957,278 | 160,015 | 0.3744 | 560.4 |
| run-10/harness-bench-2026-10-11T093657Z.json | gridcalc | 1 | native-luna-low | completed | 136/154 | 136/154 | 33,828 | 13,082 | 566,016 | 0 | 0.0156 | 104.0 |
| run-10/harness-bench-2026-10-11T093657Z.json | gridcalc | 1 | aether-single-luna-low | completed | 133/154 | 133/154 | 85,603 | 22,805 | 2,103,808 | 0 | 0.0410 | 217.0 |
| run-10/harness-bench-2026-10-11T093657Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 119/119 | 26 | 75,085 | 966,664 | 98,949 | 0.1019 | 318.1 |
| run-10/harness-bench-2026-10-11T093657Z.json | matchbook | 1 | aether-single-haiku-low | completed | 117/119 | 117/119 | 38 | 84,639 | 1,688,553 | 107,763 | 0.1761 | 360.3 |
| run-10/harness-bench-2026-10-11T093657Z.json | matchbook | 1 | native-luna-low | completed | 111/119 | 111/119 | 41,026 | 8,494 | 283,392 | 0 | 0.0112 | 70.5 |
| run-10/harness-bench-2026-10-11T093657Z.json | matchbook | 1 | aether-single-luna-low | completed | 0/119 | 106/119 (1 hung) | 52,029 | 12,576 | 818,176 | 0 | 0.0197 | 123.1 |

A figure reading "at least" comes from incomplete receipts and is a lower bound. The manifest column is the file and its directory, not a full path.

The test-by-test column repeats the counted grade unless the grader's one process timed out; then every hidden test ran in a process of its own. It is not the counted grade. "unknown" there means that grade could not be made; the counted grade beside it stands.

## All ten runs under protocol version 2

Printed by `evals/harness_bench_compare.py` from the ten saved manifests,
runs 1 to 5 of the second measurement and runs 6 to 10 of this sitting, in that
order. Its headings are lowered one level; nothing else is edited. In the
"Hidden checks per run" rows the first five counts of a case are runs 1 to 5
and the last five are runs 6 to 10. The 40 trials of runs 1 to 5 are listed in
the [second report](harness-lower-tier-results-2026-10-10-protocol-2.md#every-trial).

### Native and Aether, matched model and effort

#### haiku-low

- Native: `claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Aether: `claude-code/claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Claude all-model process receipts
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 20 | 20 | |
| Workflows finished | 20/20 | 20/20 | |
| Hidden checks passed, mean | 99.8 % (20/20) | 99.6 % (20/20) | -0.2 pp (20 pairs) |
| Hidden checks per run | gridcalc: 154/154, 154/154, 154/154, 154/154, 154/154, 154/154, 154/154, 154/154, 153/154, 154/154; matchbook: 119/119, 119/119, 119/119, 119/119, 119/119, 117/119, 117/119, 119/119, 119/119, 119/119 | gridcalc: 154/154, 154/154, 154/154, 154/154, 154/154, 154/154, 154/154, 154/154, 154/154, 154/154; matchbook: 119/119, 117/119, 119/119, 119/119, 117/119, 119/119, 119/119, 117/119, 117/119, 117/119 | |
| Uncached input tokens, mean | 426 (20/20) | 337 (20/20) | -89 (20 pairs) |
| Output tokens, mean | 120,865 (20/20) | 116,659 (20/20) | -4,205 (20 pairs) |
| Cache read tokens, mean | 2,476,166 (20/20) | 2,045,135 (20/20) | -431,030 (20 pairs) |
| Cache write tokens, mean | 148,736 (20/20) | 143,134 (20/20) | -5,602 (20 pairs) |
| API-equivalent USD, mean | 0.3428 (billed; 20/20 priced) | 0.3162 (billed; 20/20 priced) | -0.0266 (20 pairs) |
| Wall seconds, mean (min to max) | 528.3 (259.9 to 951.6; 20/20) | 490.2 (261.6 to 844.8; 20/20) | -38.1 (20 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

20 native and 20 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

#### luna-low

- Native: `gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Aether: `codex/gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Aether durable (may omit helpers)
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 20 | 20 | |
| Workflows finished | 20/20 | 20/20 | |
| Hidden checks passed, mean | 73.4 % (20/20) | 76.6 % (20/20) | +3.2 pp (20 pairs) |
| Hidden checks per run | gridcalc: 132/154, 121/154, 93/154, 140/154, 131/154, 138/154, 117/154, 110/154, 136/154, 136/154; matchbook: 94/119, 118/119, 118/119, 106/119, 0/119, 0/119, 114/119, 117/119, 0/119, 111/119 | gridcalc: 119/154, 108/154, 116/154, 128/154, 117/154, 140/154, 120/154, 126/154, 118/154, 133/154; matchbook: 110/119, 116/119, 119/119, 0/119, 104/119, 89/119, 107/119, 114/119, 117/119, 0/119 | |
| Uncached input tokens, mean | 51,775 (20/20) | 74,106 (20/20) | +22,330 (20 pairs) |
| Output tokens, mean | 14,061 (20/20) | 18,948 (20/20) | +4,887 (20 pairs) |
| Cache read tokens, mean | 983,590 (20/20) | 1,786,816 (20/20) | +803,226 (20 pairs) |
| Cache write tokens, mean | 0 (20/20) | 0 (20/20) | +0 (20 pairs) |
| API-equivalent USD, mean | 0.0220 (estimated; 20/20 priced) | 0.0348 (billed; 20/20 priced) | +0.0127 (20 pairs) |
| Wall seconds, mean (min to max) | 202.6 (70.5 to 455.3; 20/20) | 249.2 (66.3 to 882.7; 20/20) | +46.6 (20 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

20 native and 20 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

Dollar values are API-equivalent (reported metadata or list-price estimates), not subscription charges. A mean covers only the trials that measured it: a token or cost figure counts as measured only when every unit of its accounting scope reported it at a terminal point. A pair is one case and run within one manifest; trials from different manifests are never paired, so run both sides of a pair in the same block.

### The two sittings side by side

Per model and measure: the second measurement (runs 1 to 5), this sitting
(runs 6 to 10), and all 10 runs with the 95 % interval of the mean pair
difference and the sign-flip p-value. "Lower / higher / equal" counts the
pairs in which the Aether trial's figure is below, above or equal to its
native partner's.

| Model | Measure | Runs 1 to 5: native | Aether | Aether minus native | Lower / higher / equal | Runs 6 to 10: native | Aether | Aether minus native | Lower / higher / equal | All ten: native | Aether | Aether minus native | Relative to native | Lower / higher / equal | 95 % interval | p |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed, as counted (percent) | 100.0 % | 99.7 % | -0.3 pp | 2 / 0 / 8 | 99.6 % | 99.5 % | -0.1 pp | 3 / 3 / 4 | 99.8 % | 99.6 % | -0.2 pp |  | 5 / 3 / 12 | -0.7 pp to +0.2 pp | 0.453 |
| Claude Haiku 5.5 | Uncached input tokens | 254 | 30 | -224 | 8 / 2 / 0 | 598 | 644 | +46 | 5 / 5 / 0 | 426 | 337 | -89 | -20.9 % | 13 / 7 / 0 | -506 to +352 | 0.759 |
| Claude Haiku 5.5 | Output tokens | 120,216 | 112,406 | -7,810 | 5 / 5 / 0 | 121,513 | 120,912 | -601 | 5 / 5 / 0 | 120,865 | 116,659 | -4,205 | -3.5 % | 10 / 10 / 0 | -13,442 to +4,333 | 0.392 |
| Claude Haiku 5.5 | Cache read tokens | 2,637,077 | 1,660,749 | -976,328 | 8 / 2 / 0 | 2,315,255 | 2,429,522 | +114,267 | 5 / 5 / 0 | 2,476,166 | 2,045,135 | -431,030 | -17.4 % | 13 / 7 / 0 | -1,033,039 to +157,542 | 0.184 |
| Claude Haiku 5.5 | API-equivalent USD | 0.3511 | 0.2810 | -0.0701 | 6 / 4 / 0 | 0.3346 | 0.3515 | +0.0169 | 5 / 5 / 0 | 0.3428 | 0.3162 | -0.0266 | -7.8 % | 11 / 9 / 0 | -0.0868 to +0.0305 | 0.400 |
| Claude Haiku 5.5 | Wall seconds | 536.1 | 470.9 | -65.2 | 6 / 4 / 0 | 520.5 | 509.4 | -11.0 | 5 / 5 / 0 | 528.3 | 490.2 | -38.1 | -7.2 % | 11 / 9 / 0 | -95.2 to +10.5 | 0.204 |
| Claude Haiku 5.5 | Hidden checks passed, test by test (percent, not counted) | 100.0 % | 99.7 % | -0.3 pp | 2 / 0 / 8 | 99.6 % | 99.5 % | -0.1 pp | 3 / 3 / 4 | 99.8 % | 99.6 % | -0.2 pp |  | 5 / 3 / 12 | -0.7 pp to +0.2 pp | 0.453 |
| GPT-6 Luna | Hidden checks passed, as counted (percent) | 76.7 % | 75.9 % | -0.8 pp | 6 / 4 / 0 | 70.1 % | 77.2 % | +7.1 pp | 5 / 5 / 0 | 73.4 % | 76.6 % | +3.2 pp |  | 11 / 9 / 0 | -16.1 pp to +23.0 pp | 0.841 |
| GPT-6 Luna | Uncached input tokens | 50,968 | 73,320 | +22,352 | 2 / 8 / 0 | 52,583 | 74,892 | +22,309 | 4 / 6 / 0 | 51,775 | 74,106 | +22,330 | +43.1 % | 6 / 14 / 0 | +5,266 to +41,869 | 0.021 |
| GPT-6 Luna | Output tokens | 14,221 | 18,810 | +4,590 | 3 / 7 / 0 | 13,901 | 19,086 | +5,185 | 3 / 7 / 0 | 14,061 | 18,948 | +4,887 | +34.8 % | 6 / 14 / 0 | +1,451 to +8,458 | 0.013 |
| GPT-6 Luna | Cache read tokens | 1,077,325 | 1,817,984 | +740,659 | 4 / 6 / 0 | 889,856 | 1,755,648 | +865,792 | 5 / 5 / 0 | 983,590 | 1,786,816 | +803,226 | +81.7 % | 9 / 11 / 0 | -5,210 to +1,801,229 | 0.096 |
| GPT-6 Luna | API-equivalent USD | 0.0230 | 0.0349 | +0.0119 | 3 / 7 / 0 | 0.0211 | 0.0346 | +0.0135 | 3 / 7 / 0 | 0.0220 | 0.0348 | +0.0127 | +57.7 % | 6 / 14 / 0 | +0.0015 to +0.0263 | 0.047 |
| GPT-6 Luna | Wall seconds | 237.2 | 227.5 | -9.8 | 6 / 4 / 0 | 168.0 | 270.9 | +102.9 | 3 / 7 / 0 | 202.6 | 249.2 | +46.6 | +23.0 % | 9 / 11 / 0 | -32.6 to +146.7 | 0.399 |
| GPT-6 Luna | Hidden checks passed, test by test (percent, not counted) | 85.9 % | 84.9 % | -1.0 pp | 6 / 4 / 0 | 87.2 % | 86.2 % | -1.0 pp | 6 / 4 / 0 | 86.6 % | 85.5 % | -1.0 pp |  | 12 / 8 / 0 | -4.1 pp to +2.4 pp | 0.547 |

The two sittings give a mean difference of the same sign on 7 of the 12
counted model-and-measure figures. The sign differs on Haiku's uncached input
tokens (-224 in runs 1 to 5, +46 in this sitting); Haiku's cache read tokens
(-976,328 in runs 1 to 5, +114,267 in this sitting); Haiku's API-equivalent
USD (-0.0701 USD in runs 1 to 5, +0.0169 USD in this sitting); Luna's hidden
checks passed as counted (-0.8 pp in runs 1 to 5, +7.1 pp in this sitting);
Luna's wall seconds (-9.8 s in runs 1 to 5, +102.9 s in this sitting).

The sizes move between the sittings too: Haiku's output tokens -7,810 then
-601; Haiku's API-equivalent USD -0.0701 USD then +0.0169 USD; Luna's output
tokens +4,590 then +5,185; Luna's API-equivalent USD +0.0119 USD then +0.0135
USD. Each sitting's interval is in [The statistics as
measured](#the-statistics-as-measured). Pairs are formed only inside one run,
so what a day does to both sides alike drops out of a pair difference. What a
day does to the difference itself cannot be told from two sittings.

Hidden checks passed per case over all 10 runs, with every run's count in run
order 1 to 10:

| Model | Harness | `matchbook` (119 checks) | `gridcalc` (154 checks) |
| --- | --- | --- | --- |
| Claude Haiku 5.5 | Claude Code | 99.7 %: 119, 119, 119, 119, 119, 117, 117, 119, 119, 119 | 99.9 %: 154, 154, 154, 154, 154, 154, 154, 154, 153, 154 |
| Claude Haiku 5.5 | Aether | 99.2 %: 119, 117, 119, 119, 117, 119, 119, 117, 117, 117 | 100.0 %: 154, 154, 154, 154, 154, 154, 154, 154, 154, 154 |
| GPT-6 Luna | Codex | 65.4 %: 94, 118, 118, 106, 0, 0, 114, 117, 0, 111 | 81.4 %: 132, 121, 93, 140, 131, 138, 117, 110, 136, 136 |
| GPT-6 Luna | Aether | 73.6 %: 110, 116, 119, 0, 104, 89, 107, 114, 117, 0 | 79.5 %: 119, 108, 116, 128, 117, 140, 120, 126, 118, 133 |

## The statistics as measured

As [planned](#statistics): per model and measure, the two means and medians,
the mean and median pair difference (Aether minus native), the mean
difference relative to the native mean, how the pairs fall, the smallest and
largest pair difference, the 95 % interval of the mean pair difference from a
paired bootstrap (10,000 resamples of whole pairs, seed 20261011, 2.5th to
97.5th percentile) and the exact two-sided sign-flip permutation p-value.
Hidden checks are in percentage points of each case's checks here and in
checks in the per-case tables. The test-by-test rows use the second,
uncounted grade.

**These are descriptive. Seven measures are looked at for each of two models,
so one interval among them that excludes zero is not by itself a finding. No
significance is claimed on five runs; over ten runs an interval is reported
as it falls.**

### Runs 6 to 10, this sitting (10 pairs per model)

| Model | Measure | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed, as counted (percent) | 99.6 % | 99.5 % | 100.0 % | 100.0 % | -0.1 pp | +0.0 pp |  | 3 / 3 / 4 | -1.7 pp | +1.7 pp | -0.8 pp to +0.6 pp | 1.000 | 10 |
| Claude Haiku 5.5 | Uncached input tokens | 598 | 644 | 37 | 34 | +46 | +0 | +7.7 % | 5 / 5 / 0 | -2,621 | +3,092 | -786 to +927 | 0.824 | 10 |
| Claude Haiku 5.5 | Output tokens | 121,513 | 120,912 | 114,850 | 117,548 | -601 | -681 | -0.5 % | 5 / 5 / 0 | -43,017 | +25,182 | -13,065 to +10,123 | 0.930 | 10 |
| Claude Haiku 5.5 | Cache read tokens | 2,315,255 | 2,429,522 | 2,074,668 | 1,822,916 | +114,267 | -2,968 | +4.9 % | 5 / 5 / 0 | -1,552,395 | +2,805,571 | -595,743 to +905,388 | 0.834 | 10 |
| Claude Haiku 5.5 | API-equivalent USD | 0.3346 | 0.3515 | 0.3214 | 0.3225 | +0.0169 | +0.0146 | +5.0 % | 5 / 5 / 0 | -0.1818 | +0.1795 | -0.0530 to +0.0855 | 0.656 | 10 |
| Claude Haiku 5.5 | Wall seconds | 520.5 | 509.4 | 483.8 | 493.2 | -11.0 | +2.4 | -2.1 % | 5 / 5 / 0 | -275.1 | +116.7 | -84.4 to +47.7 | 0.807 | 10 |
| Claude Haiku 5.5 | Hidden checks passed, test by test (percent, not counted) | 99.6 % | 99.5 % | 100.0 % | 100.0 % | -0.1 pp | +0.0 pp |  | 3 / 3 / 4 | -1.7 pp | +1.7 pp | -0.8 pp to +0.6 pp | 1.000 | 10 |
| GPT-6 Luna | Hidden checks passed, as counted (percent) | 70.1 % | 77.2 % | 88.3 % | 84.1 % | +7.1 pp | -0.3 pp |  | 5 / 5 / 0 | -93.3 pp | +98.3 pp | -21.6 pp to +37.6 pp | 0.791 | 10 |
| GPT-6 Luna | Uncached input tokens | 52,583 | 74,892 | 49,112 | 61,459 | +22,309 | +10,202 | +42.4 % | 4 / 6 / 0 | -19,925 | +141,468 | -859 to +52,699 | 0.141 | 10 |
| GPT-6 Luna | Output tokens | 13,901 | 19,086 | 14,542 | 17,165 | +5,185 | +3,162 | +37.3 % | 3 / 7 / 0 | -6,407 | +25,935 | +399 to +10,860 | 0.078 | 10 |
| GPT-6 Luna | Cache read tokens | 889,856 | 1,755,648 | 798,080 | 906,624 | +865,792 | +52,480 | +97.3 % | 5 / 5 / 0 | -1,052,416 | +7,975,936 | -220,211 to +2,555,110 | 0.367 | 10 |
| GPT-6 Luna | API-equivalent USD | 0.0211 | 0.0346 | 0.0211 | 0.0235 | +0.0135 | +0.0035 | +63.9 % | 3 / 7 / 0 | -0.0122 | +0.1069 | -0.0013 to +0.0360 | 0.227 | 10 |
| GPT-6 Luna | Wall seconds | 168.0 | 270.9 | 152.0 | 163.9 | +102.9 | +49.9 | +61.2 % | 3 / 7 / 0 | -159.8 | +791.3 | -21.5 to +275.8 | 0.236 | 10 |
| GPT-6 Luna | Hidden checks passed, test by test (percent, not counted) | 87.2 % | 86.2 % | 89.0 % | 87.7 % | -1.0 pp | -2.2 pp |  | 6 / 4 / 0 | -11.7 pp | +10.4 pp | -4.7 pp to +2.8 pp | 0.637 | 10 |

Hidden checks per case, in checks (5 pairs per case, 10 over both):

| Model | Case | Grade | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | `matchbook` | as counted | 118.2 | 117.8 | 119.0 | 117.0 | -0.4 | -2.0 | -0.3 % | 3 / 2 / 0 | -2.0 | +2.0 | -2.0 to +1.2 | 1.000 | 5 |
| Claude Haiku 5.5 | `matchbook` | test by test (not counted) | 118.2 | 117.8 | 119.0 | 117.0 | -0.4 | -2.0 | -0.3 % | 3 / 2 / 0 | -2.0 | +2.0 | -2.0 to +1.2 | 1.000 | 5 |
| Claude Haiku 5.5 | `gridcalc` | as counted | 153.8 | 154.0 | 154.0 | 154.0 | +0.2 | +0.0 | +0.1 % | 0 / 1 / 4 | +0.0 | +1.0 | +0.0 to +0.6 | 1.000 | 5 |
| Claude Haiku 5.5 | `gridcalc` | test by test (not counted) | 153.8 | 154.0 | 154.0 | 154.0 | +0.2 | +0.0 | +0.1 % | 0 / 1 / 4 | +0.0 | +1.0 | +0.0 to +0.6 | 1.000 | 5 |
| Claude Haiku 5.5 | both, in checks | as counted | 136.0 | 135.9 | 136.0 | 136.5 | -0.1 | +0.0 | -0.1 % | 3 / 3 / 4 | -2.0 | +2.0 | -1.0 to +0.8 | 1.000 | 10 |
| Claude Haiku 5.5 | both, in checks | test by test (not counted) | 136.0 | 135.9 | 136.0 | 136.5 | -0.1 | +0.0 | -0.1 % | 3 / 3 / 4 | -2.0 | +2.0 | -1.0 to +0.8 | 1.000 | 10 |
| GPT-6 Luna | `matchbook` | as counted | 68.4 | 85.4 | 111.0 | 107.0 | +17.0 | -3.0 | +24.9 % | 3 / 2 / 0 | -111.0 | +117.0 | -50.2 to +81.8 | 0.875 | 5 |
| GPT-6 Luna | `matchbook` | test by test (not counted) | 109.0 | 106.6 | 111.0 | 107.0 | -2.4 | -5.0 | -2.2 % | 4 / 1 / 0 | -7.0 | +8.0 | -5.8 to +2.8 | 0.500 | 5 |
| GPT-6 Luna | `gridcalc` | as counted | 127.4 | 127.4 | 136.0 | 126.0 | +0.0 | +2.0 | +0.0 % | 2 / 3 / 0 | -18.0 | +16.0 | -10.0 to +9.4 | 1.000 | 5 |
| GPT-6 Luna | `gridcalc` | test by test (not counted) | 127.4 | 127.4 | 136.0 | 126.0 | +0.0 | +2.0 | +0.0 % | 2 / 3 / 0 | -18.0 | +16.0 | -10.0 to +9.4 | 1.000 | 5 |
| GPT-6 Luna | both, in checks | as counted | 97.9 | 106.4 | 115.5 | 117.5 | +8.5 | -0.5 | +8.7 % | 5 / 5 / 0 | -111.0 | +117.0 | -26.2 to +45.1 | 0.793 | 10 |
| GPT-6 Luna | both, in checks | test by test (not counted) | 118.2 | 117.0 | 115.5 | 117.5 | -1.2 | -3.0 | -1.0 % | 6 / 4 / 0 | -18.0 | +16.0 | -6.6 to +4.3 | 0.711 | 10 |

### Runs 1 to 5, the second measurement (10 pairs per model)

Recomputed from its saved manifests. The means and pair counts are those its
report prints; the medians, intervals and p-values are new. Its manifests hold
no test-by-test grade: for its two trials graded 0 by a hung grader the
per-test output saved with it is used (107 and 110 of 119), and the counted
grade for every other trial.

| Model | Measure | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed, as counted (percent) | 100.0 % | 99.7 % | 100.0 % | 100.0 % | -0.3 pp | +0.0 pp |  | 2 / 0 / 8 | -1.7 pp | +0.0 pp | -0.8 pp to +0.0 pp | 0.500 | 10 |
| Claude Haiku 5.5 | Uncached input tokens | 254 | 30 | 35 | 27 | -224 | -9 | -88.2 % | 8 / 2 / 0 | -2,158 | +6 | -656 to -4 | 0.021 | 10 |
| Claude Haiku 5.5 | Output tokens | 120,216 | 112,406 | 118,876 | 113,326 | -7,810 | -4,200 | -6.5 % | 5 / 5 / 0 | -55,954 | +16,247 | -21,679 to +4,936 | 0.332 | 10 |
| Claude Haiku 5.5 | Cache read tokens | 2,637,077 | 1,660,749 | 1,978,319 | 1,368,877 | -976,328 | -742,359 | -37.0 % | 8 / 2 / 0 | -3,726,708 | +614,400 | -1,771,318 to -237,857 | 0.037 | 10 |
| Claude Haiku 5.5 | API-equivalent USD | 0.3511 | 0.2810 | 0.3239 | 0.2971 | -0.0701 | -0.0946 | -20.0 % | 6 / 4 / 0 | -0.3382 | +0.1259 | -0.1567 to +0.0165 | 0.168 | 10 |
| Claude Haiku 5.5 | Wall seconds | 536.1 | 470.9 | 513.1 | 465.8 | -65.2 | -19.2 | -12.2 % | 6 / 4 / 0 | -401.3 | +64.7 | -152.3 to +4.9 | 0.145 | 10 |
| Claude Haiku 5.5 | Hidden checks passed, test by test (percent, not counted) | 100.0 % | 99.7 % | 100.0 % | 100.0 % | -0.3 pp | +0.0 pp |  | 2 / 0 / 8 | -1.7 pp | +0.0 pp | -0.8 pp to +0.0 pp | 0.500 | 10 |
| GPT-6 Luna | Hidden checks passed, as counted (percent) | 76.7 % | 75.9 % | 85.4 % | 80.2 % | -0.8 pp | -4.7 pp |  | 6 / 4 / 0 | -89.1 pp | +87.4 pp | -27.3 pp to +24.2 pp | 0.887 | 10 |
| GPT-6 Luna | Uncached input tokens | 50,968 | 73,320 | 38,418 | 66,852 | +22,352 | +9,339 | +43.9 % | 2 / 8 / 0 | -41,568 | +104,202 | -538 to +46,620 | 0.078 | 10 |
| GPT-6 Luna | Output tokens | 14,221 | 18,810 | 12,390 | 18,518 | +4,590 | +7,514 | +32.3 % | 3 / 7 / 0 | -9,130 | +14,651 | -254 to +9,050 | 0.098 | 10 |
| GPT-6 Luna | Cache read tokens | 1,077,325 | 1,817,984 | 593,920 | 1,359,872 | +740,659 | +214,912 | +68.7 % | 4 / 6 / 0 | -2,010,368 | +4,427,776 | -235,597 to +1,784,627 | 0.246 | 10 |
| GPT-6 Luna | API-equivalent USD | 0.0230 | 0.0349 | 0.0160 | 0.0307 | +0.0119 | +0.0053 | +51.9 % | 3 / 7 / 0 | -0.0288 | +0.0620 | -0.0023 to +0.0267 | 0.166 | 10 |
| GPT-6 Luna | Wall seconds | 237.2 | 227.5 | 232.7 | 217.2 | -9.8 | -22.0 | -4.1 % | 6 / 4 / 0 | -185.4 | +271.1 | -88.7 to +75.8 | 0.824 | 10 |
| GPT-6 Luna | Hidden checks passed, test by test (percent, not counted) | 85.9 % | 84.9 % | 87.4 % | 85.3 % | -1.0 pp | -3.4 pp |  | 6 / 4 / 0 | -9.1 pp | +14.9 pp | -5.8 pp to +4.5 pp | 0.689 | 10 |

| Model | Case | Grade | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | `matchbook` | as counted | 119.0 | 118.2 | 119.0 | 119.0 | -0.8 | +0.0 | -0.7 % | 2 / 0 / 3 | -2.0 | +0.0 | -1.6 to +0.0 | 0.500 | 5 |
| Claude Haiku 5.5 | `matchbook` | test by test (not counted) | 119.0 | 118.2 | 119.0 | 119.0 | -0.8 | +0.0 | -0.7 % | 2 / 0 / 3 | -2.0 | +0.0 | -1.6 to +0.0 | 0.500 | 5 |
| Claude Haiku 5.5 | `gridcalc` | as counted | 154.0 | 154.0 | 154.0 | 154.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 5 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 5 |
| Claude Haiku 5.5 | `gridcalc` | test by test (not counted) | 154.0 | 154.0 | 154.0 | 154.0 | +0.0 | +0.0 | +0.0 % | 0 / 0 / 5 | +0.0 | +0.0 | +0.0 to +0.0 | 1.000 | 5 |
| Claude Haiku 5.5 | both, in checks | as counted | 136.5 | 136.1 | 136.5 | 136.5 | -0.4 | +0.0 | -0.3 % | 2 / 0 / 8 | -2.0 | +0.0 | -1.0 to +0.0 | 0.500 | 10 |
| Claude Haiku 5.5 | both, in checks | test by test (not counted) | 136.5 | 136.1 | 136.5 | 136.5 | -0.4 | +0.0 | -0.3 % | 2 / 0 / 8 | -2.0 | +0.0 | -1.0 to +0.0 | 0.500 | 10 |
| GPT-6 Luna | `matchbook` | as counted | 87.2 | 89.8 | 106.0 | 110.0 | +2.6 | +1.0 | +3.0 % | 2 / 3 / 0 | -106.0 | +104.0 | -60.2 to +62.8 | 0.938 | 5 |
| GPT-6 Luna | `matchbook` | test by test (not counted) | 109.2 | 111.2 | 110.0 | 110.0 | +2.0 | +1.0 | +1.8 % | 2 / 3 / 0 | -6.0 | +16.0 | -3.2 to +9.4 | 0.812 | 5 |
| GPT-6 Luna | `gridcalc` | as counted | 123.4 | 117.6 | 131.0 | 117.0 | -5.8 | -13.0 | -4.7 % | 4 / 1 / 0 | -14.0 | +23.0 | -13.4 to +8.6 | 0.375 | 5 |
| GPT-6 Luna | `gridcalc` | test by test (not counted) | 123.4 | 117.6 | 131.0 | 117.0 | -5.8 | -13.0 | -4.7 % | 4 / 1 / 0 | -14.0 | +23.0 | -13.4 to +8.6 | 0.375 | 5 |
| GPT-6 Luna | both, in checks | as counted | 105.3 | 103.7 | 118.0 | 116.0 | -1.6 | -7.0 | -1.5 % | 6 / 4 / 0 | -106.0 | +104.0 | -33.1 to +28.3 | 0.842 | 10 |
| GPT-6 Luna | both, in checks | test by test (not counted) | 116.3 | 114.4 | 118.0 | 116.0 | -1.9 | -4.0 | -1.6 % | 6 / 4 / 0 | -14.0 | +23.0 | -8.8 to +6.1 | 0.648 | 10 |

### All ten runs (20 pairs per model)

| Model | Measure | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed, as counted (percent) | 99.8 % | 99.6 % | 100.0 % | 100.0 % | -0.2 pp | +0.0 pp |  | 5 / 3 / 12 | -1.7 pp | +1.7 pp | -0.7 pp to +0.2 pp | 0.453 | 20 |
| Claude Haiku 5.5 | Uncached input tokens | 426 | 337 | 35 | 29 | -89 | -5 | -20.9 % | 13 / 7 / 0 | -2,621 | +3,092 | -506 to +352 | 0.759 | 20 |
| Claude Haiku 5.5 | Output tokens | 120,865 | 116,659 | 116,062 | 116,855 | -4,205 | -1,426 | -3.5 % | 10 / 10 / 0 | -55,954 | +25,182 | -13,442 to +4,333 | 0.392 | 20 |
| Claude Haiku 5.5 | Cache read tokens | 2,476,166 | 2,045,135 | 1,978,319 | 1,597,888 | -431,030 | -318,370 | -17.4 % | 13 / 7 / 0 | -3,726,708 | +2,805,571 | -1,033,039 to +157,542 | 0.184 | 20 |
| Claude Haiku 5.5 | API-equivalent USD | 0.3428 | 0.3162 | 0.3239 | 0.3152 | -0.0266 | -0.0456 | -7.8 % | 11 / 9 / 0 | -0.3382 | +0.1795 | -0.0868 to +0.0305 | 0.400 | 20 |
| Claude Haiku 5.5 | Wall seconds | 528.3 | 490.2 | 497.7 | 488.1 | -38.1 | -19.2 | -7.2 % | 11 / 9 / 0 | -401.3 | +116.7 | -95.2 to +10.5 | 0.204 | 20 |
| Claude Haiku 5.5 | Hidden checks passed, test by test (percent, not counted) | 99.8 % | 99.6 % | 100.0 % | 100.0 % | -0.2 pp | +0.0 pp |  | 5 / 3 / 12 | -1.7 pp | +1.7 pp | -0.7 pp to +0.2 pp | 0.453 | 20 |
| GPT-6 Luna | Hidden checks passed, as counted (percent) | 73.4 % | 76.6 % | 87.0 % | 82.5 % | +3.2 pp | -1.8 pp |  | 11 / 9 / 0 | -93.3 pp | +98.3 pp | -16.1 pp to +23.0 pp | 0.841 | 20 |
| GPT-6 Luna | Uncached input tokens | 51,775 | 74,106 | 45,624 | 63,586 | +22,330 | +9,448 | +43.1 % | 6 / 14 / 0 | -41,568 | +141,468 | +5,266 to +41,869 | 0.021 | 20 |
| GPT-6 Luna | Output tokens | 14,061 | 18,948 | 13,402 | 17,914 | +4,887 | +4,420 | +34.8 % | 6 / 14 / 0 | -9,130 | +25,935 | +1,451 to +8,458 | 0.013 | 20 |
| GPT-6 Luna | Cache read tokens | 983,590 | 1,786,816 | 719,744 | 970,368 | +803,226 | +82,560 | +81.7 % | 9 / 11 / 0 | -2,010,368 | +7,975,936 | -5,210 to +1,801,229 | 0.096 | 20 |
| GPT-6 Luna | API-equivalent USD | 0.0220 | 0.0348 | 0.0181 | 0.0247 | +0.0127 | +0.0035 | +57.7 % | 6 / 14 / 0 | -0.0288 | +0.1069 | +0.0015 to +0.0263 | 0.047 | 20 |
| GPT-6 Luna | Wall seconds | 202.6 | 249.2 | 180.7 | 163.9 | +46.6 | +20.9 | +23.0 % | 9 / 11 / 0 | -185.4 | +791.3 | -32.6 to +146.7 | 0.399 | 20 |
| GPT-6 Luna | Hidden checks passed, test by test (percent, not counted) | 86.6 % | 85.5 % | 88.7 % | 86.9 % | -1.0 pp | -2.2 pp |  | 12 / 8 / 0 | -11.7 pp | +14.9 pp | -4.1 pp to +2.4 pp | 0.547 | 20 |

Hidden checks per case, in checks (10 pairs per case, 20 over both):

| Model | Case | Grade | Native mean | Aether mean | Native median | Aether median | Mean pair difference | Median pair difference | Relative to native mean | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | 95 % interval of the mean pair difference | p (sign flip, two-sided) | Pairs |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | `matchbook` | as counted | 118.6 | 118.0 | 119.0 | 118.0 | -0.6 | -1.0 | -0.5 % | 5 / 2 / 3 | -2.0 | +2.0 | -1.6 to +0.4 | 0.453 | 10 |
| Claude Haiku 5.5 | `matchbook` | test by test (not counted) | 118.6 | 118.0 | 119.0 | 118.0 | -0.6 | -1.0 | -0.5 % | 5 / 2 / 3 | -2.0 | +2.0 | -1.6 to +0.4 | 0.453 | 10 |
| Claude Haiku 5.5 | `gridcalc` | as counted | 153.9 | 154.0 | 154.0 | 154.0 | +0.1 | +0.0 | +0.1 % | 0 / 1 / 9 | +0.0 | +1.0 | +0.0 to +0.3 | 1.000 | 10 |
| Claude Haiku 5.5 | `gridcalc` | test by test (not counted) | 153.9 | 154.0 | 154.0 | 154.0 | +0.1 | +0.0 | +0.1 % | 0 / 1 / 9 | +0.0 | +1.0 | +0.0 to +0.3 | 1.000 | 10 |
| Claude Haiku 5.5 | both, in checks | as counted | 136.2 | 136.0 | 136.0 | 136.5 | -0.2 | +0.0 | -0.2 % | 5 / 3 / 12 | -2.0 | +2.0 | -0.8 to +0.2 | 0.453 | 20 |
| Claude Haiku 5.5 | both, in checks | test by test (not counted) | 136.2 | 136.0 | 136.0 | 136.5 | -0.2 | +0.0 | -0.2 % | 5 / 3 / 12 | -2.0 | +2.0 | -0.8 to +0.2 | 0.453 | 20 |
| GPT-6 Luna | `matchbook` | as counted | 77.8 | 87.6 | 108.5 | 108.5 | +9.8 | -0.5 | +12.6 % | 5 / 5 / 0 | -111.0 | +117.0 | -35.0 to +55.2 | 0.752 | 10 |
| GPT-6 Luna | `matchbook` | test by test (not counted) | 109.1 | 108.9 | 110.5 | 108.5 | -0.2 | -2.5 | -0.2 % | 6 / 4 / 0 | -7.0 | +16.0 | -3.9 to +4.5 | 0.969 | 10 |
| GPT-6 Luna | `gridcalc` | as counted | 125.4 | 122.5 | 131.5 | 119.5 | -2.9 | -7.5 | -2.3 % | 6 / 4 / 0 | -18.0 | +23.0 | -10.5 to +5.6 | 0.520 | 10 |
| GPT-6 Luna | `gridcalc` | test by test (not counted) | 125.4 | 122.5 | 131.5 | 119.5 | -2.9 | -7.5 | -2.3 % | 6 / 4 / 0 | -18.0 | +23.0 | -10.5 to +5.6 | 0.520 | 10 |
| GPT-6 Luna | both, in checks | as counted | 101.6 | 105.0 | 117.0 | 116.5 | +3.5 | -2.5 | +3.4 % | 11 / 9 / 0 | -111.0 | +117.0 | -19.9 to +27.1 | 0.850 | 20 |
| GPT-6 Luna | both, in checks | test by test (not counted) | 117.2 | 115.7 | 117.0 | 116.5 | -1.6 | -3.0 | -1.3 % | 12 / 8 / 0 | -18.0 | +23.0 | -6.0 to +3.2 | 0.542 | 20 |

Read off the tables above, the intervals of the mean pair difference that exclude
zero:

- **Runs 6 to 10:** 1 of the 14 intervals excludes zero: Luna's output tokens
  (+5,185, +399 to +10,860, p 0.078).
- **Runs 1 to 5:** 2 of the 14 intervals exclude zero: Haiku's uncached input
  tokens (-224, -656 to -4, p 0.021); Haiku's cache read tokens (-976,328,
  -1,771,318 to -237,857, p 0.037).
- **All ten runs:** 3 of the 14 intervals exclude zero: Luna's uncached input
  tokens (+22,330, +5,266 to +41,869, p 0.021); Luna's output tokens (+4,887,
  +1,451 to +8,458, p 0.013); Luna's API-equivalent USD (+0.0127 USD, +0.0015
  to +0.0263 USD, p 0.047).

An interval that excludes zero with a sign-flip p above 0.05 beside it, or the
reverse, is the two methods disagreeing at the edge on a small skewed sample;
both are printed and neither is leaned on. Every other interval includes zero.
That says the sample cannot tell the sign of that difference; it does not say
the difference is zero.

## Every pair's hidden checks, in run order

Counted, and test by test beside it. The two differ only where a grade hung.

**Claude Haiku 5.5** (native is Claude Code)

| Run | Case | Native | Aether | Aether minus native | Test by test: native | Aether | Aether minus native |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 1 | `matchbook` | 119 | 119 | +0 | 119 | 119 | +0 |
| 2 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 2 | `matchbook` | 119 | 117 | -2 | 119 | 117 | -2 |
| 3 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 3 | `matchbook` | 119 | 119 | +0 | 119 | 119 | +0 |
| 4 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 4 | `matchbook` | 119 | 119 | +0 | 119 | 119 | +0 |
| 5 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 5 | `matchbook` | 119 | 117 | -2 | 119 | 117 | -2 |
| 6 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 6 | `matchbook` | 117 | 119 | +2 | 117 | 119 | +2 |
| 7 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 7 | `matchbook` | 117 | 119 | +2 | 117 | 119 | +2 |
| 8 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 8 | `matchbook` | 119 | 117 | -2 | 119 | 117 | -2 |
| 9 | `gridcalc` | 153 | 154 | +1 | 153 | 154 | +1 |
| 9 | `matchbook` | 119 | 117 | -2 | 119 | 117 | -2 |
| 10 | `gridcalc` | 154 | 154 | +0 | 154 | 154 | +0 |
| 10 | `matchbook` | 119 | 117 | -2 | 119 | 117 | -2 |

**GPT-6 Luna** (native is Codex)

| Run | Case | Native | Aether | Aether minus native | Test by test: native | Aether | Aether minus native |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | `gridcalc` | 132 | 119 | -13 | 132 | 119 | -13 |
| 1 | `matchbook` | 94 | 110 | +16 | 94 | 110 | +16 |
| 2 | `gridcalc` | 121 | 108 | -13 | 121 | 108 | -13 |
| 2 | `matchbook` | 118 | 116 | -2 | 118 | 116 | -2 |
| 3 | `gridcalc` | 93 | 116 | +23 | 93 | 116 | +23 |
| 3 | `matchbook` | 118 | 119 | +1 | 118 | 119 | +1 |
| 4 | `gridcalc` | 140 | 128 | -12 | 140 | 128 | -12 |
| 4 | `matchbook` | 106 | 0 | -106 | 106 | 107 | +1 |
| 5 | `gridcalc` | 131 | 117 | -14 | 131 | 117 | -14 |
| 5 | `matchbook` | 0 | 104 | +104 | 110 | 104 | -6 |
| 6 | `gridcalc` | 138 | 140 | +2 | 138 | 140 | +2 |
| 6 | `matchbook` | 0 | 89 | +89 | 94 | 89 | -5 |
| 7 | `gridcalc` | 117 | 120 | +3 | 117 | 120 | +3 |
| 7 | `matchbook` | 114 | 107 | -7 | 114 | 107 | -7 |
| 8 | `gridcalc` | 110 | 126 | +16 | 110 | 126 | +16 |
| 8 | `matchbook` | 117 | 114 | -3 | 117 | 114 | -3 |
| 9 | `gridcalc` | 136 | 118 | -18 | 136 | 118 | -18 |
| 9 | `matchbook` | 0 | 117 | +117 | 109 | 117 | +8 |
| 10 | `gridcalc` | 136 | 133 | -3 | 136 | 133 | -3 |
| 10 | `matchbook` | 111 | 0 | -111 | 111 | 106 | -5 |

The counted and the test-by-test count differ in 3 trials of this sitting,
graded 0 as counted because the grader's one process timed out on a solution
that hangs: run 6, `native-luna-low` on `matchbook` (trial `7f6207f7b6`):
counted 0/119, test by test 94/119; run 9, `native-luna-low` on `matchbook`
(trial `b80bb26909`): counted 0/119, test by test 109/119; run 10,
`aether-single-luna-low` on `matchbook` (trial `fdcd88b4c4`): counted 0/119,
test by test 106/119. The 0 stands in every counted figure of this report.

## Beside the yardstick of the diagnosis

Iteration 5 of the
[diagnosis](harness-overhead-diagnosis-2026-10-10.md#iteration-5) ran the
identical Codex command twice in every block, 24 blocks on the same two cases,
and took the difference between the two. That is a yardstick: how far two
means of the same harness sit apart at that sample size, with nothing changed
between them. Those trials are diagnostic and none is counted. They are set
beside the counted Luna figures here, in separate columns, and are in no
counted figure.

| Measure | Yardstick, Codex minus Codex in one block (diagnostic, 23 or 24 pairs): mean and 95 % interval | Counted, Aether minus Codex, runs 6 to 10: mean and 95 % interval | Counted, all ten runs: mean and 95 % interval |
| --- | ---: | ---: | ---: |
| Uncached input tokens | -2,403 (-15,620 to +10,302; 23 pairs) | +22,309 (-859 to +52,699; 10 pairs) | +22,330 (+5,266 to +41,869; 20 pairs) |
| Output tokens | -1,604 (-3,961 to +740; 23 pairs) | +5,185 (+399 to +10,860; 10 pairs) | +4,887 (+1,451 to +8,458; 20 pairs) |
| Cache read tokens | +32,300 (-588,410 to +640,223; 23 pairs) | +865,792 (-220,211 to +2,555,110; 10 pairs) | +803,226 (-5,210 to +1,801,229; 20 pairs) |
| API-equivalent USD | -0.0007 (-0.0092 to +0.0074; 23 pairs) | +0.0135 (-0.0013 to +0.0360; 10 pairs) | +0.0127 (+0.0015 to +0.0263; 20 pairs) |
| Wall seconds | -79.3 (-199.9 to +21.2; 24 pairs) | +102.9 (-21.5 to +275.8; 10 pairs) | +46.6 (-32.6 to +146.7; 20 pairs) |
| Hidden checks as counted, in checks | -4.1 (-20.9 to +11.9; 24 pairs) | +8.5 (-26.2 to +45.1; 10 pairs) | +3.5 (-19.9 to +27.1; 20 pairs) |
| Hidden checks test by test, in checks | -4.9 (-16.8 to +3.9; 24 pairs) | -1.2 (-6.6 to +4.3; 10 pairs) | -1.6 (-6.0 to +3.2; 20 pairs) |

Read plainly: the yardstick's own interval for cost runs from -0.0092 to
+0.0074 USD, and the counted difference over all the version 2 runs is +0.0127
USD (+0.0015 to +0.0263). For hidden checks graded test by test the
yardstick's interval runs from -16.8 to +3.9 checks and the counted difference
is -1.6 checks (-6.0 to +3.2). The yardstick was measured on another day, with
23 or 24 pairs, one block at a time with other arms beside it, so the columns
are not one experiment. In that sitting two runs of the same native command
differed by a median of 0.0127 USD and by up to 0.0621 USD.

## From the session transcripts

`evals/harness_bench_transcript.py` reads, from each trial's own Claude Code
or Codex session log, figures a manifest does not hold. The logs were read
and not changed. "Before the first edit" runs through the response that
makes the first edit, so that response's own thinking is counted. The Luna
reasoning figures are what the Codex session log reports, which is 0 for
many sessions, as the diagnosis found; they are printed and not
interpreted. The 40 trials of this sitting:

| Model | Measure | Native mean | Aether mean | Aether minus native | Relative to native | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | Pairs read |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | First-request input tokens | 23,067 | 23,297 | +230 | +1.0 % | 0 / 10 / 0 | +220 | +238 | 10 of 10 |
| Claude Haiku 5.5 | Thinking or reasoning tokens before the first edit | 82,215 | 83,416 | +1,201 | +1.5 % | 6 / 4 / 0 | -16,887 | +23,021 | 10 of 10 |
| Claude Haiku 5.5 | Thinking or reasoning tokens in total | 90,602 | 92,115 | +1,513 | +1.7 % | 4 / 6 / 0 | -27,087 | +21,384 | 10 of 10 |
| Claude Haiku 5.5 | Requests | 18.8 | 19.4 | +0.6 | +3.2 % | 5 / 5 / 0 | -9 | +17 | 10 of 10 |
| GPT-6 Luna | First-request input tokens | 15,606 | 16,528 | +922 | +5.9 % | 0 / 10 / 0 | +915 | +932 | 10 of 10 |
| GPT-6 Luna | Thinking or reasoning tokens before the first edit | 0 | 0 | +0 |  | 0 / 0 / 8 | +0 | +0 | 8 of 10 |
| GPT-6 Luna | Thinking or reasoning tokens in total | 1,119 | 1,086 | -33 | -2.9 % | 3 / 3 / 4 | -4,357 | +6,287 | 10 of 10 |
| GPT-6 Luna | Requests | 30.4 | 40.4 | +10.0 | +32.9 % | 6 / 4 / 0 | -29 | +112 | 10 of 10 |

- **Haiku.** The first request is larger through Aether in 10 of 10 pairs
  read, by 220 to 238 tokens. Requests per session: 19.4 through Aether
  against 18.8 natively, more in 5 of 10 pairs and fewer in 5.
- **Luna.** The first request is larger through Aether in 10 of 10 pairs read,
  by 915 to 932 tokens. Requests per session: 40.4 through Aether against 30.4
  natively, more in 4 of 10 pairs and fewer in 6.
- **Unknown.** 2 of 40 trials have a figure the reader could not give:
  `d547d225ff` (run 9, `aether-single-luna-low` on `gridcalc`: thinking before
  the first edit); `99e174adb3` (run 10, `native-luna-low` on `matchbook`:
  thinking before the first edit). Where the first edit is unknown the reader
  found no file-change event in the session log, so it has no first edit to
  count up to. Such a pair is left out of that row of the table.

These figures describe the sessions; they are not a second measurement of
cost. The size of the first request is what the session starts with and is
near constant per cell. The number of requests is the solver's own course
through the task and moves from trial to trial on both sides.

Per trial:

| Run | Case | Cell | Trial | First-request input tokens | Thinking or reasoning before the first edit | Thinking or reasoning in total | Requests |
| ---: | --- | --- | --- | ---: | ---: | ---: | ---: |
| 6 | `gridcalc` | `native-haiku-low` | `35112ca29d` | 23,048 | 118,282 | 142,044 | 25 |
| 6 | `gridcalc` | `aether-single-haiku-low` | `739abb2e5e` | 23,276 | 101,395 | 114,957 | 19 |
| 6 | `gridcalc` | `native-luna-low` | `32c0004923` | 15,593 | 0 | 0 | 42 |
| 6 | `gridcalc` | `aether-single-luna-low` | `a79e25b4b7` | 16,520 | 0 | 6,287 | 30 |
| 6 | `matchbook` | `native-haiku-low` | `d7ba54fd35` | 23,086 | 61,931 | 63,958 | 13 |
| 6 | `matchbook` | `aether-single-haiku-low` | `fd9e5b2e91` | 23,324 | 59,862 | 68,946 | 12 |
| 6 | `matchbook` | `native-luna-low` | `7f6207f7b6` | 15,624 | 0 | 0 | 21 |
| 6 | `matchbook` | `aether-single-luna-low` | `ac6d9befec` | 16,539 | 0 | 0 | 133 |
| 7 | `gridcalc` | `native-haiku-low` | `e9a3c2ddb8` | 23,056 | 98,049 | 105,205 | 22 |
| 7 | `gridcalc` | `aether-single-haiku-low` | `3de604dea2` | 23,276 | 101,962 | 110,473 | 18 |
| 7 | `gridcalc` | `native-luna-low` | `2d5adf966f` | 15,597 | 0 | 0 | 28 |
| 7 | `gridcalc` | `aether-single-luna-low` | `24d1193d23` | 16,514 | 0 | 3,400 | 26 |
| 7 | `matchbook` | `native-haiku-low` | `20037881b7` | 23,080 | 59,564 | 65,470 | 13 |
| 7 | `matchbook` | `aether-single-haiku-low` | `28a6c4e397` | 23,311 | 52,417 | 53,676 | 14 |
| 7 | `matchbook` | `native-luna-low` | `c72f98c6a2` | 15,622 | 0 | 0 | 54 |
| 7 | `matchbook` | `aether-single-luna-low` | `9988a7e1eb` | 16,543 | 0 | 0 | 25 |
| 8 | `gridcalc` | `native-haiku-low` | `22cf7454ed` | 23,050 | 100,986 | 116,091 | 22 |
| 8 | `gridcalc` | `aether-single-haiku-low` | `170c6b6d58` | 23,284 | 124,007 | 135,811 | 27 |
| 8 | `gridcalc` | `native-luna-low` | `4907af3368` | 15,593 | 0 | 0 | 40 |
| 8 | `gridcalc` | `aether-single-luna-low` | `ef10cdac76` | 16,515 | 0 | 0 | 38 |
| 8 | `matchbook` | `native-haiku-low` | `8f58cfaad2` | 23,086 | 55,681 | 56,779 | 10 |
| 8 | `matchbook` | `aether-single-haiku-low` | `2ab069fabc` | 23,314 | 75,111 | 78,163 | 14 |
| 8 | `matchbook` | `native-luna-low` | `6657608932` | 15,616 | 0 | 4,357 | 27 |
| 8 | `matchbook` | `aether-single-luna-low` | `9b8e7d51b0` | 16,548 | 0 | 0 | 28 |
| 9 | `gridcalc` | `native-haiku-low` | `f913693e87` | 23,046 | 121,552 | 136,585 | 30 |
| 9 | `gridcalc` | `aether-single-haiku-low` | `9a63f33fff` | 23,280 | 112,619 | 139,101 | 47 |
| 9 | `gridcalc` | `native-luna-low` | `4471900366` | 15,591 | 0 | 1,625 | 29 |
| 9 | `gridcalc` | `aether-single-luna-low` | `d547d225ff` | 16,507 | unknown | 0 | 25 |
| 9 | `matchbook` | `native-haiku-low` | `7b15ad1fbf` | 23,088 | 55,975 | 59,077 | 15 |
| 9 | `matchbook` | `aether-single-haiku-low` | `5c161bdf36` | 23,317 | 47,876 | 49,603 | 8 |
| 9 | `matchbook` | `native-luna-low` | `b80bb26909` | 15,616 | 0 | 5,207 | 29 |
| 9 | `matchbook` | `aether-single-luna-low` | `4619e92baf` | 16,539 | 0 | 1,085 | 17 |
| 10 | `gridcalc` | `native-haiku-low` | `f950e031d9` | 23,048 | 100,216 | 106,689 | 25 |
| 10 | `gridcalc` | `aether-single-haiku-low` | `f51b8f1997` | 23,279 | 96,374 | 105,078 | 16 |
| 10 | `gridcalc` | `native-luna-low` | `6522ff1b00` | 15,595 | 0 | 0 | 21 |
| 10 | `gridcalc` | `aether-single-luna-low` | `faf7cb0bc9` | 16,515 | 0 | 0 | 54 |
| 10 | `matchbook` | `native-haiku-low` | `3b500b40f2` | 23,084 | 49,918 | 54,121 | 13 |
| 10 | `matchbook` | `aether-single-haiku-low` | `fc772840cc` | 23,309 | 62,540 | 65,343 | 19 |
| 10 | `matchbook` | `native-luna-low` | `99e174adb3` | 15,618 | unknown | 0 | 13 |
| 10 | `matchbook` | `aether-single-luna-low` | `fdcd88b4c4` | 16,543 | 0 | 88 | 28 |

## Timeouts, failures, guard firings and unknown figures

- **Timeouts and unfinished workflows:** 0 of 40 trials. Every workflow
  finished.
- **Guard firings:** 2.
- guard: at 09:24:07 UTC it ended a process of trial `b80bb26909` (run 9,
  `native-luna-low` on `matchbook`) that held 4.23 GB: `python -`, started by
  the solver in its workspace. The trial is counted as it stands: workflow
  completed, 0/119 hidden checks, 187.4 seconds.
- guard: at 09:24:19 UTC it ended a process of trial `b80bb26909` (run 9,
  `native-luna-low` on `matchbook`) that held 4.25 GB: `python -`, started by
  the solver in its workspace. The trial is counted as it stands: workflow
  completed, 0/119 hidden checks, 187.4 seconds.
- **Unknown or partial token and cost figures:** 0 of 40 trials. Every priced
  figure is as the comparison tool prints it above; no figure is a lower
  bound.
- **Transcripts the reader could not read:** 0.
- **Memory.** Available memory was sampled every 20 seconds. Lowest per run:
  run 6 6,340 MB at 08:46:04 UTC, run 7 6,292 MB at 09:01:59 UTC, run 8 6,144
  MB at 09:11:34 UTC, run 9 5,015 MB at 09:24:09 UTC, run 10 6,829 MB at
  09:37:36 UTC.
- **Closest to the timeout:** `native-haiku-low` on `gridcalc` in run 6, 893.3
  of 1200 seconds; workflow completed, 154/154 hidden checks.

Every trial that is graded 0, has an unfinished workflow, a test-by-test grade
that differs from the counted one, an unknown figure or a recorded violation:

- graded 0/119: `native-luna-low` on `matchbook` in run 6 (`7f6207f7b6`); grader said: ['hidden tests timed out after 60s']; test by test: 94 passed, 24 failed, 1 hung, 0 not run (one hidden test per process)
- graded 0/119: `native-luna-low` on `matchbook` in run 9 (`b80bb26909`); grader said: ['hidden tests timed out after 60s']; test by test: 109 passed, 8 failed, 2 hung, 0 not run (one hidden test per process)
- graded 0/119: `aether-single-luna-low` on `matchbook` in run 10 (`fdcd88b4c4`); grader said: ['hidden tests timed out after 60s']; test by test: 106 passed, 12 failed, 1 hung, 0 not run (one hidden test per process)

## Limits

- **Few runs.** 5 runs per cell and case give 10 pairs per model in this
  sitting, and 10 runs give 20 over both sittings. That is still a small
  sample. Every mean here is a mean of that many trials, and one trial moves
  it visibly.
- **The intervals and p-values are descriptive.** They were planned before
  the runs, and they are 14 intervals per grouping. One of them excluding
  zero is what chance alone does now and then. The bootstrap interval of a
  mean of 10 or 20 skewed pair differences is itself rough: it tends to be
  too narrow.
- **Two sittings are put into one figure, on a rule written first.** The
  binary, the counted command lines, the prompts, the cases and the CLI
  versions are the same in both. The day, the provider's load, the cache
  state and the other work on the machine are not. Pairs are formed only
  inside one run, so a difference between the days shifts both sides of a
  pair together, but it is not ruled out that the model behind a name
  changed between the sittings.
- **The first measurement is not pooled.** It ran under protocol version 1
  on another binary; the tools refuse to merge manifests of two protocol
  versions.
- **Counted and diagnostic trials are never merged.** The diagnosis's
  trials appear here only as a yardstick set beside the counted figures.
- **One machine.** Provider load, cache warmth and the other work on this
  host are not controlled. Other agents used the same machine and the same
  subscriptions while the trials ran.
- **Wall time is a paired figure.** All 8 trials of a run ran at once
  (`--jobs 8`), so each trial's wall time is time under shared load. It is
  comparable within a pair and within these runs. It is not an absolute
  latency and must not be compared with sequential results.
- **Effort is requested, not always confirmed.** Codex records the effective
  effort in its session file; the comparison tool's output above says for
  each Luna side whether session metadata confirmed it. Claude Code receipts
  and Aether's `engine.session` events do not report effort, so for both
  Haiku cells "low" is what was asked for.
- **Dollars are API-equivalent, not charges.** The trials ran on
  subscriptions. Haiku figures are the dollar value the Claude CLI reports
  for the session (on the Aether side from the same CLI's process receipts).
  Luna figures are list-price arithmetic on the token counts at the rates in
  the pinned binary's inventory (USD 0.10 input, 0.50 output, 0.01 cache read
  per million tokens). The tool labels the native side "estimated" and the
  Aether side "billed" because there the dollar figure arrives with Aether's
  usage events; it equals the same list-price arithmetic.
- **Token scope.** Native figures are the CLI's own receipt. Aether Haiku
  figures are the Claude process receipts captured for the trial's session.
  Aether Luna figures are Aether's durable usage events for the agent, which
  may omit helper-model work the Codex CLI does not report to Aether.
- **The Aether session is not the native session plus a wrapper.** Aether
  starts the same CLI with its own settings, its coordination tools over
  MCP, the task inside its own first message, and the CLI's own delegation
  tools switched off. A difference can come from any of these, or from
  chance. These runs do not say which.
- **The benchmark's Aether is not every operator's Aether.** Outside the
  benchmark an Aether agent on Claude Code still loads the operator's own
  MCP connectors. Version 2 keeps them out so that the pair starts equal,
  not because Aether does.
- **The grader's one limit.** All hidden tests of a case run in one process
  with 60 seconds; a solution that hangs in one test is graded 0. The
  counted figures keep that 0. The test-by-test grade beside it is not the
  counted grade.
- **Two cases, one kind of task.** Both are Python repair tasks with a public
  contract. Nothing here speaks to delegation, long sessions, compaction or
  other languages.
- **Ceilings.** Where both sides of a pair pass every check, the pair can
  show a loss and cannot show a gain, so equal scores there are not evidence
  that nothing is lost.

## Reproduce

Live runs spend model tokens on the logged-in Claude and Codex accounts.

```sh
cargo build --release
python3 -m unittest discover -s evals -p 'test_harness_bench*.py'   # no model calls

# One counted run: the matched set of 8 trials at once. Repeat with seeds 6 to 10 and its own run-<n> directory.
python3 evals/harness_bench.py --cases matchbook,gridcalc --runs 1 --seed 6 --jobs 8 --timeout 1200 \
  --conditions native-haiku-low,aether-single-haiku-low,native-luna-low,aether-single-luna-low \
  --binary target/release/aether --output-dir target/harness-393-v2b/run-6

# The tables and the per-trial list of this sitting.
python3 evals/harness_bench_compare.py target/harness-393-v2b/run-*/harness-bench-*.json --trials

# All ten runs: give the five manifests of the second measurement first.
python3 evals/harness_bench_compare.py target/harness-393-v2/run-*/harness-bench-*.json \
  target/harness-393-v2b/run-*/harness-bench-*.json

# The figures from the session transcripts.
python3 evals/harness_bench_transcript.py target/harness-393-v2b/run-*/harness-bench-*.json
```

The manifests and per-trial artifacts are ignored local files under
`target/`; this report carries the per-trial table. The manifests of both
sittings were also kept outside the working tree, together with the analysis
script that computes the statistics above from them (paired bootstrap with
seed 20261011, exact sign-flip test), its output, the reader's output and
the guard's log, so every table here can be recomputed. A new run gives new
numbers: the models are not deterministic, and provider load differs.
