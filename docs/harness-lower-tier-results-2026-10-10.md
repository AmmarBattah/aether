# Lower-tier harness comparison: Claude Haiku 5.5 and GPT-6 Luna, 2026-10-10

> **A later measurement exists.** The same comparison was made again later
> the same day under protocol version 2:
> [harness-lower-tier-results-2026-10-10-protocol-2.md](harness-lower-tier-results-2026-10-10-protocol-2.md).
> Its figures differ from the ones below for two located reasons, besides
> chance and a different sitting. In the runs below, Aether's first message
> told the solver its effort in words ("Effort low."), which the diagnosis
> measured to lower Haiku's thinking; and only the Aether side of the Haiku
> pair loaded the machine user's MCP connectors, so the two sides started
> with unequal MCP sets. Both are described in the
> [overhead diagnosis](harness-overhead-diagnosis-2026-10-10.md). The
> figures and tables below are the first measurement as it was reported and
> are not changed.

Does running a cheap model through Aether cost accuracy, tokens, money or
time against running it in its native harness? This report gives five counted
runs of a matched comparison on two frozen cases, 40 trials in all. Run 5 was
added after runs 1 to 4 had been seen, at a reader's request, and is counted
whole. It is a small sample from one machine and makes no significance claim.

## Summary

Means per trial. Each row is 10 trials: 5 runs of 2 cases.

| Model, requested effort | Harness | Workflows finished | Hidden checks passed | Uncached input tokens | Output tokens | Cache read tokens | API-equivalent USD | Wall seconds |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5, low | Claude Code | 10/10 | 99.9 % | 801 | 120,889 | 2,263,645 | 0.3396 | 516.3 |
| Claude Haiku 5.5, low | Aether | 10/10 | 97.9 % | 32 | 95,017 | 1,705,126 | 0.2395 | 410.6 |
| | Aether minus native, 10 pairs | | -2.0 pp | -768 | -25,872 | -558,519 | -0.1001 | -105.7 |
| GPT-6 Luna, low | Codex | 10/10 | 83.6 % | 63,428 | 15,432 | 1,741,670 | 0.0315 | 181.8 |
| GPT-6 Luna, low | Aether | 10/10 | 82.2 % | 92,508 | 18,177 | 2,267,008 | 0.0410 | 246.2 |
| | Aether minus native, 10 pairs | | -1.4 pp | +29,080 | +2,745 | +525,338 | +0.0095 | +64.4 |

Hidden checks passed per case, mean of 5 runs with every run's count:

| Model | Harness | `matchbook` (119 checks) | `gridcalc` (154 checks) |
| --- | --- | --- | --- |
| Claude Haiku 5.5 | Claude Code | 100.0 %: 119, 119, 119, 119, 119 | 99.9 %: 154, 153, 154, 154, 154 |
| Claude Haiku 5.5 | Aether | 96.0 %: 119, 119, 95, 119, 119 | 99.9 %: 154, 154, 154, 154, 153 |
| GPT-6 Luna | Codex | 85.4 %: 95, 80, 111, 104, 118 | 81.8 %: 129, 107, 132, 119, 143 |
| GPT-6 Luna | Aether | 87.9 %: 111, 99, 117, 96, 100 | 76.5 %: 94, 120, 144, 115, 116 |

Counts are in run order, 1 to 5.

The five-run figures in this summary are computed from the 40 rows of the
per-trial table under "Every trial", because the manifests of runs 1 to 4
were local files and were not kept. A mean of USD therefore carries that
table's rounding to four decimals, and a mean of wall seconds its rounding to
one. The four-run figures quoted below are the ones the report tool printed
from the manifests while they existed.

What these numbers show, and what they do not:

- **Where Aether is behind.** With Luna, on accuracy and on every consumption
  measure in the mean. Accuracy: 82.2 % against 83.6 %, a paired difference
  of -1.4 points, lower in 5 of the 10 pairs and higher in 5; on `gridcalc`
  alone 76.5 % against 81.8 %. Consumption: uncached input +29,080 tokens
  (92,508 against 63,428), output +2,745, cache read +525,338,
  API-equivalent cost +0.0095 USD per trial (0.0410 against 0.0315) and wall
  time +64.4 seconds (246.2 against 181.8). With Haiku, on accuracy: 97.9 %
  against 99.9 %, a paired difference of -2.0 points. One trial accounts for
  it (`matchbook`, run 3: 95/119 through Aether, 119/119 native). The other
  two unequal Haiku pairs are one `gridcalc` check each and cancel: in run 5
  Aether passed 153/154 and native 154/154, in run 2 the reverse.
