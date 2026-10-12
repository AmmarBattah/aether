"""Parallel trial blocks: matched pairs share one block, so they share its load."""
import os
from concurrent.futures import ThreadPoolExecutor, as_completed

from harness_bench_process import terminate_owned_process_groups

MAX_JOBS = 8
PAIRED_PREFIXES = ("native-", "aether-single-")
BLOCK_SCHEME = "paired units packed into blocks of at most --jobs trials; a block ends before the next starts"


def pair_label(condition_key):
    """`native-X` and `aether-single-X` are one matched pair; any other cell pairs with nothing."""
    for prefix in PAIRED_PREFIXES:
        if condition_key.startswith(prefix):
            return condition_key[len(prefix):]
    return condition_key


def paired_units(order, seed=0):
    """Group each case's matched pair per run; which member launches first alternates per pair."""
    units, appearances, ordered = {}, {}, []
    for trial in order:
        case_id, condition_key, run_index = trial
        units.setdefault((run_index, case_id, pair_label(condition_key)), []).append(trial)
    for (_, _, label), members in units.items():
        assert len(members) <= 2, f"more than two cells share the pair label {label!r}: {members}"
        count = appearances.get(label, 0)
        appearances[label] = count + 1
        members = sorted(members, key=lambda trial: not trial[1].startswith("native-"))
        if len(members) == 2 and (count + seed) % 2 == 1:
            members.reverse()
        ordered.append(members)
    assert sum(len(unit) for unit in ordered) == len(order), "every trial belongs to exactly one unit"
    return ordered


def paired_blocks(order, jobs, seed=0):
    """Split a trial order into blocks run at once. One job keeps the sequential order."""
    assert 1 <= jobs <= MAX_JOBS, f"--jobs must be between 1 and {MAX_JOBS}"
    assert len(set(order)) == len(order), "a trial may be scheduled only once"
    if jobs == 1:
        return [[trial] for trial in order]
    blocks, current = [], []
    for unit in paired_units(order, seed):
        if len(current) + len(unit) > jobs:
            blocks.append(current)
            current = []
        current.extend(unit)
    if current:
        blocks.append(current)
    assert sorted(trial for block in blocks for trial in block) == sorted(order), "blocks must cover the order"
    assert all(0 < len(block) <= jobs for block in blocks), "a block never exceeds --jobs"
    return blocks


def host_load_average():
    """1/5/15-minute load, or None where the host cannot report it: unknown, never zero."""
    try:
        load = os.getloadavg()
    except OSError:
        return None
    assert len(load) == 3 and all(value >= 0 for value in load), f"impossible load average: {load}"
    return [round(value, 2) for value in load]


def run_block(block, run_one):
    """Start every trial of a block together and yield each row as it finishes.

    `run_one(trial, position)` owns its trial's processes; an interrupt stops them all."""
    assert block, "a block needs at least one trial"
    with ThreadPoolExecutor(max_workers=len(block)) as pool:
        futures = [pool.submit(run_one, trial, position) for position, trial in enumerate(block, 1)]
        assert len(futures) == len(block), "every trial of the block must be launched"
        try:
            for future in as_completed(futures):
                yield future.result()
        except BaseException:
            # Unblock the worker threads so the pool can drain before the error propagates.
            terminate_owned_process_groups()
            raise
