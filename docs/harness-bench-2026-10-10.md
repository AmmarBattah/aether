# Native CLI versus Aether: Haiku 5.5 and GPT-6 Luna — 2026-10-10

> **Note, 2026-10-11.** These trials were measured under benchmark protocol
> version 1, before the changes that made protocol version 2. A later sitting
> of the same 72 trials on the merged tree is reported in
> [harness-bench-2026-10-11.md](harness-bench-2026-10-11.md).

This benchmark asks whether a model loses accuracy, tokens, cost, or time
when it runs as an Aether agent instead of directly in its own CLI. Each pair
uses the same model, the same requested effort, and the same CLI engine:

- **Claude Haiku 5.5** (`claude-haiku-5-5`): native Claude Code versus
  Aether `claude-code/claude-haiku-5-5`, medium effort requested on both sides.
- **GPT-6 Luna** (`gpt-6-luna`): native Codex versus Aether
  `codex/gpt-6-luna`, medium effort on both sides.

Each Aether trial is a single agent with no delegation, so the comparison
measures harness overhead, not multi-agent coordination.

## Method

- **Tasks:** six repair tasks, each graded by held-out checks that are never
  copied into a solver's workspace. Three are new and were added because the
  older cases were close to saturated:
  - `billing` (Decimal invoicing, 79 checks);
  - `semver` (SemVer ranges with a backtracking resolver, 80 checks);
  - `minilang` (a small interpreter with closures and precise error positions, 102 checks).

  Their buggy starters fail 70%, 58%, and 56% of the hidden checks, and their
  separate reference implementations pass every check. The other three are the
  existing `concurrent_v2` (67), `eventlog` (49), and `leases` (28) cases.
  The new cases were frozen before any counted trial; none was piloted.
- **Runs:**
  - 3 runs per case and condition: 72 trials in total.
  - Seed 393, with seeded run-major latin rotation order and a 900 s timeout.
  - Every trial got a fresh Git workspace and its own isolated Aether daemon.
  - All trials ran from clean commit `5bc4d8d` with Aether binary SHA-256
    `fe7d3f2a8eb9dc4633e7f2a4b1811658d7a0ee968ac6df33a2dfc96dbf923803`,
    Claude Code 2.1.295, and the installed Codex CLI. Hashes are recorded in
    the manifests.
- **Lanes:** the Haiku pair and the Luna pair ran as two concurrent lanes.
  Within a lane, trials ran one at a time and alternated native and Aether, so
  each pair shared the same load conditions. Compare wall time within a model,
  not across models.
- **Measures:**
  - **Accuracy:** the fraction of hidden checks passed per trial, averaged.
    A *full pass* is every check passed.
  - **Tokens:** uncached input, output, cache reads, and cache writes; total
    tokens is their sum. Claude Code caches nearly all input, so Haiku's input
    appears almost entirely in the cache columns. Codex reports no cache writes.
  - **Cost:** API-equivalent USD, which is not a subscription charge.
    - Haiku costs are Claude Code's reported dollar metadata on both sides;
      the Aether side uses all-model Claude process receipts.
    - Luna costs use list prices from the pinned binary's price table:
      estimated from Codex receipts on the native side, priced by Aether's
      runtime on the Aether side.
  - **Time:** wall seconds from launch to finished workflow.
- **Model check:** all 36 Haiku trials resolved `claude-haiku-5-5`. All 36
  Luna trials resolved `gpt-6-luna` at medium effort. Claude's init events do
  not report effort, so Haiku's medium effort is requested, not confirmed.
- **Integrity:** all 72 workflows finished before the timeout, with no harness,
  grading, usage, input-integrity, or protocol error.

## Results