- **Where Aether is ahead.** With Haiku, on every consumption measure in the
  mean: output tokens -25,872, cache read -558,519, cost -0.1001 USD per
  trial (0.2395 against 0.3396) and wall time -105.7 seconds (410.6 against
  516.3); Aether was cheaper and faster in 8 of the 10 pairs and dearer and
  slower in 2 (`gridcalc`, run 4, and `matchbook`, run 5). The Haiku
  uncached input difference (-768: 32 against 801) is one native trial:
  `gridcalc`, run 5, reported 7,666 uncached input tokens, and the other 19
  Haiku trials reported 16 to 62. With Luna, on accuracy on `matchbook`
  alone: 87.9 % against 85.4 %.
- **The Luna consumption means rest on one trial.** `matchbook`, run 4,
  through Aether took 884.0 seconds, 279,290 uncached input tokens and 0.1411
  USD, against 62.4 seconds, 43,336 tokens and 0.0114 USD for its native
  partner. That one pair's cost difference (+0.1297 USD) is larger than the
  sum over all 10 pairs (+0.0953 USD). It is a counted trial and stays in the
  mean. Across the 10 Luna pairs Aether was cheaper in 4 and dearer in 6,
  faster in 6 and slower in 4.
- **The spread between runs is larger than the difference between
  harnesses.** The same Luna cell on the same case ranged from 94 to 144 of
  154 checks through Aether and from 107 to 143 natively. The Luna accuracy
  difference was +2.3 points over runs 1 to 4 and is -1.4 points over runs 1
  to 5: one added run changed its sign. With 5 runs per case, no difference
  above is established, in either direction.
- **The accuracy ceiling.** The cases were meant to leave no score at 100 %.
  That held for GPT-6 Luna: none of its 20 trials passed every check on
  either case (best 144/154 and 118/119). It did not hold for Claude Haiku
  5.5: 17 of its 20 trials passed every check, 9 of 10 native and 8 of 10
  through Aether. For Haiku, accuracy on these cases can show a loss but
  hardly a gain, so the near-equal scores are not evidence that nothing is
  lost; apart from the one low trial, only tokens, cost and wall time tell
  the Haiku cells apart. The report tool prints its ceiling note only when
  every run of both sides of a pair is perfect on a case. Over five runs
  that happened on neither case; in run 5 alone it happened on `matchbook`,
  and the tool's run 5 table below prints the note.
- **No timeout, no failed workflow, no unknown figure.** All 40 workflows
  finished inside the 1200-second limit, every token receipt was complete,
  and every trial was priced.

## What run 5 changed

Runs 1 to 4 were reported first. A reader then asked for one more run, to see
whether the differences were chance, and for the mean over all of them. Run 5
is that run: the same settings with seed 5, made after the four-run results
had been seen, and counted whole whatever it showed. Its rule was written
down before it started (rule 7 under "Counting rule"), and no further run was
made after it.

Means per trial and paired differences, over runs 1 to 4 (8 trials per cell,
8 pairs) and over runs 1 to 5 (10 trials per cell, 10 pairs):

Claude Haiku 5.5, low:

| Measure, mean per trial | Runs 1 to 4: Claude Code | Aether | Aether minus native | Runs 1 to 5: Claude Code | Aether | Aether minus native |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Hidden checks passed | 99.9 % | 97.5 % | -2.4 pp | 99.9 % | 97.9 % | -2.0 pp |
| Uncached input tokens | 40 | 32 | -7 | 801 | 32 | -768 |
| Output tokens | 118,887 | 95,584 | -23,303 | 120,889 | 95,017 | -25,872 |
| Cache read tokens | 2,261,965 | 1,732,993 | -528,972 | 2,263,645 | 1,705,126 | -558,519 |
| API-equivalent USD | 0.3418 | 0.2428 | -0.0991 | 0.3396 | 0.2395 | -0.1001 |
| Wall seconds | 500.7 | 416.6 | -84.0 | 516.3 | 410.6 | -105.7 |

- Accuracy: Aether is still behind, and the difference shrank from -2.4 to
  -2.0 points. It shrank because the one low trial of run 3 is now one of 10
  instead of one of 8. Run 5 itself went against Aether by one check: 153/154
  on `gridcalc` against 154/154 native, with `matchbook` equal at 119/119.
- Tokens, cost and time: Aether is still ahead, and the difference grew on
  each measure (cost from -0.0991 to -0.1001 USD, wall time from -84.0 to
  -105.7 seconds). In run 5 that came from `gridcalc`, where the native trial
  was the longest and dearest of all 40 (884.3 seconds, 0.5967 USD, against
  487.2 seconds and 0.3540 USD through Aether). On `matchbook` in run 5
  Aether was dearer and slower: 0.0991 against 0.0651 USD and 285.7 against
  273.9 seconds. The jump in uncached input (from -7 to -768) is that one
  native `gridcalc` trial's 7,666 tokens.

