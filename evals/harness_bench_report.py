"""Render benchmark evidence without turning missing measurements into zero."""

import argparse
import csv
import html
import json
from pathlib import Path

from harness_bench_treatments import is_diagnostic, is_native_trial


def accounted_usage(row):
    """The usage a trial is reported under, with its coverage and scope.

    `lower_bound` marks a scope known to omit work whatever its coverage says."""
    report = row.get("usage_report") or {}
    usage = report.get("usage") or {}
    coverage = report.get("coverage") or {}
    scope = "native receipt" if is_native_trial(row) else "Aether durable (may omit helpers)"
    reconciliation = row.get("claude_reconciliation") or {}
    treatment = row.get("requested_treatment") or {}
    single_claude = (row["condition"] == "aether-single-claude"
                     or (treatment.get("kind") == "aether-single"
                         and treatment.get("model", "").startswith("claude-code/")))
    if (single_claude
            and reconciliation.get("full_process_usage", {}).get("total_tokens") is not None):
        return {"usage": reconciliation["full_process_usage"], "coverage": reconciliation["coverage"],
                "scope": "Claude all-model process receipts", "lower_bound": False}
    if single_claude:
        scope = "Aether durable lower bound; native final receipt unavailable"
    return {"usage": usage, "coverage": coverage, "scope": scope, "lower_bound": single_claude}


def trial_summary(row):
    """Cost-first: uncached input, output and API-equivalent cost are the
    headline consumption figures because cache reads are far cheaper than
    fresh input and counting them at face value (the old "observed tokens"
    total) can overstate a condition's cost by an order of magnitude for a
    warm-cache run. Cache reads still get their own column below."""
    accounted = accounted_usage(row)
    usage, coverage, scope = accounted["usage"], accounted["coverage"], accounted["scope"]
    cost_coverage = coverage.get("api_equivalent_cost_usd", {})
    billed, estimated = cost_coverage.get("billed", 0), cost_coverage.get("estimated", 0)
    basis = "mixed" if billed and estimated else "estimated" if estimated else "billed" if billed else "unknown"
    grade = row.get("grade") or {}
    return {
        "batch": row.get("batch", "pilot"), "condition": row["condition"], "case": row["case"], "run": row["run"],
        "outcome": row.get("status", "unknown"), "success": row.get("success", False),
        "workflow": row.get("workflow_status", row.get("status")),
        "checks_passed": grade.get("passed"), "checks_total": grade.get("total"),
        "checks_by_test": row.get("grade_by_test"),
        "checks_by_test_error": row.get("grade_by_test_error"),
        "uncached_input_tokens": usage.get("uncached_input_tokens"), "output_tokens": usage.get("output_tokens"),
        "api_equivalent_usd": usage.get("api_equivalent_cost_usd"),
        "cost_basis": basis,
        "cost_billed_units": cost_coverage.get("billed"),
        "cost_estimated_units": cost_coverage.get("estimated"),
        "cost_unpriced_units": cost_coverage.get("unpriced"),
        "cost_priced": (usage.get("api_equivalent_cost_usd") is not None
                        and cost_coverage.get("complete", True)),
        "cost_coverage_complete": cost_coverage.get("complete", False),
        "solve_seconds": row.get("solve_seconds"),
        "cached_input_tokens": usage.get("cache_read_tokens"), "cache_write_tokens": usage.get("cache_write_tokens"),
        "setup_seconds": row.get("daemon_setup_seconds"),
        "tokens": usage.get("total_tokens"), "input_tokens": usage.get("input_tokens"),
        "token_receipt_coverage_complete": coverage.get("total_tokens", {}).get("complete", False),
        "descendants": row.get("descendant_count", 0) if is_native_trial(row) else row.get("descendant_count"),
        "accounting_scope": scope, "artifact": str(Path(row["trial_dir"]) / "result.json"),
        "error": row.get("harness_error") or row.get("workflow_error") or row.get("usage_error") or row.get("grade_error"),
    }


def _mean(values):
    return sum(values) / len(values) if values else None


def accuracy(row):
    """Fraction of hidden checks passed, or None when the trial was not graded."""
    if row["checks_passed"] is None or not row["checks_total"]:
        return None
    return row["checks_passed"] / row["checks_total"]