| Model | Harness | Trials | Mean accuracy | Full passes | Finished workflows | Uncached input | Output | Cache reads | Cache writes | Total tokens | API-equiv. USD | Wall s |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Haiku 5.5 | Native CLI | 18 | 99.6% | 13/18 | 18/18 | 28 | 52,924 | 897,620 | 75,907 | 1,026,479 | $0.0622 | 226 |
| Claude Haiku 5.5 | Aether | 18 | 99.6% | 12/18 | 18/18 | 25 | 39,819 | 678,727 | 64,249 | 782,820 | $0.0395 | 171 |
| Claude Haiku 5.5 | **Aether − native** | | -0.1 pp | -1 | +0 | -12% | -25% | -24% | -15% | -24% | -36% | -25% |
| GPT-6 Luna | Native CLI | 18 | 97.5% | 6/18 | 18/18 | 31,943 | 7,149 | 359,751 | 0 | 398,842 | $0.0104 | 101 |
| GPT-6 Luna | Aether | 18 | 93.2% | 3/18 | 18/18 | 31,221 | 6,148 | 266,069 | 0 | 303,439 | $0.0089 | 98 |
| GPT-6 Luna | **Aether − native** | | -4.3 pp | -3 | +0 | -2% | -14% | -26% | n/a | -24% | -15% | -3% |

Mean accuracy by case, with the checks passed in each of the three runs:

| Case | Checks | Claude Haiku 5.5 native | Claude Haiku 5.5 Aether | GPT-6 Luna native | GPT-6 Luna Aether |
| --- | --- | --- | --- | --- | --- |
| `billing` | 79 | 98.7% (78, 78, 78) | 98.7% (78, 78, 78) | 93.7% (67, 78, 77) | 98.3% (78, 78, 77) |
| `concurrent_v2` | 67 | 99.5% (67, 67, 66) | 99.0% (67, 66, 66) | 97.5% (64, 66, 66) | 99.0% (66, 66, 67) |
| `eventlog` | 49 | 100.0% (49, 49, 49) | 100.0% (49, 49, 49) | 98.0% (49, 49, 46) | 98.0% (48, 48, 48) |
| `leases` | 28 | 100.0% (28, 28, 28) | 100.0% (28, 28, 28) | 98.8% (28, 27, 28) | 67.9% (27, 3, 27) |
| `minilang` | 102 | 100.0% (102, 102, 102) | 100.0% (102, 102, 102) | 98.7% (102, 98, 102) | 99.3% (102, 102, 100) |
| `semver` | 80 | 99.6% (80, 80, 79) | 99.6% (80, 80, 79) | 98.3% (79, 78, 79) | 96.7% (76, 77, 79) |

Paired by case and run index:

- **Haiku:** 17 of 18 pairs tied on accuracy. In the one remaining pair
  (`concurrent_v2` run 2), Aether passed one check fewer. The median paired
  difference was $0.0085 cheaper and 49 s faster through Aether.
- **Luna:** Aether scored higher in 5 pairs, lower in 8, and tied in 5. The
  median paired accuracy difference was 0.0 points, the median paired cost
  difference was $0.0010 cheaper, and the median paired time difference was
  4 s slower.

## Reading the results

- **Haiku 5.5: no accuracy loss, lower cost.** Accuracy was equal, at 99.6%
  on both sides. Through Aether, total tokens were 24% lower, cost 36% lower,
  and mean wall time 25% lower. Haiku is near the ceiling on these cases (13
  versus 12 full passes out of 18), so this pair has little accuracy headroom
  to show a difference.
- **Three native Haiku runs drive part of the cost gap.** Native
  `concurrent_v2` run 1 and `minilang` runs 2 and 3 cost about twice as much
  per token as the other native runs. They were the only Haiku runs, on
  either side, whose peak prompt exceeded 105k tokens; every Aether peak
  stayed under 87k. Without those three pairs, Aether was still 18% cheaper,
  used 17% fewer tokens, and took 19% less time. Costs are Claude Code's
  reported amounts and were not re-derived.
- **GPT-6 Luna: fewer tokens and lower cost, mixed accuracy.** Through Aether,
  total tokens were 24% lower and cost 15% lower, with similar wall time.
  Mean accuracy was 93.2% against 97.5%, and full passes 3 against 6.