GPT-6 Luna, low:

| Measure, mean per trial | Runs 1 to 4: Codex | Aether | Aether minus native | Runs 1 to 5: Codex | Aether | Aether minus native |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Hidden checks passed | 80.5 % | 82.8 % | +2.3 pp | 83.6 % | 82.2 % | -1.4 pp |
| Uncached input tokens | 70,147 | 101,035 | +30,888 | 63,428 | 92,508 | +29,080 |
| Output tokens | 16,092 | 18,434 | +2,343 | 15,432 | 18,177 | +2,745 |
| Cache read tokens | 2,048,672 | 2,547,936 | +499,264 | 1,741,670 | 2,267,008 | +525,338 |
| API-equivalent USD | 0.0355 | 0.0448 | +0.0093 | 0.0315 | 0.0410 | +0.0095 |
| Wall seconds | 197.8 | 267.8 | +70.0 | 181.8 | 246.2 | +64.4 |

- Accuracy: the difference changed sign, from +2.3 points in Aether's favour
  to -1.4 points against it. In run 5 the native side had its best score of
  any run on both cases (118/119 and 143/154) and Aether was lower on both
  (100/119 and 116/154), a difference of -16.3 points in that run alone.
- Tokens, cost and time: Aether is still behind on every measure. The
  difference shrank on uncached input (from +30,888 to +29,080 tokens) and on
  wall time (from +70.0 to +64.4 seconds), and grew on output tokens (from
  +2,343 to +2,745), cache read (from +499,264 to +525,338) and cost (from
  +0.0093 to +0.0095 USD). In run 5 Aether was dearer in both Luna pairs,
  slower on `gridcalc` (230.7 against 141.8 seconds) and faster on
  `matchbook` (88.0 against 93.1 seconds).

One more run cannot settle whether a difference is noise, and this one does
not. It moved one difference across zero and left the others with their
signs, which is what chance alone could also do with a sample this small:
five runs per case, with one cell ranging from 94 to 144 of 154 checks
between runs.
What would settle it is many more runs per cell and case, their number fixed
before the first one is seen. No significance is claimed for any figure here.

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
team of agents.

The two cases were validated, piloted once and frozen before any trial here
(see the [protocol](../evals/harness_bench_protocol.md)):

| Case | Hidden checks | Unmodified starter passes | Frozen digest |
| --- | ---: | ---: | --- |
| `matchbook` (limit order book with a journal and iceberg orders) | 119 | 23 | `7f719f2d3e0b7c98228f9bdf9a922aea93b605cbec9cb3b46535fe66ab8d630a` |
| `gridcalc` (spreadsheet engine with formulas and structural edits) | 154 | 5 | `df35116f6492541fbdc8078c26355647483ff321332b91bb7ab3c9c975dd4758` |

The digests were recomputed with `fixture_digest()` before the first trial
and again before run 5, and equal the frozen values. Every manifest records
the hash of each starter and hidden-check file, and every trial rechecks them
before and after it runs.

## How it was run

| | |
| --- | --- |
| Aether binary | `aether 0.1.1`, `cargo build --release`, SHA-256 `d687547b7c7b55a770927af54ef434a1fe25e004e488a31c3dd762b771608512` in all five runs |
| Built from, runs 1 to 4 | commit `6a515fc412c0016e9d60b86dfe9c7e47e39fc94d`. The manifests record commit `4b90abf9365f766346d06b4c2c6ecca70f799db3` with a clean tree: it adds only the report's per-trial table and changes no Rust source |
| Built from, run 5 | a new build from commit `28d2643955a30d922a07b0436206f52e08d64560`, which changes no Rust source against `6a515fc4`; its SHA-256 equals that of the binary of runs 1 to 4. The manifest records commit `f2bcdb2c1a02ea63f944511f92860c3fd91760ea` with a clean tree: it adds only rule 7 of the counting rule to this report |
| Native CLIs | `claude` 2.1.294 (Claude Code), `codex-cli` 0.162.1, the same versions in all five runs |
| Cells | `native-haiku-low`, `aether-single-haiku-low`, `native-luna-low`, `aether-single-luna-low` |
| Cases | `matchbook`, `gridcalc` |
| Runs | 5, one runner invocation each, `--runs 1`, seeds 1 to 5. Each is the matched set of 8 trials. Run 5 was added after runs 1 to 4 had been seen |
| Trials at once | `--jobs 8`: all 8 trials of a run in one block, so both sides of every pair ran under the same load |
| Timeout | `--timeout 1200` seconds for every trial on both sides |
| Machine | one Linux host with 12 hardware threads and 15 GB of memory, shared with other work |

