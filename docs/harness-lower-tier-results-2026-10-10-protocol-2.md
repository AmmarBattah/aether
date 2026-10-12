# Lower-tier harness comparison under protocol version 2: Claude Haiku 5.5 and GPT-6 Luna, 2026-10-10

> **Note, 2026-10-11.** Five more runs of this comparison, seeds 6 to 10, were
> made the next day on the same binary with the same settings. They, and the
> figures over all ten runs, are in the [report of
> 2026-10-11](harness-lower-tier-results-2026-10-11.md). Nothing below this note
> was changed.

Does running a cheap model through Aether cost accuracy, tokens, money or time
against running it in its native harness? This report gives five counted runs
of a matched comparison on two frozen cases, 40 trials in all, measured under
protocol version 2 on the Aether binary that no longer repeats the effort in
an engine's status line. It is the second measurement. The
[first](harness-lower-tier-results-2026-10-10.md) was made under protocol
version 1 earlier on the same UTC day and stands as it was written. The number
of runs and the rule for counting them were committed before the first run
started; no run was added, dropped or repeated. It is a small sample from one
machine and makes no significance claim.

## Summary

Means per trial. Each row is 10 trials: 5 runs of 2 cases.

| Model, requested effort | Harness | Workflows finished | Hidden checks passed | Uncached input tokens | Output tokens | Cache read tokens | API-equivalent USD | Wall seconds |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5, low | Claude Code | 10/10 | 100.0 % | 254 | 120,216 | 2,637,077 | 0.3511 | 536.1 |
| Claude Haiku 5.5, low | Aether | 10/10 | 99.7 % | 30 | 112,406 | 1,660,749 | 0.2810 | 470.9 |
| | Aether minus native, 10 pairs | | -0.3 pp | -224 | -7,810 | -976,328 | -0.0701 | -65.2 |
| GPT-6 Luna, low | Codex | 10/10 | 76.7 % | 50,968 | 14,221 | 1,077,325 | 0.0230 | 237.2 |
| GPT-6 Luna, low | Aether | 10/10 | 75.9 % | 73,320 | 18,810 | 1,817,984 | 0.0349 | 227.5 |
| | Aether minus native, 10 pairs | | -0.8 pp | +22,352 | +4,590 | +740,659 | +0.0119 | -9.8 |