- **One run causes most of the Luna gap.** In `leases` run 2, Luna garbled
  the absolute path in an `apply_patch` call:
  - It deleted the real file and wrote the replacement outside the trial
    workspace.
  - It then restored the original from Git and reported that it had failed,
    so the starter remained (3/28).
  - Without that run, Luna's mean accuracy is 98.0% through Aether and 97.6%
    natively. Full passes stay lower through Aether (3 versus 6), mostly on
    `eventlog` and `leases`, where Aether runs repeatedly missed one check.

  The run still counts in every headline number above.
- **No harness reached 100%.** Every condition missed checks on at least one
  case.

The benchmark measures these differences but does not explain them; their
cause has not been investigated.

## Limits

- Three runs per case is a small sample, and no significance test was run.
  Pair outcomes can change with provider load and cache warmth.
- **Haiku tokens:** both sides come from Claude process receipts, so they are
  comparable.
- **Luna tokens:** the two sides come from different sources. Native counts
  come from Codex rollout receipts. Aether counts come from its durable
  counters, which label themselves as possibly omitting helper models; these
  trials had no descendants.
- **Out-of-workspace write:** both the native benchmark command and Aether
  launch Codex with `--dangerously-bypass-approvals-and-sandbox`, so either
  side could make the same write. The README already notes that native
  file operations do not pass through Aether's tool guards. The stray file
  was moved into that trial's artifacts.
- **Lanes:** the two lanes ran concurrently, unlike the protocol's default of
  strictly sequential trials. Pairs within a lane remain matched.

## Reproduce

```sh
python3 -m unittest discover -s evals -p 'test_harness_bench*.py'
python3 evals/harness_bench.py --runs 3 --seed 393 --timeout 900 \
  --cases billing,semver,minilang,concurrent_v2,eventlog,leases \
  --conditions native-haiku-medium,aether-single-haiku-medium \
  --binary target/release/aether --output-dir target/results/claude-lane
# Same command with native-luna-medium,aether-single-luna-medium for the Luna lane.
python3 evals/harness_bench_report.py target/results/*/harness-bench-*.json \
  --output-dir target/report
```

The runner checks that each daemon socket path is shorter than 100 bytes. From
a deeply nested checkout, run it through a short symlink to the repository, and
invoke it by that absolute path. The report writes `summary.md` (by
condition and by case), `summary.json`, `results.csv`, and `index.html`.
The manifests and per-trial artifacts are ignored local files under
`target/hb393-frozen/target/results/{claude,codex}-lane/`. Live runs spend
model tokens: this matrix cost about $2.18 API-equivalent over about three
hours of solve time.

## Every trial