`--jobs 8` instead of 4 was chosen before the first counted run and kept for
all of them: more than 6 GB of memory was available before every run, the
four-at-once smoke had left more than 6 GB available, and the lowest reading
during any counted run was 5,167 MB (sampled every 20 seconds). The timeout
of 1200 seconds instead of the default 1800 was chosen from the calibration
pilot, whose longest trial took 670 seconds, so that a run could not outlast
its slot; the longest counted trial took 884 seconds. Run 5 kept both
settings.

Runs were launched one after another, never side by side: runs 1 to 4 back
to back, run 5 about 23 minutes after run 4 ended. Within a run all 8 trials
start together; the seed sets the launch order in the block and which side
of a pair is launched first.

Per run (times in UTC on 2026-10-10; load average is 1, 5 and 15 minutes):

| Run | Seed | `--jobs` | `--timeout` (s) | Start (UTC) | End (UTC) | Load average at start | Load average at end | Lowest available memory (MB) | Manifest |
| --- | ---: | ---: | ---: | --- | --- | --- | --- | ---: | --- |
| 1 | 1 | 8 | 1200 | 10:36:33 | 10:47:03 | 1.86 1.81 1.74 | 1.40 1.70 1.79 | 5167 | `target/harness-393/run-1/harness-bench-2026-10-10T103634Z.json` |
| 2 | 2 | 8 | 1200 | 10:47:12 | 10:58:29 | 1.34 1.68 1.78 | 3.67 2.30 2.07 | 5348 | `target/harness-393/run-2/harness-bench-2026-10-10T104714Z.json` |
| 3 | 3 | 8 | 1200 | 10:58:36 | 11:09:18 | 3.45 2.28 2.07 | 2.49 2.30 2.23 | 5326 | `target/harness-393/run-3/harness-bench-2026-10-10T105837Z.json` |
| 4 | 4 | 8 | 1200 | 11:09:26 | 11:24:12 | 2.53 2.31 2.23 | 1.23 1.52 1.86 | 6219 | `target/harness-393/run-4/harness-bench-2026-10-10T110927Z.json` |
| 5 | 5 | 8 | 1200 | 11:47:00 | 12:01:46 | 5.18 3.95 2.41 | 1.35 1.24 1.72 | 6121 | `target/harness-393/run-5/harness-bench-2026-10-10T114701Z.json` |

The load average at the start of run 5 is higher than for runs 1 to 4: the
release build for it had ended less than a minute earlier. Before run 5,
7,109 MB of memory was available and 39 GB of disk was free.

## Results per model pair, runs 1 to 4

Printed by `evals/harness_bench_compare.py --trials` from the four manifests
of runs 1 to 4, unedited, before run 5 was made. These are four-run figures;
the five-run figures are in the Summary.

### haiku-low

