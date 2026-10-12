#!/usr/bin/env python3
"""Four-measure Markdown comparison of matched native and Aether cells.

For every model pair (`native-X` beside `aether-single-X`) found in one or
more manifests, prints accuracy, tokens, API-equivalent cost and wall time side
by side, plus the paired difference (Aether minus native). Unknown stays
"unknown"; a token or cost figure from incomplete receipts is a lower bound and
enters no mean; pilot manifests are refused; no significance is claimed.

Usage:
  python3 evals/harness_bench_compare.py MANIFEST [MANIFEST ...] [--trials] [--output FILE]
"""
import argparse
import json
import sys
from pathlib import Path

from harness_bench_blocks import pair_label
from harness_bench_report import accounted_usage, is_diagnostic_manifest, refuse_mixed_protocols, trial_summary

TOKEN_MEASURES = (("uncached_input_tokens", "Uncached input tokens"), ("output_tokens", "Output tokens"),
                  ("cached_input_tokens", "Cache read tokens"), ("cache_write_tokens", "Cache write tokens"))
# Row key -> (trial_summary key holding the reported value, usage field whose coverage guards it).
RECEIPT_MEASURES = {"uncached_input_tokens": ("uncached_input_tokens", "uncached_input_tokens"),
                    "output_tokens": ("output_tokens", "output_tokens"),
                    "cached_input_tokens": ("cached_input_tokens", "cache_read_tokens"),
                    "cache_write_tokens": ("cache_write_tokens", "cache_write_tokens"),
                    "usd": ("api_equivalent_usd", "api_equivalent_cost_usd")}
SIDES = (("native", "native-"), ("aether", "aether-single-"))


def _mean(values):
    return sum(values) / len(values) if values else None


def _side(condition):
    for side, prefix in SIDES:
        if condition.startswith(prefix):
            return side
    return None


def _effort_confirmed(raw, requested):
    """True only when the trial's own session metadata reports the requested effort."""
    reported = {item.get("effort") for item in raw.get("resolved_metadata") or []} - {None}
    return bool(requested) and reported == {requested}


def receipt_measures(raw, summary):
    """Split a trial's token and cost figures into fully measured values and lower bounds.

    A figure is measured only when every unit of its accounting scope reported it
    at a terminal point; otherwise the sum is a floor and must enter no mean."""
    accounted = accounted_usage(raw)
    coverage = accounted["coverage"]
    assert isinstance(coverage, dict), "usage coverage must be a mapping of field to counts"
    measured, lower_bounds = {}, {}
    for key, (summary_key, field) in RECEIPT_MEASURES.items():
        value = summary[summary_key]
        assert value is None or (isinstance(value, (int, float)) and value >= 0), f"impossible {field}: {value!r}"
        complete = not accounted["lower_bound"] and (coverage.get(field) or {}).get("complete") is True
        measured[key] = value if complete else None
        if value is not None and not complete:
            lower_bounds[key] = value
    return measured, lower_bounds


def comparison_row(raw, source, jobs, manifest):
    """One trial reduced to the compared measures; a missing or partial measure stays None."""
    assert type(manifest) is int and manifest >= 1, f"manifest position must count from 1: {manifest!r}"
    assert isinstance(raw.get("condition"), str) and isinstance(raw.get("case"), str), "trial needs condition and case"
    summary = trial_summary(raw)
    measured, lower_bounds = receipt_measures(raw, summary)
    treatment = raw.get("requested_treatment") or {}
    passed, total = summary["checks_passed"], summary["checks_total"]
    graded = type(passed) is int and type(total) is int and total > 0
    assert not graded or 0 <= passed <= total, f"impossible grade {passed}/{total}"
    models = sorted({item["model"] for item in raw.get("resolved_metadata") or [] if item.get("model")})
    return {
        **summary, **measured, "lower_bounds": lower_bounds,
        "source": source, "manifest": manifest, "jobs": jobs, "pair": pair_label(raw["condition"]),
        "side": _side(raw["condition"]), "check_fraction": passed / total if graded else None,
        "requested_model": treatment.get("model"), "requested_effort": treatment.get("effort"),
        "effort_confirmed": _effort_confirmed(raw, treatment.get("effort")), "resolved_models": models,
    }