def aggregate(rows, include_blocked=False, by_case=False):
    """A `blocked` trial (host/environment refusal, not a model mistake) is
    excluded from every mean and from the pass-rate denominator by default;
    pass `include_blocked=True` to fold it back in for an audit view.
    `by_case=True` splits each condition into one group per case."""
    def key(row):
        return (row["batch"], row["condition"], row["case"] if by_case else None)

    groups = []
    for batch, condition, case in sorted(set(map(key, rows)), key=lambda item: (item[0], item[1], item[2] or "")):
        all_selected = [row for row in rows if key(row) == (batch, condition, case)]
        blocked = [row for row in all_selected if row["outcome"] == "blocked"]
        selected = all_selected if include_blocked else [row for row in all_selected if row["outcome"] != "blocked"]
        measured_tokens = [row["tokens"] for row in selected if row["tokens"] is not None]
        measured_times = [row["solve_seconds"] for row in selected if row["solve_seconds"] is not None]
        measured_uncached = [row["uncached_input_tokens"] for row in selected if row["uncached_input_tokens"] is not None]
        measured_output = [row["output_tokens"] for row in selected if row["output_tokens"] is not None]
        measured_cost = [row["api_equivalent_usd"] for row in selected if row["cost_priced"]]
        measured_cache_reads = [row["cached_input_tokens"] for row in selected if row["cached_input_tokens"] is not None]
        measured_accuracy = [value for value in map(accuracy, selected) if value is not None]
        all_tokens = len(measured_tokens) == len(selected)
        successes = sum(row["success"] for row in selected)
        groups.append({
            "batch": batch, "condition": condition, "trials": len(selected), "successes": successes,
            "blocked_trials": len(blocked),
            "mean_accuracy": _mean(measured_accuracy), "graded_trials": len(measured_accuracy),
            "full_passes": sum(value == 1 for value in measured_accuracy),
            "mean_uncached_input_tokens": _mean(measured_uncached), "mean_output_tokens": _mean(measured_output),
            "mean_api_equivalent_usd": _mean(measured_cost), "api_equivalent_usd_trials": len(measured_cost),
            "mean_seconds": _mean(measured_times), "mean_cache_read_tokens": _mean(measured_cache_reads),
            "mean_observed_tokens": _mean(measured_tokens),
            "trials_with_token_measurements": len(measured_tokens),
            "observed_tokens_per_success": sum(measured_tokens) / successes if all_tokens and successes else None,
            "all_token_receipts_complete": all(row["token_receipt_coverage_complete"] for row in selected),
            **({"case": case} if by_case else {}),
        })
    return groups


def display(value):
    if value is None:
        return "unknown"
    if isinstance(value, float):
        return f"{value:,.2f}"
    if isinstance(value, int):
        return f"{value:,}"
    return str(value)


def _markdown_table(headers, body):
    lines = ["| " + " | ".join(headers) + " |", "|" + " --- |" * len(headers)]
    lines += ["| " + " | ".join(cell.replace("|", "\\|") for cell in row) + " |" for row in body]
    return "\n".join(lines)


def _percent(value):
    return "unknown" if value is None else f"{value:.1%}"


def _usd(value):
    return "unknown" if value is None else f"${value:,.4f}"


def render_markdown(groups, case_groups):
    """Headline tables for README/docs; unknown measurements stay `unknown`."""
    def numbers(group):
        return [_percent(group["mean_accuracy"]), f'{group["full_passes"]}/{group["graded_trials"]}',
                display(group["mean_uncached_input_tokens"]), display(group["mean_output_tokens"]),
                display(group["mean_cache_read_tokens"]), display(group["mean_observed_tokens"]),
                _usd(group["mean_api_equivalent_usd"]) + ("" if group["api_equivalent_usd_trials"] == group["trials"]
                                                         else f' ({group["api_equivalent_usd_trials"]}/{group["trials"]} priced)'),
                display(group["mean_seconds"])]

    headers = ["Mean accuracy", "Full passes", "Mean uncached input", "Mean output", "Mean cache reads",
               "Mean total tokens", "Mean API-equivalent USD", "Mean wall seconds"]
    overall = _markdown_table(["Batch", "Condition", "Trials"] + headers,
                              [[g["batch"], g["condition"], str(g["trials"])] + numbers(g) for g in groups])
    per_case = _markdown_table(["Case", "Condition", "Trials"] + headers,
                               [[g["case"], g["condition"], str(g["trials"])] + numbers(g)
                                for g in sorted(case_groups, key=lambda g: (g["case"], g["condition"]))])
    return f"## By condition\n\n{overall}\n\n## By case\n\n{per_case}\n"