| Model | Harness | Case | Run | Checks | Workflow | Uncached input | Output | Cache reads | Cache writes | API-equiv. USD | Wall s |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Haiku 5.5 | Native CLI | `billing` | 1 | 78/79 | completed | 18 | 42,863 | 459,647 | 65,517 | $0.0391 | 169 |
| Claude Haiku 5.5 | Aether | `billing` | 1 | 78/79 | completed | 18 | 31,500 | 402,724 | 56,207 | $0.0310 | 126 |
| Claude Haiku 5.5 | Native CLI | `billing` | 2 | 78/79 | completed | 28 | 36,416 | 694,489 | 58,274 | $0.0368 | 147 |
| Claude Haiku 5.5 | Aether | `billing` | 2 | 78/79 | completed | 22 | 48,248 | 616,282 | 72,766 | $0.0448 | 183 |
| Claude Haiku 5.5 | Native CLI | `billing` | 3 | 78/79 | completed | 26 | 47,472 | 779,636 | 71,280 | $0.0458 | 188 |
| Claude Haiku 5.5 | Aether | `billing` | 3 | 78/79 | completed | 22 | 38,886 | 578,582 | 63,718 | $0.0380 | 170 |
| Claude Haiku 5.5 | Native CLI | `concurrent_v2` | 1 | 67/67 | completed | 24 | 74,680 | 896,423 | 97,423 | $0.1159 | 310 |
| Claude Haiku 5.5 | Aether | `concurrent_v2` | 1 | 67/67 | completed | 28 | 36,765 | 739,931 | 60,043 | $0.0378 | 177 |
| Claude Haiku 5.5 | Native CLI | `concurrent_v2` | 2 | 67/67 | completed | 30 | 64,330 | 1,037,298 | 83,970 | $0.0593 | 276 |
| Claude Haiku 5.5 | Aether | `concurrent_v2` | 2 | 66/67 | completed | 28 | 46,558 | 809,806 | 69,130 | $0.0452 | 202 |
| Claude Haiku 5.5 | Native CLI | `concurrent_v2` | 3 | 66/67 | completed | 30 | 65,280 | 1,026,307 | 88,071 | $0.0605 | 275 |
| Claude Haiku 5.5 | Aether | `concurrent_v2` | 3 | 66/67 | completed | 22 | 49,563 | 628,273 | 73,754 | $0.0458 | 206 |
| Claude Haiku 5.5 | Native CLI | `eventlog` | 1 | 49/49 | completed | 20 | 36,544 | 467,747 | 56,271 | $0.0342 | 150 |
| Claude Haiku 5.5 | Aether | `eventlog` | 1 | 49/49 | completed | 16 | 28,829 | 307,952 | 59,109 | $0.0293 | 121 |
| Claude Haiku 5.5 | Native CLI | `eventlog` | 2 | 49/49 | completed | 20 | 32,417 | 441,557 | 50,790 | $0.0308 | 139 |
| Claude Haiku 5.5 | Aether | `eventlog` | 2 | 49/49 | completed | 12 | 29,698 | 206,412 | 48,465 | $0.0266 | 120 |
| Claude Haiku 5.5 | Native CLI | `eventlog` | 3 | 49/49 | completed | 20 | 38,599 | 476,911 | 57,317 | $0.0355 | 161 |
| Claude Haiku 5.5 | Aether | `eventlog` | 3 | 49/49 | completed | 14 | 25,757 | 254,282 | 45,177 | $0.0245 | 107 |
| Claude Haiku 5.5 | Native CLI | `leases` | 1 | 28/28 | completed | 24 | 42,751 | 605,166 | 60,449 | $0.0395 | 211 |
| Claude Haiku 5.5 | Aether | `leases` | 1 | 28/28 | completed | 32 | 36,327 | 855,618 | 59,826 | $0.0387 | 157 |
| Claude Haiku 5.5 | Native CLI | `leases` | 2 | 28/28 | completed | 26 | 44,464 | 708,145 | 62,981 | $0.0419 | 189 |
| Claude Haiku 5.5 | Aether | `leases` | 2 | 28/28 | completed | 44 | 40,649 | 1,272,891 | 61,055 | $0.0453 | 187 |
| Claude Haiku 5.5 | Native CLI | `leases` | 3 | 28/28 | completed | 20 | 41,055 | 492,978 | 59,863 | $0.0374 | 172 |
| Claude Haiku 5.5 | Aether | `leases` | 3 | 28/28 | completed | 18 | 34,632 | 403,342 | 54,700 | $0.0323 | 146 |
| Claude Haiku 5.5 | Native CLI | `minilang` | 1 | 102/102 | completed | 30 | 56,983 | 1,044,567 | 84,130 | $0.0558 | 243 |
| Claude Haiku 5.5 | Aether | `minilang` | 1 | 102/102 | completed | 34 | 46,484 | 1,088,530 | 75,860 | $0.0493 | 211 |
| Claude Haiku 5.5 | Native CLI | `minilang` | 2 | 102/102 | completed | 56 | 68,807 | 2,146,043 | 101,121 | $0.1361 | 351 |
| Claude Haiku 5.5 | Aether | `minilang` | 2 | 102/102 | completed | 30 | 41,274 | 878,169 | 70,806 | $0.0436 | 183 |
| Claude Haiku 5.5 | Native CLI | `minilang` | 3 | 102/102 | completed | 38 | 73,208 | 1,550,441 | 111,032 | $0.1621 | 317 |
| Claude Haiku 5.5 | Aether | `minilang` | 3 | 102/102 | completed | 34 | 46,054 | 1,094,703 | 75,223 | $0.0490 | 204 |
| Claude Haiku 5.5 | Native CLI | `semver` | 1 | 80/80 | completed | 24 | 59,286 | 761,263 | 82,527 | $0.0538 | 244 |
| Claude Haiku 5.5 | Aether | `semver` | 1 | 80/80 | completed | 20 | 48,924 | 572,663 | 73,387 | $0.0449 | 200 |
| Claude Haiku 5.5 | Native CLI | `semver` | 2 | 80/80 | completed | 28 | 63,390 | 980,223 | 85,745 | $0.0586 | 264 |
| Claude Haiku 5.5 | Aether | `semver` | 2 | 80/80 | completed | 24 | 44,343 | 705,751 | 70,216 | $0.0433 | 180 |
| Claude Haiku 5.5 | Native CLI | `semver` | 3 | 79/80 | completed | 42 | 64,079 | 1,588,328 | 89,559 | $0.0766 | 270 |
| Claude Haiku 5.5 | Aether | `semver` | 3 | 79/80 | completed | 28 | 42,257 | 801,182 | 67,033 | $0.0425 | 190 |
| GPT-6 Luna | Native CLI | `billing` | 1 | 67/79 | completed | 23,548 | 5,740 | 305,408 | 0 | $0.0083 | 68 |
| GPT-6 Luna | Aether | `billing` | 1 | 78/79 | completed | 37,366 | 9,615 | 286,208 | 0 | $0.0114 | 199 |
| GPT-6 Luna | Native CLI | `billing` | 2 | 78/79 | completed | 23,708 | 5,947 | 398,592 | 0 | $0.0093 | 65 |
| GPT-6 Luna | Aether | `billing` | 2 | 78/79 | completed | 23,143 | 5,384 | 282,112 | 0 | $0.0078 | 120 |
| GPT-6 Luna | Native CLI | `billing` | 3 | 77/79 | completed | 41,705 | 8,581 | 496,384 | 0 | $0.0134 | 95 |
| GPT-6 Luna | Aether | `billing` | 3 | 77/79 | completed | 41,253 | 5,079 | 262,912 | 0 | $0.0093 | 119 |
| GPT-6 Luna | Native CLI | `concurrent_v2` | 1 | 64/67 | completed | 18,534 | 5,611 | 177,664 | 0 | $0.0064 | 58 |
| GPT-6 Luna | Aether | `concurrent_v2` | 1 | 66/67 | completed | 28,555 | 6,972 | 275,968 | 0 | $0.0091 | 75 |
| GPT-6 Luna | Native CLI | `concurrent_v2` | 2 | 66/67 | completed | 33,430 | 5,680 | 235,264 | 0 | $0.0085 | 65 |
| GPT-6 Luna | Aether | `concurrent_v2` | 2 | 66/67 | completed | 31,205 | 5,777 | 264,704 | 0 | $0.0087 | 61 |
| GPT-6 Luna | Native CLI | `concurrent_v2` | 3 | 66/67 | completed | 25,041 | 6,052 | 278,016 | 0 | $0.0083 | 136 |
| GPT-6 Luna | Aether | `concurrent_v2` | 3 | 67/67 | completed | 33,451 | 6,454 | 265,984 | 0 | $0.0092 | 138 |
| GPT-6 Luna | Native CLI | `eventlog` | 1 | 49/49 | completed | 20,282 | 5,091 | 198,912 | 0 | $0.0066 | 58 |
| GPT-6 Luna | Aether | `eventlog` | 1 | 48/49 | completed | 21,709 | 4,018 | 170,496 | 0 | $0.0059 | 95 |
| GPT-6 Luna | Native CLI | `eventlog` | 2 | 49/49 | completed | 30,453 | 6,417 | 218,112 | 0 | $0.0084 | 61 |
| GPT-6 Luna | Aether | `eventlog` | 2 | 48/49 | completed | 37,635 | 4,448 | 189,696 | 0 | $0.0079 | 52 |
| GPT-6 Luna | Native CLI | `eventlog` | 3 | 46/49 | completed | 21,915 | 7,264 | 255,744 | 0 | $0.0084 | 66 |
| GPT-6 Luna | Aether | `eventlog` | 3 | 48/49 | completed | 10,842 | 3,888 | 158,464 | 0 | $0.0046 | 92 |
| GPT-6 Luna | Native CLI | `leases` | 1 | 28/28 | completed | 37,256 | 5,559 | 264,704 | 0 | $0.0092 | 60 |
| GPT-6 Luna | Aether | `leases` | 1 | 27/28 | completed | 28,546 | 4,109 | 131,072 | 0 | $0.0062 | 42 |
| GPT-6 Luna | Native CLI | `leases` | 2 | 27/28 | completed | 24,166 | 7,041 | 286,208 | 0 | $0.0088 | 164 |
| GPT-6 Luna | Aether | `leases` | 2 | 3/28 | completed | 19,019 | 4,354 | 205,056 | 0 | $0.0061 | 49 |
| GPT-6 Luna | Native CLI | `leases` | 3 | 28/28 | completed | 17,655 | 4,691 | 181,760 | 0 | $0.0059 | 44 |
| GPT-6 Luna | Aether | `leases` | 3 | 27/28 | completed | 13,394 | 4,914 | 187,904 | 0 | $0.0057 | 50 |
| GPT-6 Luna | Native CLI | `minilang` | 1 | 102/102 | completed | 73,889 | 9,148 | 820,224 | 0 | $0.0202 | 117 |
| GPT-6 Luna | Aether | `minilang` | 1 | 102/102 | completed | 34,455 | 7,587 | 389,120 | 0 | $0.0111 | 176 |
| GPT-6 Luna | Native CLI | `minilang` | 2 | 98/102 | completed | 43,791 | 10,783 | 699,136 | 0 | $0.0168 | 114 |
| GPT-6 Luna | Aether | `minilang` | 2 | 102/102 | completed | 36,569 | 7,753 | 512,000 | 0 | $0.0127 | 191 |
| GPT-6 Luna | Native CLI | `minilang` | 3 | 102/102 | completed | 47,713 | 10,646 | 606,464 | 0 | $0.0162 | 232 |
| GPT-6 Luna | Aether | `minilang` | 3 | 100/102 | completed | 70,705 | 8,474 | 450,048 | 0 | $0.0158 | 99 |
| GPT-6 Luna | Native CLI | `semver` | 1 | 79/80 | completed | 32,870 | 9,032 | 430,336 | 0 | $0.0121 | 195 |
| GPT-6 Luna | Aether | `semver` | 1 | 76/80 | completed | 45,233 | 6,571 | 297,216 | 0 | $0.0108 | 66 |
| GPT-6 Luna | Native CLI | `semver` | 2 | 78/80 | completed | 29,592 | 6,448 | 264,960 | 0 | $0.0088 | 141 |
| GPT-6 Luna | Aether | `semver` | 2 | 77/80 | completed | 16,154 | 6,850 | 164,864 | 0 | $0.0067 | 62 |
| GPT-6 Luna | Native CLI | `semver` | 3 | 79/80 | completed | 29,420 | 8,946 | 357,632 | 0 | $0.0110 | 84 |
| GPT-6 Luna | Aether | `semver` | 3 | 79/80 | completed | 32,746 | 8,423 | 295,424 | 0 | $0.0104 | 77 |