How the 10 pairs of each model fall. A pair is one case in one run, both sides
started in the same block. "Lower" means the Aether trial's figure is below
its native partner's. The last column is the same difference in the first
measurement, for reference only: see [Against the first
measurement](#against-the-first-measurement).

| Model | Measure | Native mean | Aether mean | Aether minus native | Relative to native | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | Version 1: difference (lower / higher / equal) |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | Hidden checks passed (percent) | 100.0 % | 99.7 % | -0.3 pp |  | 2 / 0 / 8 | -1.7 pp | +0.0 pp | -2.0 pp (2 / 1 / 7) |
| Claude Haiku 5.5 | Uncached input tokens | 254 | 30 | -224 | -88.2 % | 8 / 2 / 0 | -2,158 | +6 | -768 (7 / 2 / 1) |
| Claude Haiku 5.5 | Output tokens | 120,216 | 112,406 | -7,810 | -6.5 % | 5 / 5 / 0 | -55,954 | +16,247 | -25,872 (9 / 1 / 0) |
| Claude Haiku 5.5 | Cache read tokens | 2,637,077 | 1,660,749 | -976,328 | -37.0 % | 8 / 2 / 0 | -3,726,708 | +614,400 | -558,519 (7 / 3 / 0) |
| Claude Haiku 5.5 | API-equivalent USD | 0.3511 | 0.2810 | -0.0701 | -20.0 % | 6 / 4 / 0 | -0.3382 | +0.1259 | -0.1001 (8 / 2 / 0) |
| Claude Haiku 5.5 | Wall seconds | 536.1 | 470.9 | -65.2 | -12.2 % | 6 / 4 / 0 | -401.3 | +64.7 | -105.7 (8 / 2 / 0) |
| GPT-6 Luna | Hidden checks passed (percent) | 76.7 % | 75.9 % | -0.8 pp |  | 6 / 4 / 0 | -89.1 pp | +87.4 pp | -1.4 pp (5 / 5 / 0) |
| GPT-6 Luna | Uncached input tokens | 50,968 | 73,320 | +22,352 | +43.9 % | 2 / 8 / 0 | -41,568 | +104,202 | +29,080 (3 / 7 / 0) |
| GPT-6 Luna | Output tokens | 14,221 | 18,810 | +4,590 | +32.3 % | 3 / 7 / 0 | -9,130 | +14,651 | +2,745 (5 / 5 / 0) |
| GPT-6 Luna | Cache read tokens | 1,077,325 | 1,817,984 | +740,659 | +68.7 % | 4 / 6 / 0 | -2,010,368 | +4,427,776 | +525,338 (5 / 5 / 0) |
| GPT-6 Luna | API-equivalent USD | 0.0230 | 0.0349 | +0.0119 | +51.9 % | 3 / 7 / 0 | -0.0288 | +0.0620 | +0.0095 (4 / 6 / 0) |
| GPT-6 Luna | Wall seconds | 237.2 | 227.5 | -9.8 | -4.1 % | 6 / 4 / 0 | -185.4 | +271.1 | +64.4 (6 / 4 / 0) |

Hidden checks passed per case, mean of 5 runs with every run's count:

| Model | Harness | `matchbook` (119 checks) | `gridcalc` (154 checks) |
| --- | --- | --- | --- |
| Claude Haiku 5.5 | Claude Code | 100.0 %: 119, 119, 119, 119, 119 | 100.0 %: 154, 154, 154, 154, 154 |
| Claude Haiku 5.5 | Aether | 99.3 %: 119, 117, 119, 119, 117 | 100.0 %: 154, 154, 154, 154, 154 |
| GPT-6 Luna | Codex | 73.3 %: 94, 118, 118, 106, 0 | 80.1 %: 132, 121, 93, 140, 131 |
| GPT-6 Luna | Aether | 75.5 %: 110, 116, 119, 0, 104 | 76.4 %: 119, 108, 116, 128, 117 |

Counts are in run order, 1 to 5. The same per case, as pairs:

| Model | Case | Native mean (checks) | Aether mean (checks) | Aether minus native (checks) | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | `matchbook` | 119.0 | 118.2 | -0.8 | 2 / 0 / 3 | -2 | +0 |
| Claude Haiku 5.5 | `gridcalc` | 154.0 | 154.0 | +0.0 | 0 / 0 / 5 | +0 | +0 |
| GPT-6 Luna | `matchbook` | 87.2 | 89.8 | +2.6 | 2 / 3 / 0 | -106 | +104 |
| GPT-6 Luna | `gridcalc` | 123.4 | 117.6 | -5.8 | 4 / 1 / 0 | -14 | +23 |

What these numbers show, and what they do not:

- **Where Aether is behind with Haiku, in the mean:** hidden checks passed
  (percent) 99.7 % against 100.0 % (-0.3 pp; lower in 2 of 10 pairs, higher in
  0, equal in 8).
- **Where Aether is ahead with Haiku, in the mean:** uncached input tokens 30
  against 254 (-224; lower in 8 of 10 pairs, higher in 2, equal in 0); output
  tokens 112,406 against 120,216 (-7,810; lower in 5 of 10 pairs, higher in 5,
  equal in 0); cache read tokens 1,660,749 against 2,637,077 (-976,328; lower
  in 8 of 10 pairs, higher in 2, equal in 0); API-equivalent USD 0.2810
  against 0.3511 (-0.0701; lower in 6 of 10 pairs, higher in 4, equal in 0);
  wall seconds 470.9 against 536.1 (-65.2; lower in 6 of 10 pairs, higher in
  4, equal in 0).
- **Where Aether is behind with Luna, in the mean:** hidden checks passed
  (percent) 75.9 % against 76.7 % (-0.8 pp; lower in 6 of 10 pairs, higher in
  4, equal in 0); uncached input tokens 73,320 against 50,968 (+22,352; lower
  in 2 of 10 pairs, higher in 8, equal in 0); output tokens 18,810 against
  14,221 (+4,590; lower in 3 of 10 pairs, higher in 7, equal in 0); cache read
  tokens 1,817,984 against 1,077,325 (+740,659; lower in 4 of 10 pairs, higher
  in 6, equal in 0); API-equivalent USD 0.0349 against 0.0230 (+0.0119; lower
  in 3 of 10 pairs, higher in 7, equal in 0).
- **Where Aether is ahead with Luna, in the mean:** wall seconds 227.5 against
  237.2 (-9.8; lower in 6 of 10 pairs, higher in 4, equal in 0).
- **Ceilings.** Trials that passed every hidden check: Claude Code with Haiku
  5 of 5 on `matchbook` and 5 of 5 on `gridcalc`; Aether with Haiku 3 of 5 on
  `matchbook` and 5 of 5 on `gridcalc`; Codex with Luna 0 of 5 on `matchbook`
  and 0 of 5 on `gridcalc`; Aether with Luna 1 of 5 on `matchbook` and 0 of 5
  on `gridcalc`. Where both sides of a pair pass everything, the pair can show
  a loss and cannot show a gain, so equal scores there are not evidence that
  nothing is lost.
- **Hung grades.** Two Luna trials on `matchbook`, one on each side, were
  graded 0 because the solution hung in the grader's one process: the Aether
  trial of run 4 and the Codex trial of run 5. Both stay at 0 in every figure
  of this report. Run one test at a time, 107 and 110 of their 119 tests pass
  ([The second look at hung grades](#the-second-look-at-hung-grades)); those
  numbers are in no mean. Each 0 puts its side's Luna accuracy mean about 9
  points below what a grade per test would give, so both Luna accuracy means
  here (76.7 % and 75.9 %) carry one such trial.
- **Haiku, thinking before the first edit** (read from the session
  transcripts): 80,480 tokens through Aether against 81,369 natively (-889,
  -1.1 %), lower in 5 of 10 pairs and higher in 5, single pairs from -20,014
  to +21,856. Thinking in total: 86,244 against 91,023 (-4,778, -5.2 %), lower
  in 6 pairs and higher in 4. In runs 1 to 4 of the first measurement the
  diagnosis read total thinking as lower through Aether in 8 of 8 pairs.
- **One long native Haiku trial.** `native-haiku-low` on `gridcalc` in run 5
  took 951.6 of its 1200 seconds, 190,517 output tokens, 2,196 uncached input
  tokens and USD 0.7315, the largest figure of each kind among the 20 Haiku
  trials. It is counted as it stands. It is the pair at the low end of every
  Haiku token and cost range: with that one pair left out, Haiku's mean
  difference would be -2.2 % in output tokens, -24.7 % in uncached input
  tokens and -13.1 % in USD.
- **Memory.** The memory guard ended no process. Twice, for under a minute,
  available memory on the host fell to about 1.1 GB (1,079 MB in run 4, 1,215
  MB in run 5), each time while the grader ran the hidden tests of one of the
  two solutions that hung. The guard leaves the grader alone. See [Timeouts,
  failures, guard firings and unknown
  figures](#timeouts-failures-guard-firings-and-unknown-figures).
- **The stop rule of the overhead loop** is **not met** for Claude Haiku 5.5
  and **not met** for GPT-6 Luna. See [The stop rule](#the-stop-rule).

## What changed since the first measurement

Two things, both located by the
[overhead diagnosis](harness-overhead-diagnosis-2026-10-10.md):

1. **Aether no longer repeats the effort in an engine's status line**
   (commit `f346e09b`, `crates/aether-context/src/render.rs`). The first
   message of an agent on a CLI engine ended "Turn 1. Effort low."; it now
   ends "Turn 1.". Every engine already hands the effort to its CLI as a
   setting. Why: the diagnosis measured, natively, that the two words lower
   Haiku's thinking below what the setting alone asks for, so the Aether side
   of the Haiku pair was not working at the effort both sides were asked for.
   The change reaches the Luna cell too: its first message is 12 characters
   shorter.
2. **Protocol version 2: both sides of a Claude pair start with the same MCP
   set** (commit `ad3d9b5a`, `evals/harness_bench_receipts.py`). Under
   version 1 a native Claude trial was started with `--strict-mcp-config` and
   an Aether one was not, so only the Aether solver was shown the machine
   user's own MCP connectors. Under version 2 the benchmark adds the flag on
   the Aether side as well, and the session loads Aether's own server and
   nothing else. Why: the two sides did not start equal, and the diagnosis
   measured that loading those connectors lowers Haiku's output, through
   Aether and natively. Aether itself is not changed by this, and nothing
   changes for Codex.

Kept as they were: the two frozen cases and their digests, the four cells,
seeds 1 to 5, `--jobs 8`, `--timeout 1200`, one runner invocation per run,
the task text, the graders, the CLI versions and the machine.

What else differs, and is not a setting of the comparison:

- **The runs were planned.** Five runs were fixed before the first. The
  first measurement made four and added a fifth after looking.
- **A memory guard ran beside every run.** It ends a process started inside
  a trial workspace once that process holds more than 4 GB. Whether it fired
  is under
  [Timeouts, failures, guard firings and unknown figures](#timeouts-failures-guard-firings-and-unknown-figures).
- **The benchmark code is later.** Between the two measurements the runner
  gained the diagnostic arms, a transcript reader and the version 2 flag. No
  counted cell uses a diagnostic arm. The cases and graders
  (`evals/harness_bench_cases.py`, the fixtures) and the task text are byte
  for byte what they were, and the two native cells start as they did.
- **The sitting.** These runs started about eleven hours after the first
  measurement's, back to back; the first measurement's fifth run followed
  its fourth by 23 minutes.

## Against the first measurement

Means per trial and the paired difference, Aether minus native, in the first
measurement (protocol version 1, 10 pairs per model) beside this one. The four
headline measures are in bold. "Lower / higher / equal" counts the pairs in
which the Aether trial's figure is below, above or equal to its native
partner's.

| Model | Measure | Version 1: native | Aether | Aether minus native | Lower / higher / equal | Version 2: native | Aether | Aether minus native | Lower / higher / equal |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | **Hidden checks passed (percent)** | 99.9 % | 97.9 % | -2.0 pp | 2 / 1 / 7 | 100.0 % | 99.7 % | -0.3 pp | 2 / 0 / 8 |
| Claude Haiku 5.5 | **Uncached input tokens** | 801 | 32 | -768 | 7 / 2 / 1 | 254 | 30 | -224 | 8 / 2 / 0 |
| Claude Haiku 5.5 | **Output tokens** | 120,889 | 95,017 | -25,872 | 9 / 1 / 0 | 120,216 | 112,406 | -7,810 | 5 / 5 / 0 |
| Claude Haiku 5.5 | Cache read tokens | 2,263,645 | 1,705,126 | -558,519 | 7 / 3 / 0 | 2,637,077 | 1,660,749 | -976,328 | 8 / 2 / 0 |
| Claude Haiku 5.5 | **API-equivalent USD** | 0.3396 | 0.2395 | -0.1001 | 8 / 2 / 0 | 0.3511 | 0.2810 | -0.0701 | 6 / 4 / 0 |
| Claude Haiku 5.5 | Wall seconds | 516.3 | 410.6 | -105.7 | 8 / 2 / 0 | 536.1 | 470.9 | -65.2 | 6 / 4 / 0 |
| GPT-6 Luna | **Hidden checks passed (percent)** | 83.6 % | 82.2 % | -1.4 pp | 5 / 5 / 0 | 76.7 % | 75.9 % | -0.8 pp | 6 / 4 / 0 |
| GPT-6 Luna | **Uncached input tokens** | 63,428 | 92,508 | +29,080 | 3 / 7 / 0 | 50,968 | 73,320 | +22,352 | 2 / 8 / 0 |
| GPT-6 Luna | **Output tokens** | 15,432 | 18,177 | +2,745 | 5 / 5 / 0 | 14,221 | 18,810 | +4,590 | 3 / 7 / 0 |
| GPT-6 Luna | Cache read tokens | 1,741,670 | 2,267,008 | +525,338 | 5 / 5 / 0 | 1,077,325 | 1,817,984 | +740,659 | 4 / 6 / 0 |
| GPT-6 Luna | **API-equivalent USD** | 0.0315 | 0.0410 | +0.0095 | 4 / 6 / 0 | 0.0230 | 0.0349 | +0.0119 | 3 / 7 / 0 |
| GPT-6 Luna | Wall seconds | 181.8 | 246.2 | +64.4 | 6 / 4 / 0 | 237.2 | 227.5 | -9.8 | 6 / 4 / 0 |

**These are two sittings, about eleven hours apart on the same UTC day, with a
different Aether binary and a different MCP set on the Aether side of the
Haiku pair.** Provider load, cache state and the other work on the machine
were not held equal between them. Only the pairs inside one run are a
controlled comparison. The step from the version 1 column to the version 2
column is not one: it holds the two changes and everything else that differed
between the sittings.

How much "everything else" can be is visible in the native cells, which
neither change touches: Claude Code with Haiku went from 120,889 to 120,216
output tokens per trial (-0.6 %); Claude Code with Haiku went from 0.3396 to
0.3511 USD per trial (+3.4 %); Codex with Luna went from 15,432 to 14,221
output tokens per trial (-7.8 %); Codex with Luna went from 0.0315 to 0.0230
USD per trial (-27.0 %).

- **Haiku.** The output difference was -25,872 tokens with Aether lower in 9
  of 10 pairs; it is -7,810 with Aether lower in 5 and higher in 5. Uncached
  input, cache read, cost and wall time are lower through Aether in the mean
  in both measurements; the cost difference went from -0.1001 (8 lower, 2
  higher) to -0.0701 (6 lower, 4 higher). Hidden checks: -2.0 points then (2
  lower, 1 higher, 7 equal), -0.3 points now (2 lower, 0 higher, 8 equal).
- **Luna.** Aether is above Codex in tokens and cost in the mean in both
  measurements: uncached input +29,080 then and +22,352 now, output +2,745
  then and +4,590 now, USD +0.0095 then and +0.0119 now. Wall time was +64.4
  seconds then and is -9.8 now, with Aether lower in 6 of 10 pairs both times;
  the first measurement held one Aether trial of 884 seconds. Hidden checks:
  -1.4 points then, -0.8 now.
- **What may be said of the step.** Nothing about Luna was changed between the
  two measurements except 12 characters of its first message, and its
  differences still moved by this much; that is what two sittings do by
  themselves. The smaller and no longer one-sided Haiku output difference is
  what the diagnosis's two located causes would produce. These runs do not
  show that the two changes produced it.

## The stop rule

The overhead loop on the pull request fixed its stop rule before its first
diagnostic trial, in these words:

> On the matched test after a fix, for each model:
> - output tokens, cost and held-out checks are no longer one-sided across
>   the pairs [...], and
> - the mean difference in tokens and cost is within 5 % of native.
>
> Accuracy alone cannot meet or fail the rule on a handful of runs, because
> run-to-run spread is larger than the gaps seen so far; I'll show it beside
> the token measures each time.

How it is applied here. This was written while run 1 was in flight, before
any result of this measurement had been read:

- **Part 1 is read strictly.** A measure is one-sided when every pair that
  differs falls the same way, however many pairs are equal. The diagnosis
  read 0 lower, 2 higher and 4 equal as not one-sided; the strict reading
  calls that one-sided.
- **Part 2** is held on output tokens, uncached input tokens and
  API-equivalent USD, as in the diagnosis: the mean paired difference
  divided by the native mean, within 5 % in either direction. A difference
  in Aether's favour beyond 5 % fails it as one against Aether does.
- The rule is met for a model only when every part is met on every measure
  it names.

**Claude Haiku 5.5, 10 pairs**

| Measure | Native mean | Aether mean | Mean difference | Relative to native | Aether lower / higher / equal | Smallest pair difference | Largest pair difference | Part 1: not one-sided | Part 2: within 5 % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| Output tokens | 120,216 | 112,406 | -7,810 | -6.5 % | 5 / 5 / 0 | -55,954 | +16,247 | met | not met |
| Uncached input tokens | 254 | 30 | -224 | -88.2 % | 8 / 2 / 0 | -2,158 | +6 | not part of it | not met |
| API-equivalent USD | 0.3511 | 0.2810 | -0.0701 | -20.0 % | 6 / 4 / 0 | -0.3382 | +0.1259 | met | not met |
| Hidden checks passed (percent) | 100.0 % | 99.7 % | -0.3 pp | -0.3 % | 2 / 0 / 8 | -1.7 pp | +0.0 pp | not met | not part of it |

- **Part 1, not one-sided** (Aether lower / higher / equal over 10 pairs). Met
  on: output tokens (5 / 5 / 0); API-equivalent USD (6 / 4 / 0). Not met on:
  hidden checks passed (percent) (2 / 0 / 8).
- **Part 2, mean difference within 5 % of the native mean.** Met on: no
  measure. Not met on: output tokens (5 / 5 / 0; -6.5 %); uncached input
  tokens (8 / 2 / 0; -88.2 %); API-equivalent USD (6 / 4 / 0; -20.0 %).
- **The stop rule is not met for Claude Haiku 5.5.**

**GPT-6 Luna, 10 pairs**

| Measure | Native mean | Aether mean | Mean difference | Relative to native | Aether lower / higher / equal | Smallest pair difference | Largest pair difference | Part 1: not one-sided | Part 2: within 5 % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| Output tokens | 14,221 | 18,810 | +4,590 | +32.3 % | 3 / 7 / 0 | -9,130 | +14,651 | met | not met |
| Uncached input tokens | 50,968 | 73,320 | +22,352 | +43.9 % | 2 / 8 / 0 | -41,568 | +104,202 | not part of it | not met |
| API-equivalent USD | 0.0230 | 0.0349 | +0.0119 | +51.9 % | 3 / 7 / 0 | -0.0288 | +0.0620 | met | not met |
| Hidden checks passed (percent) | 76.7 % | 75.9 % | -0.8 pp | -1.0 % | 6 / 4 / 0 | -89.1 pp | +87.4 pp | met | not part of it |

- **Part 1, not one-sided** (Aether lower / higher / equal over 10 pairs). Met
  on: output tokens (3 / 7 / 0); API-equivalent USD (3 / 7 / 0); hidden checks
  passed (percent) (6 / 4 / 0). Not met on: no measure.
- **Part 2, mean difference within 5 % of the native mean.** Met on: no
  measure. Not met on: output tokens (3 / 7 / 0; +32.3 %); uncached input
  tokens (2 / 8 / 0; +43.9 %); API-equivalent USD (3 / 7 / 0; +51.9 %).
- **The stop rule is not met for GPT-6 Luna.**

**What 10 pairs can and cannot decide about a 5 % mean.**

- Claude Haiku 5.5, output tokens: 5 % of the native mean is 6,011. The 10
  pair differences run from -55,954 (`gridcalc`, run 5) to +16,247
  (`matchbook`, run 5), a range 12 times the bar. With any one pair left out,
  the mean difference would lie between -8.4 % and -2.2 % of the native mean;
  one pair decides on which side of the bar it falls.
- Claude Haiku 5.5, uncached input tokens: 5 % of the native mean is 13. The
  10 pair differences run from -2,158 (`gridcalc`, run 5) to +6 (`gridcalc`,
  run 4), a range 170 times the bar. With any one pair left out, the mean
  difference would lie between -90.1 % and -24.7 % of the native mean; no
  single pair moves it across the bar.
- Claude Haiku 5.5, API-equivalent USD: 5 % of the native mean is 0.0176. The
  10 pair differences run from -0.3382 (`gridcalc`, run 5) to +0.1259
  (`matchbook`, run 5), a range 26 times the bar. With any one pair left out,
  the mean difference would lie between -24.6 % and -13.1 % of the native
  mean; no single pair moves it across the bar.
- GPT-6 Luna, output tokens: 5 % of the native mean is 711. The 10 pair
  differences run from -9,130 (`gridcalc`, run 4) to +14,651 (`gridcalc`, run
  5), a range 33 times the bar. With any one pair left out, the mean
  difference would lie between +24.1 % and +48.1 % of the native mean; no
  single pair moves it across the bar.
- GPT-6 Luna, uncached input tokens: 5 % of the native mean is 2,548. The 10
  pair differences run from -41,568 (`gridcalc`, run 4) to +104,202
  (`gridcalc`, run 5), a range 57 times the bar. With any one pair left out,
  the mean difference would lie between +25.2 % and +64.0 % of the native
  mean; no single pair moves it across the bar.
- GPT-6 Luna, API-equivalent USD: 5 % of the native mean is 0.0011. The 10
  pair differences run from -0.0288 (`gridcalc`, run 4) to +0.0620
  (`gridcalc`, run 5), a range 79 times the bar. With any one pair left out,
  the mean difference would lie between +27.0 % and +84.2 % of the native
  mean; no single pair moves it across the bar.

10 pairs can show that a difference does not fall the same way in every
pair: that is part 1, and it is a count. They cannot place a mean within 5 %
when single pairs differ by many times the bar. "Not met" on part 2 says that
the measured mean lies outside the bar. It does not show that Aether costs or
saves that much. A "met" on part 2 says that the measured mean fell inside
the bar in these 10 pairs. It does not show parity, and nothing in this
report is called parity on a mean.

## The Luna accuracy question

The diagnosis left one question open: on `matchbook`, full-tool Aether passed
fewer hidden checks than Codex in 12 of 16 diagnostic blocks. Those trials
are diagnostic and uncounted. The counted pairs of this measurement are set
beside them here. The two are never added into one figure.

Every counted Luna pair, in run order:

| Run | Case | Codex | Aether | Aether minus Codex | Hung grade | Second look (passed alone, of total) |
| ---: | --- | ---: | ---: | ---: | --- | --- |
| 1 | `gridcalc` | 132 | 119 | -13 | none |  |
| 1 | `matchbook` | 94 | 110 | +16 | none |  |
| 2 | `gridcalc` | 121 | 108 | -13 | none |  |
| 2 | `matchbook` | 118 | 116 | -2 | none |  |
| 3 | `gridcalc` | 93 | 116 | +23 | none |  |
| 3 | `matchbook` | 118 | 119 | +1 | none |  |
| 4 | `gridcalc` | 140 | 128 | -12 | none |  |
| 4 | `matchbook` | 106 | 0 | -106 | aether-single-luna-low `f2f5c8702a` | `f2f5c8702a`: 107 of 119 |
| 5 | `gridcalc` | 131 | 117 | -14 | none |  |
| 5 | `matchbook` | 0 | 104 | +104 | native-luna-low `338b338dab` | `338b338dab`: 110 of 119 |

| Case | Pairs | As counted: Aether below / above / equal | Without pairs that hold a hung grade | With the second look in place of a hung grade (not a counted figure) |
| --- | ---: | ---: | ---: | ---: |
| `matchbook` | 5 | 2 / 3 / 0 | 1 / 2 / 0 (of 3) | 2 / 3 / 0 |
| `gridcalc` | 5 | 4 / 1 / 0 | 4 / 1 / 0 (of 5) | 4 / 1 / 0 |
| both | 10 | 6 / 4 / 0 | 5 / 3 / 0 (of 8) | 6 / 4 / 0 |

In the 10 counted Luna pairs, Aether passed fewer hidden checks than Codex in
6, more in 4 and the same number in 0. Per case: `matchbook` fewer in 2 of 5,
more in 3, equal in 0 (means 89.8 against 87.2 checks); `gridcalc` fewer in 4
of 5, more in 1, equal in 0 (means 117.6 against 123.4 checks). 2 of the pairs
hold a grade of 0 from a grader that ran out of time (`aether-single-luna-low`
on `matchbook`, run 4, `native-luna-low` on `matchbook`, run 5). Without those
pairs Aether is below Codex in 5 of 8.

With the second look in place of the two hung grades, which is not a counted
figure, the `matchbook` count is again 2 below and 3 above, on other pairs:
run 4 turns from Aether below (0 against 106) to above (107 against 106), and
run 5 from Aether above (104 against 0) to below (104 against 110).

**What this adds to the diagnosis's 12 of 16.** Set side by side and never
added. The diagnostic blocks ran on `matchbook` only, on other binaries and
with other arms beside them:

| Source | Kind | Case | Aether below Codex |
| --- | --- | --- | ---: |
| Diagnosis, iterations 1 and 2, full-tool Aether against Codex | diagnostic, never counted | `matchbook` | 12 of 16 blocks |
| First measurement, protocol version 1 | counted | `matchbook` | 2 of 5 pairs |
| This measurement, protocol version 2 | counted | `matchbook` | 2 of 5 pairs (1 of 3 without the pairs that hold a hung grade) |
| First measurement, protocol version 1 | counted | `gridcalc` | 3 of 5 pairs |
| This measurement, protocol version 2 | counted | `gridcalc` | 4 of 5 pairs |

- **On `matchbook`, the counted pairs do not repeat the 12 of 16.** Aether is
  below Codex in 2 of 5 here, as in the first measurement, and one of the two
  is the Aether trial at 0 from the grader's limit. One trial at 0 on each
  side moves each side's `matchbook` mean by more than 20 checks, so the
  means there (89.8 against 87.2) say little.
- **On `gridcalc`, which no diagnostic block ran, Aether is below Codex in 4
  of 5 pairs**, by 12 to 14 checks each (runs 1, 2, 4 and 5), and above by 23
  in run 3; the mean is 117.6 against 123.4 checks. In the first measurement
  it was below in 3 of 5. This is against Aether and is said as such. It was
  not a question fixed before these runs, and Codex alone ranged from 93 to
  140 checks on this case between runs, so 4 of 5 is an observation to test
  and not a finding.
- **What stays unknown.** Whether Aether costs Luna hidden checks against
  Codex. 10 counted pairs, 6 below and 4 above, with a mean difference of
  -0.8 points and single pairs from -89.1 to +87.4 points, do not show that it
  does and do not show that it does not.

A later diagnostic sitting took the question up with 24 matched Luna pairs,
12 on each case, and the same Codex command run twice in every block as a
yardstick; none of its trials is counted. Under a rule written before its
first trial it claims no Aether effect on hidden checks, tokens, cost or
time, and it reports the `gridcalc` lean as still there and not claimed.
The trials, the intervals and what they still allow are under
[Iteration 5](harness-overhead-diagnosis-2026-10-10.md#iteration-5) of the
diagnosis.

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

The two cases are the frozen cases of the first measurement (see the
[protocol](../evals/harness_bench_protocol.md)):

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
| Aether binary | `aether 0.1.1`, `cargo build --release`, SHA-256 `251c0b08f359c8a679a76831dea1d892b0c2a0a2c34e51427323c8b25d4d4145`, the same file in all 5 runs, not installed. Every Aether trial ran on its own isolated daemon |
| Built from | commit `08af6d32d3da41c498e0b1b96f12f35c52c75c17`, which contains the status line change `f346e09b`. Its SHA-256 equals that of the binary the diagnosis's second iteration built at `f346e09b`; no Rust source differs between the two commits. The first measurement's binary was `d687547b7c7b55a770927af54ef434a1fe25e004e488a31c3dd762b771608512` |
| Benchmark code | every manifest records commit `2a7c3023faffafee7a163e34dc217118f6b37529` with a clean tree and `protocol_version` 2. That commit adds this report's counting rule and one diagnostic arm to `08af6d32`; no counted cell, case, grader or prompt differs from `ad3d9b5a`, where version 2 was introduced |
| Native CLIs | `claude` 2.1.294 (Claude Code), `codex-cli` 0.162.1: the versions of the first measurement |
| Cells | `native-haiku-low`, `aether-single-haiku-low`, `native-luna-low`, `aether-single-luna-low` |
| Cases | `matchbook`, `gridcalc` |
| Runs | 5, one runner invocation each, `--runs 1`, seeds 1 to 5, launched one after another by one script. Each is the matched set of 8 trials. Five were planned before the first, on the rule under "Counting rule" |
| Trials at once | `--jobs 8`: all 8 trials of a run in one block, so both sides of every pair ran under the same load |
| Timeout | `--timeout 1200` seconds for every trial on both sides |
| Memory guard | the script of the diagnosis's second iteration, changed in three lines: where it logs, which script's lifetime it follows, and the directory of the trial workspaces. Every 2 seconds it ends any process above 4 GB of resident memory whose working directory is inside one of this measurement's trial workspaces. It leaves the grader alone, which has its own 60-second limit |
| Machine | one Linux host with 12 hardware threads and 15 GB of memory, shared with other work |

The settings are those of the first measurement. `--jobs 8` and
`--timeout 1200` were chosen there, before its first counted run, and are
kept so that the two measurements differ in as little as possible.

Per run (times in UTC on 2026-10-10; load average is 1, 5 and 15 minutes;
available memory was sampled every 20 seconds):

| Run | Seed | `--jobs` | `--timeout` (s) | Start (UTC) | End (UTC) | Load average at start | Load average at end | Available memory at start (MB) | Free disk at start (GB) | Lowest available memory (MB) | Manifest |
| --- | ---: | ---: | ---: | --- | --- | --- | --- | ---: | ---: | ---: | --- |
| 1 | 1 | 8 | 1200 | 21:47:09 | 21:58:54 | 1.89 4.99 7.57 | 0.53 1.10 3.91 | 8981 | 40 | 7884 | `run-1/harness-bench-2026-10-10T214710Z.json` |
| 2 | 2 | 8 | 1200 | 21:58:54 | 22:09:40 | 0.53 1.10 3.91 | 0.42 1.37 2.88 | 8645 | 40 | 7629 | `run-2/harness-bench-2026-10-10T215855Z.json` |
| 3 | 3 | 8 | 1200 | 22:09:40 | 22:21:18 | 0.42 1.37 2.88 | 0.79 0.97 1.80 | 8832 | 40 | 7687 | `run-3/harness-bench-2026-10-10T220941Z.json` |
| 4 | 4 | 8 | 1200 | 22:21:18 | 22:32:13 | 0.79 0.97 1.80 | 0.56 0.76 1.33 | 8734 | 40 | 1079 | `run-4/harness-bench-2026-10-10T222119Z.json` |
| 5 | 5 | 8 | 1200 | 22:32:13 | 22:48:07 | 0.56 0.76 1.33 | 1.38 1.54 1.65 | 9202 | 39 | 1215 | `run-5/harness-bench-2026-10-10T223214Z.json` |

Before each run the script waited for at least 4,000 MB of available memory
and 15 GB of free disk and recorded both; `--jobs` was never lowered. Each
manifest was copied out of the working tree as soon as its run ended and
before the next run started. The counting rule was committed at 21:45:17
UTC; run 1 started at 21:47:09.

## Results per model pair, runs 1 to 5

Printed by `evals/harness_bench_compare.py --trials` from the five saved
manifests, unedited.

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
| Hidden checks passed, mean | 100.0 % (10/10) | 99.7 % (10/10) | -0.3 pp (10 pairs) |
| Hidden checks per run | gridcalc: 154/154, 154/154, 154/154, 154/154, 154/154; matchbook: 119/119, 119/119, 119/119, 119/119, 119/119 | gridcalc: 154/154, 154/154, 154/154, 154/154, 154/154; matchbook: 119/119, 117/119, 119/119, 119/119, 117/119 | |
| Uncached input tokens, mean | 254 (10/10) | 30 (10/10) | -224 (10 pairs) |
| Output tokens, mean | 120,216 (10/10) | 112,406 (10/10) | -7,810 (10 pairs) |
| Cache read tokens, mean | 2,637,077 (10/10) | 1,660,749 (10/10) | -976,328 (10 pairs) |
| Cache write tokens, mean | 146,370 (10/10) | 136,824 (10/10) | -9,546 (10 pairs) |
| API-equivalent USD, mean | 0.3511 (billed; 10/10 priced) | 0.2810 (billed; 10/10 priced) | -0.0701 (10 pairs) |
| Wall seconds, mean (min to max) | 536.1 (259.9 to 951.6; 10/10) | 470.9 (261.6 to 668.5; 10/10) | -65.2 (10 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

10 native and 10 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

Accuracy is at the ceiling on gridcalc: every run of both sides passed every hidden check, so this pair can show a loss there but not a gain.

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
| Hidden checks passed, mean | 76.7 % (10/10) | 75.9 % (10/10) | -0.8 pp (10 pairs) |
| Hidden checks per run | gridcalc: 132/154, 121/154, 93/154, 140/154, 131/154; matchbook: 94/119, 118/119, 118/119, 106/119, 0/119 | gridcalc: 119/154, 108/154, 116/154, 128/154, 117/154; matchbook: 110/119, 116/119, 119/119, 0/119, 104/119 | |
| Uncached input tokens, mean | 50,968 (10/10) | 73,320 (10/10) | +22,352 (10 pairs) |
| Output tokens, mean | 14,221 (10/10) | 18,810 (10/10) | +4,590 (10 pairs) |
| Cache read tokens, mean | 1,077,325 (10/10) | 1,817,984 (10/10) | +740,659 (10 pairs) |
| Cache write tokens, mean | 0 (10/10) | 0 (10/10) | +0 (10 pairs) |
| API-equivalent USD, mean | 0.0230 (estimated; 10/10 priced) | 0.0349 (billed; 10/10 priced) | +0.0119 (10 pairs) |
| Wall seconds, mean (min to max) | 237.2 (71.4 to 455.3; 10/10) | 227.5 (66.3 to 457.3; 10/10) | -9.8 (10 pairs) |
| Partial-receipt trials (in no token or cost mean) | 0 | 0 | |
| Blocked trials (excluded above) | 0 | 0 | |

10 native and 10 Aether trials. Counts in brackets are measured/total trials. No significance is claimed.

Dollar values are API-equivalent (reported metadata or list-price estimates), not subscription charges. A mean covers only the trials that measured it: a token or cost figure counts as measured only when every unit of its accounting scope reported it at a terminal point. A pair is one case and run within one manifest; trials from different manifests are never paired, so run both sides of a pair in the same block.

## Every trial

Manifests are listed in the order given, each one a separate runner invocation; the Run column is the run within its manifest.

| Manifest | Case | Run | Cell | Workflow | Hidden checks | Uncached input | Output | Cache read | Cache write | USD | Wall seconds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| run-1/harness-bench-2026-10-10T214710Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 66 | 159,867 | 4,716,190 | 185,034 | 0.5873 | 703.7 |
| run-1/harness-bench-2026-10-10T214710Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 54 | 144,392 | 3,548,483 | 170,101 | 0.5019 | 668.5 |
| run-1/harness-bench-2026-10-10T214710Z.json | gridcalc | 1 | native-luna-low | completed | 132/154 | 76,793 | 21,034 | 2,288,128 | 0 | 0.0411 | 455.3 |
| run-1/harness-bench-2026-10-10T214710Z.json | gridcalc | 1 | aether-single-luna-low | completed | 119/154 | 120,498 | 29,188 | 4,253,696 | 0 | 0.0692 | 283.0 |
| run-1/harness-bench-2026-10-10T214710Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 36 | 90,946 | 1,733,743 | 115,652 | 0.2137 | 381.6 |
| run-1/harness-bench-2026-10-10T214710Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 22 | 69,029 | 769,341 | 92,952 | 0.0856 | 286.7 |
| run-1/harness-bench-2026-10-10T214710Z.json | matchbook | 1 | native-luna-low | completed | 94/119 | 37,138 | 8,272 | 518,144 | 0 | 0.0130 | 175.1 |
| run-1/harness-bench-2026-10-10T214710Z.json | matchbook | 1 | aether-single-luna-low | completed | 110/119 | 92,877 | 18,378 | 1,795,328 | 0 | 0.0364 | 135.6 |
| run-2/harness-bench-2026-10-10T215855Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 54 | 146,807 | 3,751,533 | 173,813 | 0.5147 | 628.5 |
| run-2/harness-bench-2026-10-10T215855Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 30 | 155,975 | 2,002,787 | 180,305 | 0.4108 | 643.9 |
| run-2/harness-bench-2026-10-10T215855Z.json | gridcalc | 1 | native-luna-low | completed | 121/154 | 82,906 | 18,567 | 1,844,992 | 0 | 0.0360 | 397.6 |
| run-2/harness-bench-2026-10-10T215855Z.json | gridcalc | 1 | aether-single-luna-low | completed | 108/154 | 83,913 | 26,307 | 1,686,272 | 0 | 0.0384 | 284.1 |
| run-2/harness-bench-2026-10-10T215855Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 28 | 90,788 | 1,249,038 | 117,391 | 0.2466 | 397.6 |
| run-2/harness-bench-2026-10-10T215855Z.json | matchbook | 1 | aether-single-haiku-low | completed | 117/119 | 22 | 63,307 | 728,722 | 86,238 | 0.0562 | 261.6 |
| run-2/harness-bench-2026-10-10T215855Z.json | matchbook | 1 | native-luna-low | completed | 118/119 | 29,921 | 12,584 | 445,184 | 0 | 0.0137 | 251.6 |
| run-2/harness-bench-2026-10-10T215855Z.json | matchbook | 1 | aether-single-luna-low | completed | 116/119 | 39,415 | 9,570 | 289,024 | 0 | 0.0116 | 66.3 |
| run-3/harness-bench-2026-10-10T220941Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 56 | 150,579 | 3,781,374 | 178,342 | 0.5403 | 697.0 |
| run-3/harness-bench-2026-10-10T220941Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 26 | 141,145 | 1,507,224 | 165,912 | 0.3596 | 572.5 |
| run-3/harness-bench-2026-10-10T220941Z.json | gridcalc | 1 | native-luna-low | completed | 93/154 | 31,824 | 8,995 | 343,552 | 0 | 0.0111 | 186.2 |
| run-3/harness-bench-2026-10-10T220941Z.json | gridcalc | 1 | aether-single-luna-low | completed | 116/154 | 73,098 | 21,774 | 2,077,696 | 0 | 0.0390 | 457.3 |
| run-3/harness-bench-2026-10-10T220941Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 22 | 62,902 | 727,751 | 86,382 | 0.0560 | 259.9 |
| run-3/harness-bench-2026-10-10T220941Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 18 | 78,781 | 611,327 | 100,057 | 0.1196 | 324.5 |
| run-3/harness-bench-2026-10-10T220941Z.json | matchbook | 1 | native-luna-low | completed | 118/119 | 39,698 | 12,196 | 590,592 | 0 | 0.0160 | 103.0 |
| run-3/harness-bench-2026-10-10T220941Z.json | matchbook | 1 | aether-single-luna-low | completed | 119/119 | 43,685 | 12,758 | 650,240 | 0 | 0.0172 | 98.5 |
| run-4/harness-bench-2026-10-10T222119Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 34 | 155,600 | 2,222,895 | 178,490 | 0.4011 | 637.0 |
| run-4/harness-bench-2026-10-10T222119Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 40 | 156,633 | 2,837,295 | 180,823 | 0.4636 | 652.3 |
| run-4/harness-bench-2026-10-10T222119Z.json | gridcalc | 1 | native-luna-low | completed | 140/154 | 95,194 | 27,787 | 3,043,840 | 0 | 0.0539 | 253.8 |
| run-4/harness-bench-2026-10-10T222119Z.json | gridcalc | 1 | aether-single-luna-low | completed | 128/154 | 53,626 | 18,657 | 1,033,472 | 0 | 0.0250 | 372.7 |
| run-4/harness-bench-2026-10-10T222119Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 26 | 78,312 | 1,004,311 | 101,277 | 0.1109 | 319.4 |
| run-4/harness-bench-2026-10-10T222119Z.json | matchbook | 1 | aether-single-haiku-low | completed | 119/119 | 28 | 88,146 | 1,230,530 | 114,461 | 0.1845 | 367.3 |
| run-4/harness-bench-2026-10-10T222119Z.json | matchbook | 1 | native-luna-low | completed | 106/119 | 51,421 | 9,969 | 597,248 | 0 | 0.0161 | 213.7 |
| run-4/harness-bench-2026-10-10T222119Z.json | matchbook | 1 | aether-single-luna-low | completed | 0/119 | 60,605 | 17,256 | 967,424 | 0 | 0.0244 | 125.5 |
| run-5/harness-bench-2026-10-10T223214Z.json | gridcalc | 1 | native-haiku-low | completed | 154/154 | 2,196 | 190,517 | 6,176,563 | 229,103 | 0.7315 | 951.6 |
| run-5/harness-bench-2026-10-10T223214Z.json | gridcalc | 1 | aether-single-haiku-low | completed | 154/154 | 38 | 134,563 | 2,449,855 | 162,329 | 0.3933 | 550.3 |
| run-5/harness-bench-2026-10-10T223214Z.json | gridcalc | 1 | native-luna-low | completed | 131/154 | 36,137 | 12,681 | 708,096 | 0 | 0.0170 | 264.5 |
| run-5/harness-bench-2026-10-10T223214Z.json | gridcalc | 1 | aether-single-luna-low | completed | 117/154 | 140,339 | 27,332 | 5,135,872 | 0 | 0.0791 | 300.2 |
| run-5/harness-bench-2026-10-10T223214Z.json | matchbook | 1 | native-haiku-low | completed | 119/119 | 26 | 75,842 | 1,007,372 | 98,211 | 0.1088 | 384.5 |
| run-5/harness-bench-2026-10-10T223214Z.json | matchbook | 1 | aether-single-haiku-low | completed | 117/119 | 22 | 92,089 | 921,927 | 115,061 | 0.2346 | 381.4 |
| run-5/harness-bench-2026-10-10T223214Z.json | matchbook | 1 | native-luna-low | completed | 0/119 | 28,645 | 10,121 | 393,472 | 0 | 0.0119 | 71.4 |
| run-5/harness-bench-2026-10-10T223214Z.json | matchbook | 1 | aether-single-luna-low | completed | 104/119 | 25,139 | 6,882 | 290,816 | 0 | 0.0089 | 151.5 |

A figure reading "at least" comes from incomplete receipts and is a lower bound. The manifest column is the file and its directory, not a full path.

## From the session transcripts

`evals/harness_bench_transcript.py` reads, from each trial's own Claude Code
or Codex session log, figures a manifest does not hold. The logs were read
and not changed. "Before the first edit" runs through the response that
makes the first edit, so that response's own thinking is counted. The Luna
reasoning figures are what the Codex session log reports, which is 0 for
many sessions, as the diagnosis found; they are printed and not
interpreted.

Means over the pairs whose two transcripts could be read, and how the pairs
fall:

| Model | Measure | Native mean | Aether mean | Aether minus native | Relative to native | Pairs: Aether lower / higher / equal | Smallest pair difference | Largest pair difference | Pairs read |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Haiku 5.5 | First-request input tokens | 23,073 | 23,309 | +235 | +1.0 % | 0 / 10 / 0 | +223 | +246 | 10 of 10 |
| Claude Haiku 5.5 | Thinking or reasoning tokens before the first edit | 81,369 | 80,480 | -889 | -1.1 % | 5 / 5 / 0 | -20,014 | +21,856 | 10 of 10 |
| Claude Haiku 5.5 | Thinking or reasoning tokens in total | 91,023 | 86,244 | -4,778 | -5.2 % | 6 / 4 / 0 | -37,430 | +16,978 | 10 of 10 |
| Claude Haiku 5.5 | Requests | 21.0 | 15.0 | -6.0 | -28.6 % | 8 / 2 / 0 | -18 | +4 | 10 of 10 |
| GPT-6 Luna | First-request input tokens | 15,612 | 16,542 | +930 | +6.0 % | 0 / 10 / 0 | +925 | +937 | 10 of 10 |
| GPT-6 Luna | Thinking or reasoning tokens before the first edit | 181 | 0 | -181 | -100.0 % | 2 / 0 / 5 | -1,027 | +0 | 7 of 10 |
| GPT-6 Luna | Thinking or reasoning tokens in total | 1,128 | 233 | -895 | -79.3 % | 5 / 2 / 3 | -4,742 | +245 | 10 of 10 |
| GPT-6 Luna | Requests | 31.3 | 45.3 | +14.0 | +44.7 % | 4 / 6 / 0 | -39 | +80 | 10 of 10 |

- **Haiku.** The first request is larger through Aether in 10 of 10 pairs, by
  223 to 246 tokens (+1.0 %). Thinking before the first edit is lower through
  Aether in 5 pairs and higher in 5 (-1.1 % in the mean). Aether sessions made
  fewer requests: 15.0 against 21.0, fewer in 8 of 10 pairs.
- **Luna.** The first request is larger through Aether in 10 of 10 pairs, by
  925 to 937 tokens (+6.0 %); the diagnosis measured 939 on the earlier
  binary. Aether sessions made more requests in the mean: 45.3 against 31.3,
  more in 6 of 10 pairs and fewer in 4.
- **Unknown.** Reasoning before the first edit reads "unknown" for three Luna
  trials (`fc77bbe2d3`, `73c3950e4d`, `ebdf031604`): the reader found no
  file-change event in those session logs, so it has no first edit to count up
  to. Their three pairs are left out of that row, which is why it reads 7 of
  10.

Per trial:

| Run | Case | Cell | Trial | First-request input tokens | Thinking or reasoning before the first edit | Thinking or reasoning in total | Requests |
| ---: | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | `gridcalc` | `native-haiku-low` | `93a21f04a5` | 23,056 | 105,381 | 125,175 | 33 |
| 1 | `gridcalc` | `aether-single-haiku-low` | `cfc33c32e8` | 23,300 | 93,166 | 109,502 | 27 |
| 1 | `gridcalc` | `native-luna-low` | `fc77bbe2d3` | 15,601 | unknown | 0 | 58 |
| 1 | `gridcalc` | `aether-single-luna-low` | `a49e2e3e9c` | 16,537 | 0 | 245 | 96 |
| 1 | `matchbook` | `native-haiku-low` | `392f6448d0` | 23,088 | 62,023 | 66,774 | 18 |
| 1 | `matchbook` | `aether-single-haiku-low` | `1b9763bf7a` | 23,334 | 48,470 | 50,005 | 11 |
| 1 | `matchbook` | `native-luna-low` | `91329a75ef` | 15,624 | 0 | 174 | 20 |
| 1 | `matchbook` | `aether-single-luna-low` | `38b099069e` | 16,549 | 0 | 0 | 47 |
| 2 | `gridcalc` | `native-haiku-low` | `4d0a2a05b1` | 23,060 | 93,852 | 107,979 | 27 |
| 2 | `gridcalc` | `aether-single-haiku-low` | `a7627ab284` | 23,289 | 115,708 | 124,957 | 15 |
| 2 | `gridcalc` | `native-luna-low` | `8d45a09282` | 15,601 | 0 | 0 | 49 |
| 2 | `gridcalc` | `aether-single-luna-low` | `25f5a30486` | 16,526 | 0 | 0 | 45 |
| 2 | `matchbook` | `native-haiku-low` | `31e9c6bf36` | 23,092 | 63,902 | 66,747 | 14 |
| 2 | `matchbook` | `aether-single-haiku-low` | `8c8124ad20` | 23,329 | 45,215 | 46,458 | 11 |
| 2 | `matchbook` | `native-luna-low` | `52d9edca36` | 15,626 | 516 | 2,515 | 17 |
| 2 | `matchbook` | `aether-single-luna-low` | `73c3950e4d` | 16,554 | unknown | 0 | 11 |
| 3 | `gridcalc` | `native-haiku-low` | `b2ceadbfb6` | 23,060 | 89,020 | 110,792 | 28 |
| 3 | `gridcalc` | `aether-single-haiku-low` | `7ada8e1680` | 23,288 | 107,039 | 109,876 | 13 |
| 3 | `gridcalc` | `native-luna-low` | `ebdf031604` | 15,597 | unknown | 0 | 14 |
| 3 | `gridcalc` | `aether-single-luna-low` | `1cb6ecec97` | 16,532 | 0 | 0 | 52 |
| 3 | `matchbook` | `native-haiku-low` | `9dbcf5e63d` | 23,094 | 45,071 | 45,760 | 11 |
| 3 | `matchbook` | `aether-single-haiku-low` | `2176794c31` | 23,317 | 60,221 | 61,651 | 9 |
| 3 | `matchbook` | `native-luna-low` | `22cdcdbe66` | 15,624 | 1,027 | 2,720 | 21 |
| 3 | `matchbook` | `aether-single-luna-low` | `80775845e9` | 16,550 | 0 | 2,042 | 24 |
| 4 | `gridcalc` | `native-haiku-low` | `8837c88164` | 23,052 | 128,030 | 128,847 | 16 |
| 4 | `gridcalc` | `aether-single-haiku-low` | `f1b078ea88` | 23,292 | 115,194 | 124,475 | 20 |
| 4 | `gridcalc` | `native-luna-low` | `5b40bec178` | 15,599 | 0 | 4,742 | 70 |
| 4 | `gridcalc` | `aether-single-luna-low` | `4782bd229e` | 16,524 | 0 | 0 | 31 |
| 4 | `matchbook` | `native-haiku-low` | `15df945e4e` | 23,088 | 49,891 | 55,993 | 13 |
| 4 | `matchbook` | `aether-single-haiku-low` | `450007ac83` | 23,316 | 60,013 | 62,865 | 14 |
| 4 | `matchbook` | `native-luna-low` | `49fac87e2b` | 15,626 | 0 | 0 | 23 |
| 4 | `matchbook` | `aether-single-luna-low` | `f2f5c8702a` | 16,559 | 0 | 0 | 29 |
| 5 | `gridcalc` | `native-haiku-low` | `8ed362f948` | 23,054 | 120,355 | 143,824 | 37 |
| 5 | `gridcalc` | `aether-single-haiku-low` | `8f88ca7f63` | 23,299 | 100,341 | 106,394 | 19 |
| 5 | `gridcalc` | `native-luna-low` | `5f5658bb1c` | 15,603 | 0 | 0 | 26 |
| 5 | `gridcalc` | `aether-single-luna-low` | `0c3e572967` | 16,529 | 0 | 44 | 106 |
| 5 | `matchbook` | `native-haiku-low` | `43ff7b2202` | 23,090 | 56,169 | 58,335 | 13 |
| 5 | `matchbook` | `aether-single-haiku-low` | `a68e2c9492` | 23,323 | 59,437 | 66,261 | 11 |
| 5 | `matchbook` | `native-luna-low` | `338b338dab` | 15,622 | 239 | 1,134 | 15 |
| 5 | `matchbook` | `aether-single-luna-low` | `90c55e41f9` | 16,559 | 0 | 0 | 12 |

## Timeouts, failures, guard firings and unknown figures

- **Timeouts and workflows not finished:** none of 40. The longest trial took
  951.6 seconds of the 1200 allowed (`native-haiku-low` on `gridcalc`, run 5);
  the next longest took 703.7 (`native-haiku-low` on `gridcalc`, run 1).
- **Memory guard:** ran beside every run and never fired: it ended no process.
- **Grades of 0 because the grader ran out of time:** `aether-single-luna-low`
  on `matchbook`, run 4 (`f2f5c8702a`): 0/119, "hidden tests timed out after
  60s"; `native-luna-low` on `matchbook`, run 5 (`338b338dab`): 0/119, "hidden
  tests timed out after 60s". The 0 is the counted figure. The per-test second
  look is in the next section.
- **Environment faults** (rate or usage limit, out of memory, daemon start):
  none seen. No trial was rerun and no run was left out.
- **Partial receipts, unknown or unpriced figures:** none. Every mean is over
  10 of 10 trials.
- **Blocked trials:** none.
- **Integrity:** no trial reports a changed benchmark input.
- **Contract flags:** none.
- **Transcripts the reader could not read:** none.
- **Memory pressure, twice.** Available memory was sampled every 20 seconds.
  In run 4 it fell from 7,636 MB at 22:23:19 UTC to 1,079 MB at 22:24:19 and
  was back at 8,690 MB at 22:24:39. In run 5 it fell from 8,106 MB at 22:33:13
  to 1,215 MB at 22:34:13 and was back at 9,559 MB at 22:34:34. In runs 1 to 3
  it never went below 7,629 MB. Each fall coincides with the 60 seconds in
  which the grader ran the hidden tests of a solution that hung:
  `aether-single-luna-low` on `matchbook` in run 4, whose solver finished
  after 125.5 seconds, and `native-luna-low` on `matchbook` in run 5, whose
  solver finished after 71.4. The guard leaves the grader alone by its design,
  so it ended nothing. No record per process was kept, so that the grader's
  process held the memory is inferred from the timing and was not observed.
  The other trials of both runs were in flight at the time; all finished their
  workflows. Whether the pressure changed any of their figures is not known.
- **Closest to the timeout:** `native-haiku-low` on `gridcalc` in run 5, 951.6
  of 1200 seconds. It finished its workflow and passed 154/154.

The runner's own `status` field marks a trial `failed` unless it passes every
hidden check, so most Luna rows read `failed` in the manifests although their
workflows finished. The tables above use the workflow and the check counts.

## The second look at hung grades

The grader runs all hidden tests of a case in one process with one limit of
60 seconds, and reports 0 when that limit is reached. For each counted trial
graded 0 in that way, every hidden test was run again in its own process
with a limit of 5 seconds, on a copy of the files the trial left (the script
of the diagnosis's second iteration, extended to take the case). This is a
second look. It is not a grade under the frozen protocol and it replaces no
counted figure: the trial stays at 0 in every table and mean of this
report.

| Run | Case | Cell | Trial | Counted grade | Passed alone | Failed alone | Hung alone (5 s each) | Tests that hung |
| ---: | --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| 4 | `matchbook` | `aether-single-luna-low` | `f2f5c8702a` | 0/119 | 107 | 10 | 2 | `IcebergInvariantTests.test_random_flow_replays_to_the_same_engine`, `IcebergInvariantTests.test_tranches_stay_consistent_under_random_flow` |
| 5 | `matchbook` | `native-luna-low` | `338b338dab` | 0/119 | 110 | 8 | 1 | `InvariantTests.test_filled_quantities_equal_the_trade_tape` |

The Aether trial of run 4 passes 107 of its 119 tests when each runs alone,
fails 10 and hangs in 2. The Codex trial of run 5 passes 110, fails 8 and
hangs in 1, a different test. In the grader's one process a single hanging
test uses up the whole 60 seconds, so both were graded 0, as the frozen
protocol has it. Graded per test they would stand at 107 and 110 of 119; no
table and no mean in this report uses those numbers. During the second look,
with 5 seconds per test, available memory did not go below 8,131 MB.

## Counting rule

Written down and committed before the first counted trial of this
measurement started, and not changed since. Rules 1 to 6 are the rules of the
[first measurement](harness-lower-tier-results-2026-10-10.md), as that report
quotes them:

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

7. Runs 1 to 5 of protocol version 2 are counted whole, whatever they show.
   No run is added, dropped or repeated on the strength of a result. A trial
   ended by the memory guard is counted as it stands and listed.
8. Five runs are planned, with seeds 1 to 5, launched one after another. The
   number is fixed here, before the first run, and by the clock alone: a run
   is started only before 23:08 UTC on 2026-10-10, minute 88 of the sitting.
   If that limit keeps the fifth run from starting, four runs are counted and
   this report says so. No result is looked at to decide it.
9. A run whose manifest has been saved is never made again.
10. The one trial that checks Aether's `complete_task` call under protocol
    version 2 (`diag-aether-haiku-low-complete-task` on the older `ledger`
    case) is diagnostic and is never counted. Neither counted case is smoked
    or piloted before run 1.
11. A counted trial graded 0 because the grader ran out of time keeps its 0
    in every counted figure. A per-test second look at such a trial is
    reported beside the counted figure and never in place of it.

Separately, the report tool keeps its own frozen rule: a trial the runner
classifies as `blocked` (the host refused a tool call and the agent idled) is
left out of the means and counted in its own row.

## Not counted: the `complete_task` check

Protocol version 2 starts an Aether Claude session with
`--strict-mcp-config`. In the diagnosis all six such trials listed Aether's
20 tools and none called one, so it had not been observed that Aether's own
MCP server answers a call under the flag. That was checked once, before the
first counted run:

- **The trial.** One trial of `diag-aether-haiku-low-complete-task` on the
  older `ledger` case (22 hidden checks), run with `--diagnostic`. The arm is
  `aether-single-haiku-low` with one sentence added to the task: "When the
  work is done, finish by calling the Aether tool `complete_task` with a
  one-line summary."
- **What the records show.** The receipt records that the shim added
  `--strict-mcp-config`. The isolated daemon's event record holds the
  solver's call of `mcp__aether__complete_task`: requested at 21:46:42.026
  UTC, the task completed at 21:46:42.035, the tool call succeeded at
  21:46:42.043 with the reply that the task was completed, and the agent
  completed at 21:46:45.954.
- **The trial itself** finished its workflow in 86.6 seconds and passed
  22/22. Its manifest carries `diagnostic: true`, which the comparison and
  report tools refuse, and `ledger` is not a counted case.

So Aether's own server takes a call under the stricter flag, and the counted
Aether Haiku cell is Aether as the benchmark starts it. One trial shows that
the call works. It says nothing about how often a solver makes the call
unasked. In the counted trials, the event records of 20 of 20 Aether trials
were read: 2 of them hold a call of an Aether tool (`8c8124ad20`:
`mcp__aether__complete_task`; `450007ac83`: `mcp__aether__complete_task`).

No smoke and no pilot of `matchbook` or `gridcalc` was made in this sitting.

## Limits

- **Few runs.** 5 runs per cell and case give 10 pairs per model.
  Every mean here is a mean of that many trials, and one trial moves it
  visibly. No significance is claimed, and none should be read into a
  difference or into the absence of one.
- **Two measurements are not one experiment.** The first measurement and
  this one are two sittings with a different binary and a different MCP set.
  The comparison between them is not controlled, and their trials are never
  pooled: the tools refuse to merge manifests of two protocol versions.
- **The stop rule's 5 % bar is finer than this sample.** See
  [The stop rule](#the-stop-rule).
- **One machine, one evening.** Provider load, cache warmth and the other
  work on this host are not controlled. Other agents used the same machine
  and the same subscriptions while the trials ran.
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
- **The Aether session is still not the native session plus a wrapper.**
  Aether starts the same CLI with its own settings, its coordination tools
  over MCP, the task inside its own first message, and the CLI's own
  delegation tools switched off. Protocol version 2 removes one inequality,
  the connectors, and the binary removes one, the effort words. A remaining
  difference can come from any of the rest, or from chance. These runs do
  not say which.
- **The benchmark's Aether is not every operator's Aether.** Outside the
  benchmark an Aether agent on Claude Code still loads the operator's own
  MCP connectors. Version 2 keeps them out so that the pair starts equal,
  not because Aether does.
- **The grader's one limit.** All hidden tests of a case run in one process
  with 60 seconds; a solution that hangs in one test is graded 0. The
  counted figures keep that 0.
- **Two cases, one kind of task.** Both are Python repair tasks with a public
  contract. Nothing here speaks to delegation, long sessions, compaction or
  other languages.
- **Ceilings.** The comparison tool prints a ceiling note under a pair's
  table only when every run of both sides passed every check on a case. The
  summary says how many trials of each cell passed every check; where most
  do, equal scores are not evidence that nothing is lost.

## Reproduce

Live runs spend model tokens on the logged-in Claude and Codex accounts.

```sh
cargo build --release
python3 -m unittest discover -s evals -p 'test_harness_bench*.py'   # no model calls

# One counted run: the matched set of 8 trials at once. Repeat with seeds 1 to 5 and its own run-<n> directory.
python3 evals/harness_bench.py --cases matchbook,gridcalc --runs 1 --seed 1 --jobs 8 --timeout 1200 \
  --conditions native-haiku-low,aether-single-haiku-low,native-luna-low,aether-single-luna-low \
  --binary target/release/aether --output-dir target/harness-393-v2/run-1

# The tables and the per-trial list in this report.
python3 evals/harness_bench_compare.py target/harness-393-v2/run-*/harness-bench-*.json --trials

# The figures from the session transcripts.
python3 evals/harness_bench_transcript.py target/harness-393-v2/run-*/harness-bench-*.json

# The check of Aether's complete_task call under protocol version 2; diagnostic, never counted.
python3 evals/harness_bench.py --diagnostic --cases ledger --runs 1 --seed 1 --jobs 1 --timeout 600 \
  --conditions diag-aether-haiku-low-complete-task \
  --binary target/release/aether --output-dir target/harness-393-v2/pilot-complete-task
```

The manifests and per-trial artifacts are ignored local files under
`target/harness-393-v2/`; this report carries the per-trial table. The
manifests of this measurement were also kept outside the working tree, so
its tables can be recomputed from them. A new run gives new numbers: the
models are not deterministic, and provider load differs.