def load_rows(paths):
    """Read counted manifests. Pilot, overlapping and mixed-protocol manifests are errors, not rows."""
    rows, seen = [], set()
    manifests = [(path, json.loads(Path(path).read_text())) for path in paths]
    if manifests:
        refuse_mixed_protocols(manifests)
    for position, (path, manifest) in enumerate(manifests, start=1):
        if not isinstance(manifest.get("trials"), list):
            raise ValueError(f"{path}: not a harness-bench manifest (no trials list)")
        if manifest.get("pilot"):
            raise ValueError(f"{path}: pilot manifests are calibration only and are never counted")
        if is_diagnostic_manifest(manifest):
            raise ValueError(f"{path}: diagnostic manifests locate a cause and are never counted")
        for raw in manifest["trials"]:
            if raw["short_id"] in seen:
                raise ValueError(f"duplicate trial {raw['short_id']}: overlapping manifests")
            seen.add(raw["short_id"])
            rows.append(comparison_row(raw, str(path), manifest.get("jobs"), position))
    if not rows:
        raise ValueError("no trial results to compare")
    return rows


def side_measures(rows):
    """Aggregate one cell. Every mean carries how many trials it was measured on."""
    counted = [row for row in rows if row["outcome"] != "blocked"]
    measures = {"trials": len(counted), "blocked": len(rows) - len(counted),
                "finished": sum(row["workflow"] == "completed" for row in counted),
                "partial": sum(bool(row["lower_bounds"]) for row in counted)}
    for key in ("check_fraction", "usd", "solve_seconds", *(name for name, _ in TOKEN_MEASURES)):
        known = [row[key] for row in counted if row[key] is not None]
        measures[key] = {"mean": _mean(known), "known": len(known), "values": known}
    measures["cost_basis"] = sorted({row["cost_basis"] for row in counted if row["usd"] is not None})
    measures["cost_partial"] = sum("usd" in row["lower_bounds"] for row in counted)
    measures["per_run"] = [(row["case"], row["manifest"], row["run"], row["checks_passed"], row["checks_total"])
                           for row in counted]
    assert all(m["known"] <= len(counted) for m in measures.values() if isinstance(m, dict)), "more values than trials"
    assert measures["finished"] <= measures["trials"], "more finished workflows than trials"
    return measures


def paired_differences(native_rows, aether_rows):
    """Aether minus native per (manifest, case, run) pair where both sides measured it."""
    def by_pair(rows):
        keyed = {}
        for row in rows:
            if row["outcome"] == "blocked":
                continue
            key = (row["source"], row["case"], row["run"])
            assert key not in keyed, f"two trials of one cell share {key}"
            keyed[key] = row
        return keyed
    native, aether = by_pair(native_rows), by_pair(aether_rows)
    differences = {}
    for key in ("check_fraction", "usd", "solve_seconds", *(name for name, _ in TOKEN_MEASURES)):
        values = [aether[pair][key] - native[pair][key] for pair in sorted(native.keys() & aether.keys())
                  if native[pair][key] is not None and aether[pair][key] is not None]
        differences[key] = {"mean": _mean(values), "pairs": len(values)}
    assert all(d["pairs"] <= min(len(native), len(aether)) for d in differences.values()), "more pairs than trials"
    return differences


def _number(value, digits=0, signed=False):
    if value is None:
        return "unknown"
    return f"{value:{'+' if signed else ''},.{digits}f}"


def _cell(measure, trials, digits=0, scale=1, suffix=""):
    if measure["mean"] is None:
        return f"unknown (0/{trials})"
    return f"{_number(measure['mean'] * scale, digits)}{suffix} ({measure['known']}/{trials})"