- Native: `claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Aether: `claude-code/claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Claude all-model process receipts
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 8 | 8 | |
| Workflows finished | 8/8 | 8/8 | |
| Hidden checks passed, mean | 99.9 % (8/8) | 97.5 % (8/8) | -2.4 pp (8 pairs) |
| Hidden checks per run | gridcalc: 154/154, 153/154, 154/154, 154/154; matchbook: 119/119, 119/119, 119/119, 119/119 | gridcalc: 154/154, 154/154, 154/154, 154/154; matchbook: 119/119, 119/119, 95/119, 119/119 | |
| Uncached input tokens, mean | 40 (8/8) | 32 (8/8) | -7 (8 pairs) |
| Output tokens, mean | 118,887 (8/8) | 95,584 (8/8) | -23,303 (8 pairs) |
| Cache read tokens, mean | 2,261,965 (8/8) | 1,732,993 (8/8) | -528,972 (8 pairs) |
| Cache write tokens, mean | 145,217 (8/8) | 125,122 (8/8) | -20,095 (8 pairs) |
| API-equivalent USD, mean | 0.3418 (billed; 8/8 priced) | 0.2428 (billed; 8/8 priced) | -0.0991 (8 pairs) |
| Wall seconds, mean (min to max) | 500.7 (283.6 to 674.5; 8/8) | 416.6 (217.0 to 692.8; 8/8) | -84.0 (8 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

8 native and 8 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

### luna-low

- Native: `gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Aether: `codex/gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Aether durable (may omit helpers)
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 8 | 8 | |
| Workflows finished | 8/8 | 8/8 | |
| Hidden checks passed, mean | 80.5 % (8/8) | 82.8 % (8/8) | +2.3 pp (8 pairs) |
| Hidden checks per run | gridcalc: 129/154, 107/154, 132/154, 119/154; matchbook: 95/119, 80/119, 111/119, 104/119 | gridcalc: 94/154, 120/154, 144/154, 115/154; matchbook: 111/119, 99/119, 117/119, 96/119 | |
| Uncached input tokens, mean | 70,147 (8/8) | 101,035 (8/8) | +30,888 (8 pairs) |
| Output tokens, mean | 16,092 (8/8) | 18,434 (8/8) | +2,343 (8 pairs) |
| Cache read tokens, mean | 2,048,672 (8/8) | 2,547,936 (8/8) | +499,264 (8 pairs) |
| Cache write tokens, mean | 0 (8/8) | 0 (8/8) | +0 (8 pairs) |
| API-equivalent USD, mean | 0.0355 (estimated; 8/8 priced) | 0.0448 (billed; 8/8 priced) | +0.0093 (8 pairs) |
| Wall seconds, mean (min to max) | 197.8 (62.4 to 386.5; 8/8) | 267.8 (56.5 to 884.0; 8/8) | +70.0 (8 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

8 native and 8 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

Dollar values are API-equivalent (reported metadata or list-price estimates), not subscription charges. A mean covers only the trials that measured it: a token or cost figure counts as measured only when every unit of its accounting scope reported it at a terminal point. A pair is one case and run within one manifest; trials from different manifests are never paired, so run both sides of a pair in the same block.

## Results per model pair, run 5 alone

Printed by `evals/harness_bench_compare.py --trials` from the manifest of
run 5, unedited. Each side of a pair is 2 trials here, one per case.

### haiku-low

- Native: `claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Aether: `claude-code/claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Claude all-model process receipts
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 2 | 2 | |
| Workflows finished | 2/2 | 2/2 | |
| Hidden checks passed, mean | 100.0 % (2/2) | 99.7 % (2/2) | -0.3 pp (2 pairs) |
| Hidden checks per run | gridcalc: 154/154; matchbook: 119/119 | gridcalc: 153/154; matchbook: 119/119 | |
| Uncached input tokens, mean | 3,845 (2/2) | 33 (2/2) | -3,812 (2 pairs) |
| Output tokens, mean | 128,898 (2/2) | 92,748 (2/2) | -36,149 (2 pairs) |
| Cache read tokens, mean | 2,270,365 (2/2) | 1,593,659 (2/2) | -676,706 (2 pairs) |
| Cache write tokens, mean | 161,586 (2/2) | 120,822 (2/2) | -40,765 (2 pairs) |
| API-equivalent USD, mean | 0.3309 (billed; 2/2 priced) | 0.2266 (billed; 2/2 priced) | -0.1043 (2 pairs) |
| Wall seconds, mean (min to max) | 579.1 (273.9 to 884.3; 2/2) | 386.5 (285.7 to 487.2; 2/2) | -192.6 (2 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

2 native and 2 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

Accuracy is at the ceiling on matchbook: every run of both sides passed every hidden check, so this pair can show a loss there but not a gain.

### luna-low

- Native: `gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Aether: `codex/gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)
- Native token and cost accounting: native receipt
- Aether token and cost accounting: Aether durable (may omit helpers)
- Trials at once (`--jobs`): 8. With more than one, wall time is a paired comparison under shared load, not an absolute figure.

| Measure | Native | Aether | Aether minus native |
| --- | --- | --- | --- |
| Trials | 2 | 2 | |
| Workflows finished | 2/2 | 2/2 | |
| Hidden checks passed, mean | 96.0 % (2/2) | 79.7 % (2/2) | -16.3 pp (2 pairs) |
| Hidden checks per run | gridcalc: 143/154; matchbook: 118/119 | gridcalc: 116/154; matchbook: 100/119 | |
| Uncached input tokens, mean | 36,550 (2/2) | 58,398 (2/2) | +21,848 (2 pairs) |
| Output tokens, mean | 12,792 (2/2) | 17,148 (2/2) | +4,356 (2 pairs) |
| Cache read tokens, mean | 513,664 (2/2) | 1,143,296 (2/2) | +629,632 (2 pairs) |
| Cache write tokens, mean | 0 (2/2) | 0 (2/2) | +0 (2 pairs) |
| API-equivalent USD, mean | 0.0152 (estimated; 2/2 priced) | 0.0258 (billed; 2/2 priced) | +0.0107 (2 pairs) |
| Wall seconds, mean (min to max) | 117.5 (93.1 to 141.8; 2/2) | 159.4 (88.0 to 230.7; 2/2) | +41.9 (2 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

2 native and 2 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

Dollar values are API-equivalent (reported metadata or list-price estimates), not subscription charges. A mean covers only the trials that measured it: a token or cost figure counts as measured only when every unit of its accounting scope reported it at a terminal point. A pair is one case and run within one manifest; trials from different manifests are never paired, so run both sides of a pair in the same block.

## Every trial

Manifests are listed in the order given, each one a separate runner invocation; the Run column is the run within its manifest.

The 32 rows of runs 1 to 4 are as first printed. The 8 rows of run 5 were
printed by the same tool from its manifest and added below them.

| Manifest | Case | Run | Cell | Workflow | Hidden checks | Uncached input | Output | Cache read | Cache write | USD | Wall seconds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| run-1/harness-bench-2026-10-10T103634Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 58 | 147,699 | 4,184,944 | 176,649 | 0.5220 | 628.6 |
| run-1/harness-bench-2026-10-10T103634Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 30 | 120,791 | 1,717,485 | 147,400 | 0.3329 | 508.7 |
| run-1/harness-bench-2026-10-10T103634Z.json | gridcalc | 1 | native-luna-low | completed | 129/154 | 44,487 | 13,436 | 679,168 | 0 | 0.0180 | 115.4 |
| run-1/harness-bench-2026-10-10T103634Z.json | gridcalc | 1 | aether-single-luna-low | completed | 94/154 | 45,974 | 12,153 | 562,176 | 0 | 0.0163 | 90.6 |
| run-1/harness-bench-2026-10-10T103634Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 48 | 107,357 | 2,563,314 | 134,730 | 0.3700 | 459.2 |
| run-1/harness-bench-2026-10-10T103634Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 38 | 70,121 | 1,627,997 | 100,578 | 0.1418 | 307.5 |
| run-1/harness-bench-2026-10-10T103634Z.json | matchbook | 1 | native-luna-low | completed | 95/119 | 84,548 | 16,327 | 1,077,504 | 0 | 0.0274 | 170.6 |
| run-1/harness-bench-2026-10-10T103634Z.json | matchbook | 1 | aether-single-luna-low | completed | 111/119 | 26,119 | 6,380 | 253,184 | 0 | 0.0083 | 56.5 |
| run-2/harness-bench-2026-10-10T104714Z.json | gridcalc | 1 | native-haiku-low | completed | 153/154 | 54 | 158,627 | 3,534,956 | 187,575 | 0.5752 | 674.5 |
| run-2/harness-bench-2026-10-10T104714Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 38 | 126,267 | 2,411,496 | 155,228 | 0.3670 | 540.9 |
| run-2/harness-bench-2026-10-10T104714Z.json | gridcalc | 1 | native-luna-low | completed | 107/154 | 146,159 | 33,967 | 7,142,912 | 0 | 0.1030 | 386.5 |
| run-2/harness-bench-2026-10-10T104714Z.json | gridcalc | 1 | aether-single-luna-low | completed | 120/154 | 113,126 | 25,340 | 2,938,624 | 0 | 0.0534 | 279.0 |
| run-2/harness-bench-2026-10-10T104714Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 30 | 80,415 | 1,222,649 | 103,054 | 0.1453 | 331.3 |
| run-2/harness-bench-2026-10-10T104714Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 16 | 52,969 | 453,531 | 81,160 | 0.0473 | 217.0 |
| run-2/harness-bench-2026-10-10T104714Z.json | matchbook | 1 | native-luna-low | completed | 80/119 | 24,078 | 8,366 | 327,168 | 0 | 0.0099 | 79.8 |
| run-2/harness-bench-2026-10-10T104714Z.json | matchbook | 1 | aether-single-luna-low | completed | 99/119 | 32,958 | 8,792 | 370,944 | 0 | 0.0114 | 72.3 |
| run-3/harness-bench-2026-10-10T105837Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 32 | 155,300 | 1,953,746 | 179,622 | 0.4150 | 641.0 |
| run-3/harness-bench-2026-10-10T105837Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 32 | 132,596 | 2,032,135 | 168,427 | 0.3826 | 556.5 |
| run-3/harness-bench-2026-10-10T105837Z.json | gridcalc | 1 | native-luna-low | completed | 132/154 | 101,582 | 22,290 | 4,264,448 | 0 | 0.0639 | 316.6 |
| run-3/harness-bench-2026-10-10T105837Z.json | gridcalc | 1 | aether-single-luna-low | completed | 144/154 | 182,914 | 40,740 | 5,236,736 | 0 | 0.0910 | 425.5 |
| run-3/harness-bench-2026-10-10T105837Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 22 | 68,495 | 742,703 | 92,432 | 0.0838 | 283.6 |
| run-3/harness-bench-2026-10-10T105837Z.json | matchbook | 1 | aether-single-haiku-low | completed | 95/119 | 18 | 56,799 | 555,351 | 83,563 | 0.0507 | 235.1 |
| run-3/harness-bench-2026-10-10T105837Z.json | matchbook | 1 | native-luna-low | completed | 111/119 | 68,208 | 14,196 | 1,506,560 | 0 | 0.0290 | 331.1 |
| run-3/harness-bench-2026-10-10T105837Z.json | matchbook | 1 | aether-single-luna-low | completed | 117/119 | 40,761 | 7,594 | 433,920 | 0 | 0.0122 | 58.8 |
| run-4/harness-bench-2026-10-10T110927Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 36 | 149,316 | 2,275,982 | 176,490 | 0.4433 | 632.0 |
| run-4/harness-bench-2026-10-10T110927Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 62 | 138,285 | 4,194,504 | 170,905 | 0.5233 | 692.8 |
| run-4/harness-bench-2026-10-10T110927Z.json | gridcalc | 1 | native-luna-low | completed | 119/154 | 48,780 | 12,865 | 1,053,696 | 0 | 0.0218 | 120.2 |
| run-4/harness-bench-2026-10-10T110927Z.json | gridcalc | 1 | aether-single-luna-low | completed | 115/154 | 87,139 | 12,863 | 952,320 | 0 | 0.0247 | 276.1 |
| run-4/harness-bench-2026-10-10T110927Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 36 | 83,889 | 1,617,426 | 111,183 | 0.1799 | 355.0 |
| run-4/harness-bench-2026-10-10T110927Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 24 | 66,848 | 871,446 | 93,713 | 0.0965 | 274.6 |
| run-4/harness-bench-2026-10-10T110927Z.json | matchbook | 1 | native-luna-low | completed | 104/119 | 43,336 | 7,287 | 337,920 | 0 | 0.0114 | 62.4 |
| run-4/harness-bench-2026-10-10T110927Z.json | matchbook | 1 | aether-single-luna-low | completed | 96/119 | 279,290 | 33,614 | 9,635,584 | 0 | 0.1411 | 884.0 |
| run-5/harness-bench-2026-10-10T114701Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 7,666 | 191,429 | 3,704,297 | 233,423 | 0.5967 | 884.3 |
| run-5/harness-bench-2026-10-10T114701Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 153/154 | 36 | 116,428 | 2,011,955 | 144,891 | 0.3540 | 487.2 |
| run-5/harness-bench-2026-10-10T114701Z.json | gridcalc | 1 | native-luna-low | completed | 143/154 | 43,259 | 15,349 | 676,864 | 0 | 0.0188 | 141.8 |
| run-5/harness-bench-2026-10-10T114701Z.json | gridcalc | 1 | aether-single-luna-low | completed | 116/154 | 72,183 | 23,864 | 1,812,992 | 0 | 0.0373 | 230.7 |
| run-5/harness-bench-2026-10-10T114701Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 24 | 66,366 | 836,433 | 89,750 | 0.0651 | 273.9 |
| run-5/harness-bench-2026-10-10T114701Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 30 | 69,069 | 1,175,363 | 96,752 | 0.0991 | 285.7 |
| run-5/harness-bench-2026-10-10T114701Z.json | matchbook | 1 | native-luna-low | completed | 118/119 | 29,840 | 10,236 | 350,464 | 0 | 0.0116 | 93.1 |
| run-5/harness-bench-2026-10-10T114701Z.json | matchbook | 1 | aether-single-luna-low | completed | 100/119 | 44,612 | 10,432 | 473,600 | 0 | 0.0144 | 88.0 |

A figure reading "at least" comes from incomplete receipts and is a lower bound. The manifest column is the file and its directory, not a full path.

## Timeouts, failures and partial receipts

- **Timeouts:** none. The longest trial took 884.3 seconds of the 1200
  allowed (native Haiku on `gridcalc`, run 5); the next longest took 884.0
  (Aether Luna on `matchbook`, run 4).
- **Workflows not finished:** none of 40.
- **Environment faults** (rate or usage limit, out of memory, daemon start):
  none seen. No trial was rerun and no run was left out.
- **Partial receipts, unknown or unpriced figures:** none. Every five-run
  mean is over 10 of 10 trials, every four-run mean over 8 of 8.
- **Blocked trials:** none.
- **Integrity:** no trial reports a changed benchmark input.
- **One contract flag.** In run 2 the Aether Luna trial on `gridcalc` changed
  the public test file `tests/test_basic.py`, which the task tells the solver
  to leave alone. The runner records this as a contract violation. The trial
  is counted as it stands: its hidden checks (120/154) are graded from files
  the solver cannot touch. No other trial has a contract or protocol flag.
- **Run 5 came later.** A fifth run was not started with the first four: the
  time set aside for counted runs was nearly used when run 4 ended, and that
  decision was made on the clock, before run 4 was tabulated. Run 5 was made
  the same day after the four-run tables had been written and read, because
  a reader asked for one more run. It had no smoke, no pilot and no rerun,
  and it is the only run added.

The runner's own `status` field marks a trial `failed` unless it passes every
hidden check, so most Luna rows read `failed` in the manifests although their
workflows finished. The tables above use the workflow and the check counts.

## Counting rule

Written down before the first counted trial started, and not changed since:

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

Added before run 5 started (committed at 11:43:42 UTC; run 5 started at
11:47:00), with the six rules above applied to it unchanged:

7. Run 5 was added after runs 1 to 4 had been seen, at a reader's request. It
   is counted whole, whatever it shows. No further run is added on the
   strength of its result.

Separately, the report tool keeps its own frozen rule: a trial the runner
classifies as `blocked` (the host refused a tool call and the agent idled) is
left out of the means and counted in its own row.

## Not counted: the calibration pilot and the smoke

- **Calibration pilot (part of freezing the cases).** One native low-effort
  trial per case and model, written with `--pilot`, described in the
  [protocol](../evals/harness_bench_protocol.md). It ran before the freeze and
  is in no table here.
- **Smoke.** Before the first counted trial, the four cells were run once end
  to end on the older `ledger` case (22 hidden checks), four at once, with
  `--pilot --timeout 600`. It was the first end-to-end run of the Aether Haiku
  cell. All four trials finished and passed 22/22 in 31 to 103 seconds; each
  resolved the model that was asked for (`claude-haiku-5-5`, `gpt-6-luna`),
  every token receipt was complete and every trial was priced, so nothing in
  the runner's capture had to be changed. Its manifest carries `pilot: true`,
  which the report tool refuses, and `ledger` is not one of the counted cases.
  The report tool's per-trial table was edited while the smoke was running,
  so the smoke's rows carry an integrity flag for the tool and its test; the edit
  was committed before the first counted run, and no counted trial has one.

## Limits

- **Few runs.** 5 runs per cell and case give 10 pairs per model. Every
  five-run mean is a mean of that many trials, and one trial moves it
  visibly. No significance is claimed, and none should be read into a
  difference.
- **Run 5 was not planned.** It was added after runs 1 to 4 had been seen.
  A run added after looking is weaker evidence than one fixed in advance,
  even when it is counted whole on a rule written before it started.
- **Five-run means come from the per-trial table.** The manifests of runs 1
  to 4 were not kept, so those means are computed from figures already
  rounded for print: USD to four decimals, wall seconds to one.
- **One machine, 90 minutes.** Provider load, cache warmth and the other
  work on this host are not controlled. Other agents used the same machine
  and the same subscriptions while the trials ran.
- **Wall time is a paired figure.** All 8 trials of a run ran at once
  (`--jobs 8`), so each trial's wall time is time under shared load. It is
  comparable within a pair and within these runs. It is not an absolute
  latency and must not be compared with sequential results.
- **Effort is requested, not always confirmed.** Codex records the effective
  effort in its session file, and it was `low` for every Luna trial on both
  sides. Claude Code receipts and Aether's `engine.session` events do not
  report effort, so for both Haiku cells "low" is what was asked for.
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
  starts the same CLI with its own settings, its coordination tools over MCP
  and the CLI's own delegation tools switched off. A difference in tokens or
  time can come from any of that, or from chance. These runs do not say which.
  The [overhead diagnosis](harness-overhead-diagnosis-2026-10-10.md) looks
  for it with diagnostic arms, none of which is counted here.
- **Two cases, one kind of task.** Both are Python repair tasks with a public
  contract. Nothing here speaks to delegation, long sessions, compaction or
  other languages.
- **Ceilings.** Over runs 1 to 4, and over all five, no pair passed every
  hidden check in every run of a case, so the tool printed no ceiling note
  for runs 1 to 4. Its run 5 table prints one for Haiku on `matchbook`,
  where both sides passed every check in that run. The Haiku pair is close
  to the ceiling on both cases; the summary says what that means for its
  accuracy figures.

## Reproduce

Live runs spend model tokens on the logged-in Claude and Codex accounts.

```sh
cargo build --release
python3 -m unittest discover -s evals -p 'test_harness_bench*.py'   # no model calls

# One counted run: the matched set of 8 trials at once. Repeat with seeds 1 to 5 and its own run-<n> directory.
python3 evals/harness_bench.py --cases matchbook,gridcalc --runs 1 --seed 1 --jobs 8 --timeout 1200 \
  --conditions native-haiku-low,aether-single-haiku-low,native-luna-low,aether-single-luna-low \
  --binary target/release/aether --output-dir target/harness-393/run-1

# The tables and the per-trial list in this report.
python3 evals/harness_bench_compare.py target/harness-393/run-*/harness-bench-*.json --trials
```

The manifests and per-trial artifacts are ignored local files under
`target/harness-393/`, as for the earlier pilots; this report carries the
per-trial table instead. A new run gives new numbers: the models are not
deterministic, and provider load differs.
