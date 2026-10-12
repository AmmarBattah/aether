"""Rebuild usage from raw trial captures without modifying a source manifest."""

import argparse
import copy
import json
from pathlib import Path

from harness_bench_pricing import validate_price_table
from harness_bench_treatments import is_native_trial
from harness_bench_usage import parse_aether_events, parse_claude_jsonl, parse_codex_jsonl, reconcile_claude_receipts


def codex_trial_model(row, manifest):
    """Prefer the model this trial actually resolved over the operator's default.

    Several distinct resolved models cannot be allocated across turns, so the
    trial stays unpriced rather than guessing one.
    """
    resolved = {entry["model"] for entry in row.get("resolved_metadata") or []
                if isinstance(entry, dict) and entry.get("model")}
    if len(resolved) == 1:
        return resolved.pop(), "resolved model"
    if resolved:
        return None, "ambiguous resolved models"
    if row.get("requested_treatment", {}).get("model"):
        return None, "explicit model not verified by runtime metadata"
    configured = manifest.get("native", {}).get("codex", {}).get("configured_defaults", {}).get("model")
    return configured, "configured default model"


def _build_report(row, manifest, old_report):
    """Parse this trial's raw capture; raises OSError/ValueError when unusable."""
    trial_dir = Path(row["trial_dir"])
    logs = row.get("raw_logs", {})
    table = manifest["price_table"]
    condition = row["condition"]
    kind = row.get("requested_treatment", {}).get("kind", condition)
    if not is_native_trial(row):
        root = row.get("root_agent_id")
        if not root:
            raise ValueError("no root agent was created")
        events = json.loads(Path(logs.get("events", trial_dir / "events.json")).read_text())
        efforts = {u["agent_id"]: u.get("effort") for u in reversed(old_report.get("units", []))
                   if u.get("agent_id")}
        return parse_aether_events(events, root, initial_efforts=efforts, price_table=table)
    text = Path(logs.get("stdout", trial_dir / "native-stdout.jsonl")).read_text()
    if kind == "native-codex":
        model, basis = codex_trial_model(row, manifest)
        return parse_codex_jsonl(text, price_table=table, model=model, model_basis=basis)
    if kind == "native-claude":
        return parse_claude_jsonl(text)
    raise ValueError(f"unsupported condition: {condition}")


def rebuild_trial_usage(row, manifest):
    """Replace stale accounting; failed/missing captures stay explicitly unknown."""
    old_report = row.pop("usage_report", {})
    for stale in ("claude_reconciliation", "claude_reconciliation_error", "usage_error"):
        row.pop(stale, None)
    try:
        report = _build_report(row, manifest, old_report)
    except (OSError, ValueError) as error:
        row["usage_error"] = f"re-pricing failed: {error}"
        return
    row["usage_report"] = report
    if not row.get("claude_receipts"):
        return
    try:
        row["claude_reconciliation"] = reconcile_claude_receipts(report, row["claude_receipts"])
    except ValueError as error:
        row["claude_reconciliation_error"] = f"re-reconciliation failed: {error}"


def reprice_manifest(path, *, price_table=None, output_dir=None):
    path = Path(path)
    manifest = json.loads(path.read_text())
    table = validate_price_table(price_table if price_table is not None else manifest.get("price_table"))
    if table["binary_sha256"] != manifest.get("binary", {}).get("sha256"):
        raise ValueError("price_table SHA-256 does not match manifest binary")
    result = copy.deepcopy(manifest)
    result["price_table"] = table
    for row in result["trials"]:
        rebuild_trial_usage(row, result)
    output = path.with_suffix(".repriced.json")
    if output_dir is not None:
        output = Path(output_dir) / output.name
    if output.resolve() == path.resolve():
        raise ValueError("re-pricing output must differ from source manifest")
    # Exclusive creation also protects an existing repricing artifact or symlink.
    with output.open("x") as stream:
        stream.write(json.dumps(result, indent=2) + "\n")
    return output


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--price-table", type=Path, help="Saved list_models price table (offline)")
    source.add_argument("--binary", type=Path, help="Fetch prices from this matching pinned binary in isolation")
    parser.add_argument("--scratch-dir", type=Path, help="Required with --binary; owned daemon config/data scratch")
    parser.add_argument("--output-dir", type=Path, help="Existing output directory; default is beside the manifest")
    args = parser.parse_args(argv)
    if args.scratch_dir is not None and not args.binary:
        parser.error("--scratch-dir is only used with --binary")
    table = json.loads(args.price_table.read_text()) if args.price_table else None
    if args.binary:
        if args.scratch_dir is None:
            parser.error("--binary requires --scratch-dir")
        from harness_bench import fetch_price_table
        manifest = json.loads(args.manifest.read_text())
        table = fetch_price_table(str(args.binary.resolve()), manifest["binary"]["sha256"], args.scratch_dir)
    output = reprice_manifest(args.manifest, price_table=table, output_dir=args.output_dir)
    result = json.loads(output.read_text())
    errors = sum(bool(r.get("usage_error")) for r in result["trials"])
    print(f"{output} ({len(result['trials'])} trials, {errors} usage errors)")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