def _difference(difference, digits=0, scale=1, suffix=""):
    if difference["mean"] is None:
        return "unknown (0 pairs)"
    return f"{_number(difference['mean'] * scale, digits, signed=True)}{suffix} ({difference['pairs']} pairs)"


def _per_run(measures):
    """Each case's counts in manifest order, then run, so both sides of a pair line up by position."""
    by_case = {}
    for case, _manifest, _run, passed, total in sorted(measures["per_run"], key=lambda item: item[:3]):
        by_case.setdefault(case, []).append("unknown" if passed is None or total is None else f"{passed}/{total}")
    return "; ".join(f"{case}: {', '.join(values)}" for case, values in by_case.items()) or "none"


def _wall(measure, trials):
    if measure["mean"] is None:
        return f"unknown (0/{trials})"
    low, high = min(measure["values"]), max(measure["values"])
    return f"{_number(measure['mean'], 1)} ({_number(low, 1)} to {_number(high, 1)}; {measure['known']}/{trials})"


def _cost(measures):
    usd = measures["usd"]
    if usd["mean"] is None and measures["cost_partial"]:
        return f"unknown, partial receipts only (0/{measures['trials']} priced)"
    if usd["mean"] is None:
        return f"unpriced (0/{measures['trials']} priced)"
    basis = " + ".join(measures["cost_basis"])
    return f"{_number(usd['mean'], 4)} ({basis}; {usd['known']}/{measures['trials']} priced)"


def _identity(rows):
    requested = sorted({str(row["requested_model"]) for row in rows})
    resolved = sorted({model for row in rows for model in row["resolved_models"]}) or ["unknown"]
    efforts = sorted({str(row["requested_effort"]) for row in rows})
    basis = "confirmed by session metadata" if all(row["effort_confirmed"] for row in rows) else "requested"
    return f"`{', '.join(requested)}` (resolved: {', '.join(resolved)}; effort {', '.join(efforts)}, {basis})"


def _accounting(rows):
    return " / ".join(sorted({row["accounting_scope"] for row in rows}))


def _lower_bound_note(sides):
    """Name every figure withheld from the means, so a floor is never read as a measurement."""
    titles = {**dict(TOKEN_MEASURES), "usd": "API-equivalent USD"}
    notes = []
    for side, rows in sides:
        for row in sorted(rows, key=lambda row: (row["case"], row["run"])):
            if row["outcome"] == "blocked" or not row["lower_bounds"]:
                continue
            assert set(row["lower_bounds"]) <= set(titles), f"unnamed lower bound in {row['lower_bounds']}"
            floors = ", ".join(f"{titles[key]} at least {_number(value, 4 if key == 'usd' else 0)}"
                               for key, value in row["lower_bounds"].items())
            notes.append(f"{side} {row['case']} run {row['run']} ({floors})")
    if not notes:
        return []
    return ["Partial receipts are lower bounds and enter no mean or paired difference above: "
            + "; ".join(notes) + ".", ""]


def _ceiling_note(native_rows, aether_rows):
    """Name the cases where every run of both sides passed every check: accuracy cannot rise there."""
    def saturated(rows):
        perfect = {}
        for row in rows:
            if row["outcome"] != "blocked":
                perfect.setdefault(row["case"], []).append(row["check_fraction"] == 1)
        return {case for case, runs in perfect.items() if all(runs)}
    cases = sorted(saturated(native_rows) & saturated(aether_rows))
    if not cases:
        return []
    note = (f"Accuracy is at the ceiling on {', '.join(cases)}: every run of both sides passed every hidden "
            "check, so this pair can show a loss there but not a gain.")
    return [note, ""]