def render_html(rows, groups, findings=""):
    def table(headers, body):
        head = "".join(f"<th>{html.escape(label)}</th>" for label in headers)
        cells = "".join("<tr>" + "".join(f"<td>{html.escape(display(cell))}</td>" for cell in row) + "</tr>" for row in body)
        return f'<div class="scroll"><table><thead><tr>{head}</tr></thead><tbody>{cells}</tbody></table></div>'

    grouped = table(["Batch / condition", "Finished correctly", "Mean accuracy", "Blocked (excluded)", "Mean uncached input",
                     "Mean output", "Mean API-equivalent USD", "Priced / total trials", "Mean wall seconds", "Mean cache reads"],
                    [[g["batch"] + " / " + g["condition"], f'{g["successes"]}/{g["trials"]}', _percent(g["mean_accuracy"]),
                      g["blocked_trials"],
                      g["mean_uncached_input_tokens"], g["mean_output_tokens"], g["mean_api_equivalent_usd"],
                      f'{g["api_equivalent_usd_trials"]}/{g["trials"]}', g["mean_seconds"], g["mean_cache_read_tokens"]] for g in groups])
    detailed = table(["Condition", "Case", "Run", "Outcome", "Checks", "Uncached input", "Output",
                      "API-equivalent USD", "Cost basis", "Cost units billed / estimated / unpriced", "Wall seconds", "Cache reads", "Cache writes", "Receipt coverage",
                      "Descendants", "Accounting"],
                     [[r["batch"] + " / " + r["condition"], r["case"], r["run"], r["outcome"],
                       f'{r["checks_passed"]}/{r["checks_total"]}', r["uncached_input_tokens"], r["output_tokens"],
                       display(r["api_equivalent_usd"]) + (" (partial)" if r["api_equivalent_usd"] is not None and not r["cost_coverage_complete"] else ""),
                       r["cost_basis"],
                       " / ".join(display(r[k]) for k in ("cost_billed_units", "cost_estimated_units", "cost_unpriced_units")),
                       r["solve_seconds"], r["cached_input_tokens"], r["cache_write_tokens"],
                       "complete" if r["token_receipt_coverage_complete"] else "partial/unknown",
                       r["descendants"], r["accounting_scope"]] for r in rows])
    failures = "".join(f'<li><strong>{html.escape(r["condition"] + "/" + r["case"])}</strong>: {html.escape(r["error"])}</li>'
                       for r in rows if r["error"])
    audit = (f'<section><h2>Audit and improvement plan</h2><details><summary>Read the detailed findings</summary>'
             f'<div style="white-space:pre-wrap;overflow-wrap:anywhere">{html.escape(findings)}</div></details></section>') if findings else ""
    return f'''<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Aether harness benchmark pilot</title><style>
:root{{color-scheme:dark;font:16px/1.55 system-ui,sans-serif;background:#11151b;color:#e1e7ef}}
body{{max-width:1400px;margin:auto;padding:clamp(16px,4vw,48px)}}
h1,h2{{line-height:1.2}}h1{{color:#6cb6ff}}section{{margin:32px 0}}
.notice{{border-left:4px solid #ffaf00;padding:12px 20px;background:#20252d}}
.scroll{{overflow-x:auto;max-width:100%}}table{{border-collapse:collapse;width:100%;font-size:.88rem}}
th,td{{text-align:left;vertical-align:top;padding:12px;border-bottom:1px solid #39414d;overflow-wrap:anywhere}}
th{{color:#6cb6ff}}a{{color:#6cb6ff}}li{{margin:8px 0}}code{{overflow-wrap:anywhere}}
</style><h1>Aether versus native harnesses</h1><p>Fresh difficult-task pilot · {len(rows)} recorded trials</p>
<div class="notice"><strong>Diagnostic evidence, not a universal ranking.</strong>
Different model defaults and the mixed-model team are not pure harness-overhead controls.
Missing counters remain unknown; durable Aether counts may omit native helper models.
Dollar values are API-equivalent metadata or list-price estimates, not subscription charges.
Cost means include only fully priced terminal trials; billed / estimated / unpriced counts describe recorded units. Consumption leads with
uncached input, output and API-equivalent cost; cache reads are shown separately because
counting them at face value can overstate cost by an order of magnitude for a warm cache.
Blocked trials (a host/environment refusal, not a model mistake) are excluded from every
mean and from the pass-rate denominator above; see the "Blocked (excluded)" column.</div>
<section><h2>Outcome and consumption</h2>{grouped}</section>
<section><h2>Every trial, including failures and blocked runs</h2>{detailed}</section>
<section><h2>Interpretation limits</h2><ul>
<li>Code correctness alone does not count as success: the workflow must finish within its deadline.</li>
<li>One observation per task is not a reliable accuracy estimate. Run-order, cache warmth and provider load can affect time and tokens.</li>
<li>The default coordinator may implement directly. The explicit team requires two children; their work must be counted.</li>
<li>Receipt completeness describes recorded units, not proof of complete upstream billing telemetry.</li>
<li>Do not add native receipts to the durable ledger: they are overlapping views of the same work.</li>
<li>A blocked trial reflects a host/environment limitation (e.g. a refused delegate), not a graded model answer.</li>
</ul></section><section><h2>Recorded errors</h2><ul>{failures or '<li>No explicit runtime/accounting errors recorded; inspect per-case grades for correctness failures.</li>'}</ul></section>
{audit}<p><a href="results.csv">Numeric CSV</a> · <a href="summary.json">Summary JSON</a></p>
<p>Design uses Aether's Dusk palette: blue accent, amber cautions, dark neutral background.</p></html>'''


