"""Convert cumulative Claude receipts to invocation units before reconciliation."""

from collections import Counter

from harness_bench_usage import FIELDS, UsageError, normalize_usage


def invocation_units(captures, normalize):
    """Use recorder timestamps, never PID/file order, for session baselines.

    Legacy single-process captures remain readable but their fresh-session
    assumption is flagged. Repeated legacy sessions cannot be reconciled safely.
    """
    seen, units, warnings, history = set(), [], [], {}
    for capture in captures:
        if not isinstance(capture, dict):
            raise UsageError("process capture must be an object")
        identity = capture.get("capture_id")
        if not isinstance(identity, str) or not identity:
            raise UsageError("process capture requires a nonempty capture_id")
        if identity in seen:
            raise UsageError("duplicate process capture_id; receipts would be double counted")
        seen.add(identity)
        receipt = capture.get("receipt")
        if receipt is not None and not isinstance(receipt, dict):
            raise UsageError("Claude receipt must be an object")
        session = (receipt or {}).get("session_id")
        if session is not None and (not isinstance(session, str) or not session):
            raise UsageError("Claude receipt session ID must be nonempty text")
    sessions = Counter((c.get("receipt") or {}).get("session_id") for c in captures)
    repeated = {s for s, count in sessions.items() if s is not None and count > 1}
    if any(c.get("receipt") is None for c in captures):
        warnings.append("missing process receipt; later session deltas can span unobserved invocations")
    timed = []
    for capture in captures:
        session = (capture.get("receipt") or {}).get("session_id")
        timestamp = capture.get("started_ns")
        resumed = capture.get("resumed")
        valid_time = type(timestamp) is int and timestamp >= 0
        if session in repeated and (not valid_time or type(resumed) is not bool):
            raise UsageError("repeated Claude session needs recorder timestamps and resume metadata")
        if timestamp is not None and not valid_time:
            raise UsageError("invalid Claude recorder timestamp")
        if resumed is not None and type(resumed) is not bool:
            raise UsageError("invalid Claude resume metadata")
        timed.append((timestamp if valid_time else -1, capture))
    for _, capture in sorted(timed, key=lambda item: item[0]):
        report = normalize(capture.get("receipt"))
        unit = {**report["units"][0], "capture_id": capture["capture_id"],
                "agent_id": capture.get("agent_id")}
        warnings.extend(report["warnings"])
        raw = capture.get("receipt") or {}
        session, resumed = raw.get("session_id"), capture.get("resumed")
        previous = history.get(session) if session else None
        cumulative = unit["usage"].copy()
        if raw and resumed is None:
            warnings.append("legacy receipt assumes a fresh session; resume provenance is unavailable")
            unit["terminal"] = False
        if previous is not None:
            if capture["started_ns"] == previous["started_ns"]:
                raise UsageError("Claude session capture order is ambiguous")
            if not resumed:
                raise UsageError("fresh Claude invocation reused a captured session ID")
            if previous["agent_id"] != unit["agent_id"]:
                raise UsageError("Claude session crosses agent attribution boundaries")
        if raw and resumed:
            # result.usage is invocation-scoped, unlike modelUsage and dollars.
            fields = FIELDS if raw.get("modelUsage") else ("api_equivalent_cost_usd",)
            for field in fields:
                before = previous["usage"][field] if previous else None
                total = cumulative[field]
                if total is not None and before is not None and total < before:
                    raise UsageError(f"Claude cumulative session counter decreased: {field}")
                unit["usage"][field] = total - before if total is not None and before is not None else None
            if "models" in unit:
                unit["cumulative_models"] = unit.pop("models")
            unit.pop("model_coverage", None)
            unit["usage_source"] = "session receipt delta (invocation usage where modelUsage is absent)"
            if previous is None:
                warnings.append("resumed Claude session has no captured baseline; cumulative work is unknown")
                unit["usage"] = normalize_usage(raw.get("usage"), "claude")
                unit["usage_source"] = "result.usage; session cost and helper-model delta unknown"
        if session and raw:
            # An invocation-only token receipt cannot seed cumulative tokens.
            baseline = cumulative if raw.get("modelUsage") else dict.fromkeys(FIELDS)
            baseline["api_equivalent_cost_usd"] = cumulative["api_equivalent_cost_usd"]
            history[session] = {"usage": baseline, "started_ns": capture.get("started_ns"),
                                "agent_id": unit["agent_id"]}
        units.append(unit)
    return units, warnings