def render_pair(label, native_rows, aether_rows):
    assert native_rows and aether_rows, f"pair {label!r} needs both a native and an Aether cell"
    native, aether = side_measures(native_rows), side_measures(aether_rows)
    delta = paired_differences(native_rows, aether_rows)
    jobs = sorted({str(row["jobs"] or 1) for row in native_rows + aether_rows})
    lines = [f"### {label}", "", f"- Native: {_identity(native_rows)}", f"- Aether: {_identity(aether_rows)}",
             f"- Native token and cost accounting: {_accounting(native_rows)}",
             f"- Aether token and cost accounting: {_accounting(aether_rows)}",
             f"- Trials at once (`--jobs`): {', '.join(jobs)}. With more than one, wall time is a paired "
             "comparison under shared load, not an absolute figure.", "",
             "| Measure | Native | Aether | Aether minus native |", "| --- | --- | --- | --- |",
             f"| Trials | {native['trials']} | {aether['trials']} | |",
             f"| Workflows finished | {native['finished']}/{native['trials']} "
             f"| {aether['finished']}/{aether['trials']} | |",
             f"| Hidden checks passed, mean | {_cell(native['check_fraction'], native['trials'], 1, 100, ' %')} "
             f"| {_cell(aether['check_fraction'], aether['trials'], 1, 100, ' %')} "
             f"| {_difference(delta['check_fraction'], 1, 100, ' pp')} |",
             f"| Hidden checks per run | {_per_run(native)} | {_per_run(aether)} | |"]
    for key, title in TOKEN_MEASURES:
        lines.append(f"| {title}, mean | {_cell(native[key], native['trials'])} "
                     f"| {_cell(aether[key], aether['trials'])} | {_difference(delta[key])} |")
    lines += [f"| API-equivalent USD, mean | {_cost(native)} | {_cost(aether)} | {_difference(delta['usd'], 4)} |",
              f"| Wall seconds, mean (min to max) | {_wall(native['solve_seconds'], native['trials'])} "
              f"| {_wall(aether['solve_seconds'], aether['trials'])} | {_difference(delta['solve_seconds'], 1)} |",
              f"| Partial-receipt trials (in no token or cost mean) | {native['partial']} | {aether['partial']} | |",
              f"| Blocked trials (excluded above) | {native['blocked']} | {aether['blocked']} | |", "",
              f"{native['trials']} native and {aether['trials']} Aether trials. "
              "Counts in brackets are measured/total trials. No significance is claimed.", ""]
    return "\n".join(lines + _ceiling_note(native_rows, aether_rows)
                     + _lower_bound_note((("Native", native_rows), ("Aether", aether_rows))))


def render(rows):
    """Markdown for every matched pair; unmatched cells are named, never dropped silently."""
    assert rows, "nothing to render"
    pairs, unmatched = {}, set()
    for row in rows:
        if row["side"] is None:
            unmatched.add(row["condition"])
            continue
        pairs.setdefault(row["pair"], {"native": [], "aether": []})[row["side"]].append(row)
    sections = ["## Native and Aether, matched model and effort", ""]
    for label in sorted(pairs):
        if pairs[label]["native"] and pairs[label]["aether"]:
            sections.append(render_pair(label, pairs[label]["native"], pairs[label]["aether"]))
        else:
            unmatched.update(row["condition"] for side in pairs[label].values() for row in side)
    if unmatched:
        sections.append(f"Cells without a matched partner, not tabulated: {', '.join(sorted(unmatched))}.\n")
    sections.append("Dollar values are API-equivalent (reported metadata or list-price estimates), "
                    "not subscription charges. A mean covers only the trials that measured it: a token or "
                    "cost figure counts as measured only when every unit of its accounting scope reported it "
                    "at a terminal point. A pair is one case and run within one manifest; trials from "
                    "different manifests are never paired, so run both sides of a pair in the same block.\n")
    return "\n".join(sections)


TRIAL_COLUMNS = ("Manifest", "Case", "Run", "Cell", "Workflow", "Hidden checks", "Uncached input", "Output",
                 "Cache read", "Cache write", "USD", "Wall seconds")