def is_diagnostic_manifest(manifest):
    """True for a --diagnostic run, and for any manifest holding a diag- trial whatever its flag says."""
    trials = manifest.get("trials")
    assert isinstance(trials, list), "a manifest carries a trials list"
    assert all(isinstance(raw.get("condition"), str) for raw in trials), "every trial names its condition"
    return bool(manifest.get("diagnostic")) or any(is_diagnostic(raw["condition"]) for raw in trials)


def protocol_version(manifest):
    """The protocol a manifest was measured under; one written before versions were recorded is version 1."""
    version = manifest.get("protocol_version", 1)
    assert isinstance(version, int) and not isinstance(version, bool), "a protocol version is a whole number"
    assert version >= 1, "protocol versions start at 1"
    return version


def refuse_mixed_protocols(manifests_by_path):
    """Raise when the manifests were measured under more than one protocol version."""
    assert manifests_by_path, "at least one manifest is being read"
    versions = {str(path): protocol_version(manifest) for path, manifest in manifests_by_path}
    if len(set(versions.values())) > 1:
        listed = "; ".join(f"{path}: version {version}" for path, version in sorted(versions.items()))
        raise ValueError(f"manifests of different protocol versions are never merged ({listed})")
    return next(iter(versions.values()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifests", nargs="+", type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--findings", type=Path, help="Optional written audit to embed, escaped as text")
    parser.add_argument("--include-blocked", action="store_true",
                        help="Fold blocked (host/environment refusal) trials back into pass-rate and cost means")
    args = parser.parse_args()
    rows, seen = [], set()
    manifests = [(path, json.loads(path.read_text())) for path in args.manifests]
    refuse_mixed_protocols(manifests)
    for path, manifest in manifests:
        if manifest.get("pilot"):
            raise ValueError(f"{path}: pilot manifests are calibration only and are never counted")
        if is_diagnostic_manifest(manifest):
            raise ValueError(f"{path}: diagnostic manifests locate a cause and are never counted")
        for raw in manifest["trials"]:
            identity = raw["short_id"]
            if identity in seen:
                raise ValueError(f"duplicate trial {identity}: overlapping manifests")
            seen.add(identity)
            rows.append(trial_summary({**raw, "batch": path.parent.name}))
    if not rows:
        raise ValueError("no trial results to report")
    groups = aggregate(rows, include_blocked=args.include_blocked)
    case_groups = aggregate(rows, include_blocked=args.include_blocked, by_case=True)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    (args.output_dir / "summary.json").write_text(json.dumps({"sources": [str(p) for p in args.manifests], "groups": groups,
                                                              "case_groups": case_groups, "trials": rows}, indent=2) + "\n")
    (args.output_dir / "summary.md").write_text(render_markdown(groups, case_groups))
    with (args.output_dir / "results.csv").open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    findings = args.findings.read_text() if args.findings else ""
    if findings:
        (args.output_dir / "findings.md").write_text(findings)
    (args.output_dir / "index.html").write_text(render_html(rows, groups, findings))
    print(args.output_dir / "index.html")


if __name__ == "__main__":
    main()