# Shown only when a listed trial recorded it; it is beside the counted grade and enters no table above.
BY_TEST_COLUMN = "Hidden checks test by test (not counted)"
BY_TEST_NOTE = ("The test-by-test column repeats the counted grade unless the grader's one process timed out; "
                "then every hidden test ran in a process of its own. It is not the counted grade. "
                "\"unknown\" there means that grade could not be made; the counted grade beside it stands.")
TRIAL_ORDER_NOTE = ("Manifests are listed in the order given, each one a separate runner invocation; "
                    "the Run column is the run within its manifest.")


def _trial_figure(row, key, digits=0):
    """A trial's own figure: measured, a named lower bound, or unknown; never a bare floor."""
    if row[key] is not None:
        return _number(row[key], digits)
    if key in row["lower_bounds"]:
        return f"at least {_number(row['lower_bounds'][key], digits)}"
    priceable = key == "usd" and row["uncached_input_tokens"] is not None and row["output_tokens"] is not None
    return "unpriced" if priceable else "unknown"


def _by_test_figure(by_test, error):
    """A trial's test-by-test grade as text: unknown when it failed, not recorded in an older manifest."""
    assert by_test is None or not error, f"a test-by-test grade and its error exclude each other: {error!r}"
    if by_test is None:
        return "unknown" if error else "not recorded"
    passed, total = by_test["passed"], by_test["total"]
    assert type(passed) is int and type(total) is int and 0 <= passed <= total, f"impossible grade {passed}/{total}"
    apart = [f"{by_test[key]} {word}" for key, word in (("hung", "hung"), ("not_run", "not run")) if by_test[key]]
    return f"{passed}/{total}" + (f" ({', '.join(apart)})" if apart else "")


def render_trials(rows):
    """One line per trial, for a report whose manifests are not kept beside it."""
    assert rows, "nothing to list"
    with_by_test = any(row.get("checks_by_test") is not None or row.get("checks_by_test_error") for row in rows)
    columns = list(TRIAL_COLUMNS)
    if with_by_test:
        columns.insert(columns.index("Hidden checks") + 1, BY_TEST_COLUMN)
    lines = ["## Every trial", "", TRIAL_ORDER_NOTE, "", "| " + " | ".join(columns) + " |",
             "|" + " --- |" * len(columns)]
    ordered = sorted(rows, key=lambda row: (row["manifest"], row["case"], row["run"], row["pair"],
                                            row["side"] != "native", row["condition"]))
    for row in ordered:
        graded = row["check_fraction"] is not None
        cells = ["/".join(Path(row["source"]).parts[-2:]), row["case"], str(row["run"]), row["condition"],
                 "blocked" if row["outcome"] == "blocked" else str(row["workflow"]),
                 f"{row['checks_passed']}/{row['checks_total']}" if graded else "unknown",
                 *([_by_test_figure(row.get("checks_by_test"), row.get("checks_by_test_error"))]
                   if with_by_test else []),
                 *(_trial_figure(row, key) for key, _ in TOKEN_MEASURES), _trial_figure(row, "usd", 4),
                 _number(row["solve_seconds"], 1)]
        assert len(cells) == len(columns), "a trial line must fill every column"
        lines.append("| " + " | ".join(cells) + " |")
    note = ("A figure reading \"at least\" comes from incomplete receipts and is a lower bound. "
            "The manifest column is the file and its directory, not a full path.")
    return "\n".join([*lines, "", note, *(["", BY_TEST_NOTE] if with_by_test else []), ""])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("manifests", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, help="write the Markdown here instead of stdout")
    parser.add_argument("--trials", action="store_true", help="append one line per trial under the tables")
    args = parser.parse_args(argv)
    rows = load_rows(args.manifests)
    markdown = render(rows) + ("\n" + render_trials(rows) if args.trials else "")
    if args.output:
        args.output.write_text(markdown)
    else:
        sys.stdout.write(markdown)
    return markdown


if __name__ == "__main__":
    main()
