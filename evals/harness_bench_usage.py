"""Read-only, JSON-safe accounting for native CLI and Aether benchmark captures.

Input tokens INCLUDE cache reads/writes; reasoning is a subset of output.
``usage`` sums only observed values; ``coverage`` must accompany every total.
Missing is never zero. Dollar metadata is API-equivalent, not subscription billing.
Totals are derived per unit before summing; independently partial input/output
rollups need not sum to the reported total_tokens. Coverage is capture coverage,
not a claim that an upstream provider reported all work.

Aether's accounting unit is a started turn, not an assistant message or process.
Its cumulative turn_usage includes failed/retried work and compaction. Snapshots
identify model attempts, but session retries can reuse a snapshot and usage events
have no attempt ID. Consequently exact per-process attribution is unavailable.
Capture every durable event from root creation through descendant quiescence,
paginating get_events(after=..., limit=5000, query={"run": run_id}). Do not use a
root-only subscription or add invocation.completed counters to usage_updated.
"""

import json
import math
from collections import Counter

from harness_bench_pricing import estimate_cost, resolve_price, validate_price_table

FIELDS = (
    "input_tokens", "uncached_input_tokens", "output_tokens", "cache_read_tokens",
    "cache_write_tokens", "reasoning_tokens", "total_tokens", "api_equivalent_cost_usd",
)

AETHER_WARNINGS = (
    "Aether counters have source-level coverage loss: adapters serialize absent token fields as zero",
    "Aether reasoning totals may combine reported and missing values without retaining per-invocation coverage",
    "Claude durable usage can omit helper models; compare separate native modelUsage receipts, never add them",
    "Turn totals include retries/compaction but exact process-attempt and compactor-model attribution is unavailable",
    "Legacy codex exec adapter leaves cache inside input; normalized Aether totals require app-server (verify engine metadata)",
)

TURN_EVENT_TYPES = ("snapshot.created", "agent.usage_updated", "agent.turn_completed",
                    "invocation.failed", "invocation.completed", "tool.requested", "tool.retried")


class UsageError(ValueError):
    """Malformed or ambiguous accounting data; messages never include transcripts."""


def _object(value, label):
    if not isinstance(value, dict):
        raise UsageError(f"{label} must be an object")
    return value


def _number(obj, key, *, money=False):
    value = obj.get(key)
    if value is None:
        return None
    valid = isinstance(value, (int, float) if money else int)
    if isinstance(value, bool) or not valid or value < 0 or not math.isfinite(value):
        raise UsageError(f"invalid nonnegative {'cost' if money else 'counter'}: {key}")
    return value


def _sum_known(values):
    values = list(values)
    return sum(values) if all(v is not None for v in values) else None


def _empty_usage():
    return dict.fromkeys(FIELDS)


def normalize_usage(raw, schema):
    """Normalize codex, claude, claude_model, or aether counters without pricing."""
    if raw is None:
        return _empty_usage()
    raw = _object(raw, "usage")
    keys = {
        "codex": ("input_tokens", "output_tokens", "cached_input_tokens", "cache_write_input_tokens", "reasoning_output_tokens"),
        "claude": ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens", None),
        "claude_model": ("inputTokens", "outputTokens", "cacheReadInputTokens", "cacheCreationInputTokens", "thinkingTokens"),
        "aether": ("input_tokens", "output_tokens", "cache_read_tokens", "cache_write_tokens", "reasoning_tokens"),
    }
    if schema not in keys:
        raise UsageError(f"unknown usage schema: {schema}")
    inp, out, read, write, reasoning = keys[schema]
    usage = _empty_usage()
    reported_input = _number(raw, inp)
    usage["output_tokens"] = _number(raw, out)
    usage["cache_read_tokens"] = _number(raw, read)
    usage["cache_write_tokens"] = _number(raw, write) if write else None
    if reasoning:
        usage["reasoning_tokens"] = _number(raw, reasoning)
    elif raw.get("output_tokens_details") is not None:
        details = _object(raw["output_tokens_details"], "output_tokens_details")
        usage["reasoning_tokens"] = _number(details, "thinking_tokens")
    if schema == "codex":
        usage["input_tokens"] = reported_input
        cached = usage["cache_read_tokens"]
        writes = usage["cache_write_tokens"]
        # Assumes reads and writes are disjoint subsets of inclusive input.
        if cached is not None and writes is not None:
            cached += writes
        if reported_input is not None and cached is not None:
            if cached > reported_input:
                raise UsageError("cached input exceeds inclusive input")
            usage["uncached_input_tokens"] = reported_input - cached
    else:
        usage["uncached_input_tokens"] = reported_input
        usage["input_tokens"] = _sum_known(
            [reported_input, usage["cache_read_tokens"], usage["cache_write_tokens"]])
    usage["total_tokens"] = _sum_known([usage["input_tokens"], usage["output_tokens"]])
    output, reasoning = usage["output_tokens"], usage["reasoning_tokens"]
    if output is not None and reasoning is not None and reasoning > output:
        raise UsageError("reasoning exceeds inclusive output")
    cost_key = {"claude_model": "costUSD", "aether": "cost_usd"}.get(schema)
    if cost_key:
        cost = _number(raw, cost_key, money=True)
        # Aether serializes an unknown price as zero plus cost_complete=false.
        if schema != "aether" or raw.get("cost_complete") is True or cost:
            usage["api_equivalent_cost_usd"] = cost
    if schema == "aether" and raw.get("cost_complete") is False:
        estimate = _number(raw, "estimated_cost_usd", money=True)
        if estimate is not None:
            # Runtime estimates cover unbilled invocations; retain billed work too.
            usage["api_equivalent_cost_usd"] = (cost or 0) + estimate
    return usage


def _unit_cost_basis(unit):
    if unit["usage"]["api_equivalent_cost_usd"] is None:
        return "unpriced"
    return unit.get("cost_basis", "billed")


def _cost_priced(unit):
    return (unit["usage"]["api_equivalent_cost_usd"] is not None
            and (unit.get("cost_complete") is not False
                 or _unit_cost_basis(unit).startswith("estimated")))


def _rollup(units):
    usage, coverage = {}, {}
    for field in FIELDS:
        values = [u["usage"][field] for u in units if u["usage"][field] is not None]
        usage[field] = sum(values) if values else None
        terminal = sum(u.get("terminal", False) and u["usage"][field] is not None
                       and (field != "api_equivalent_cost_usd" or _cost_priced(u))
                       for u in units)
        coverage[field] = {"reported": len(values), "terminal": terminal, "units": len(units),
                           "complete": bool(units) and terminal == len(units)}
    cost = coverage["api_equivalent_cost_usd"]
    cost["billed"] = sum(_cost_priced(u) and _unit_cost_basis(u) == "billed" for u in units)
    cost["estimated"] = sum(_cost_priced(u) and _unit_cost_basis(u).startswith("estimated") for u in units)
    cost["unpriced"] = len(units) - cost["billed"] - cost["estimated"]
    return usage, coverage


def _report(source, units, warnings, **metadata):
    for unit in units:
        unit["cost_basis"] = _unit_cost_basis(unit)
    usage, coverage = _rollup(units)
    return {"source": source, "usage": usage, "coverage": coverage, "units": units,
            "warnings": list(dict.fromkeys(warnings)),
            "cost_basis": "API-equivalent reported metadata or list-price estimate; not actual subscription billed cost",
            "accounting_scope": "observed counters; missing work is not estimated", **metadata}


def _jsonl(text):
    if not isinstance(text, str):
        raise UsageError("JSONL input must be text")
    events, warnings = [], []
    lines = text.splitlines(keepends=True)
    for index, line in enumerate(lines):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            if index == len(lines) - 1 and not line.endswith(("\n", "\r")):
                warnings.append("incomplete trailing JSONL record excluded")
                break
            raise UsageError(f"malformed JSONL record at line {index + 1}") from error
        events.append(_object(event, f"JSONL record {index + 1}"))
    return events, warnings


def parse_codex_jsonl(text, *, price_table=None, model=None, model_basis="configured default model"):
    """Count each terminal exec turn once, never cumulative token_count samples.

    model_basis labels where ``model`` came from; it is recorded as the unit's
    cost basis and never used to decide how a unit is priced.
    """
    if not isinstance(model_basis, str) or not model_basis:
        raise UsageError("codex model basis must be nonempty text")
    events, warnings = _jsonl(text)
    units, tools, threads = [], {}, set()
    current = None
    for event in events:
        kind = event.get("type")
        if kind == "thread.started":
            thread = event.get("thread_id")
            if thread in threads:
                raise UsageError("duplicate Codex thread start; captures may overlap")
            threads.add(thread)
        elif kind == "turn.started":
            if current is not None and not current["terminal"]:
                warnings.append("Codex started a new turn before closing the previous one")
            current = {"turn": len(units) + 1, "terminal": False, "status": "incomplete", "usage": _empty_usage()}
            units.append(current)
        elif kind in ("turn.completed", "turn.failed"):
            if current is None:
                current = {"turn": len(units) + 1, "usage": _empty_usage(), "terminal": False}
                units.append(current)
                warnings.append("Codex terminal record has no turn.started boundary")
            if current["terminal"]:
                raise UsageError("duplicate Codex turn terminal without a new turn.started")
            current.update(terminal=True, status="completed" if kind == "turn.completed" else "failed")
            if kind == "turn.completed":
                current["usage"] = normalize_usage(event.get("usage"), "codex")
        elif kind == "error":
            warnings.append("Codex emitted an error; exit status must be checked separately")
        elif kind in ("item.started", "item.completed"):
            item = _object(event.get("item"), "Codex item")
            if (item.get("type") in ("command_execution", "mcp_tool_call", "web_search", "file_change")
                    and item.get("id") is not None):
                tools[item["id"]] = item["type"]
    if not units:
        units.append({"turn": None, "terminal": False, "status": "incomplete", "usage": _empty_usage()})
    if any(u["usage"]["total_tokens"] is None for u in units):
        warnings.append("Codex turns without complete terminal counters remain unknown")
    if price_table is not None:
        validate_price_table(price_table)
        for unit in units:
            _price_unit(unit, price_table, "codex", [model] if model else [],
                        f"estimated ({model_basis})", require_cache_write=True)
    return _report("codex_exec", units, warnings, tool_calls=dict(Counter(tools.values())))


def parse_claude_jsonl(text):
    """Parse a fresh print invocation, including helper models in modelUsage.

    For resumed processes, these fields are session totals: use receipt
    reconciliation with recorder provenance to derive invocation deltas.
    """
    events, warnings = _jsonl(text)
    results, tools, resolved = [], {}, set()
    for event in events:
        if event.get("type") == "result":
            origin = event.get("origin") or {}
            if _object(origin, "Claude origin").get("kind") == "task-notification":
                warnings.append("queued task-notification result excluded from prompt accounting")
            else:
                results.append(event)
        if event.get("type") == "system" and event.get("subtype") == "init" and event.get("model"):
            resolved.add(event["model"])
        if event.get("type") == "assistant":
            message = _object(event.get("message"), "Claude assistant message")
            for block in message.get("content", []):
                if block.get("type") == "tool_use" and block.get("id") is not None:
                    tools[block["id"]] = block.get("name", "unknown")
    if len(results) > 1:
        raise UsageError("multiple Claude prompt results; parse each process capture separately")
    unit = {"terminal": bool(results), "status": "incomplete", "usage": _empty_usage(), "models": []}
    if results:
        result = results[0]
        unit["status"] = "failed" if result.get("is_error") or result.get("subtype") != "success" else "completed"
        models = _object(result.get("modelUsage") or {}, "Claude modelUsage")
        for model, counters in models.items():
            model_unit = {"model": model, "terminal": True,
                          "usage": normalize_usage(counters, "claude_model")}
            model_unit["cost_basis"] = _unit_cost_basis(model_unit)
            unit["models"].append(model_unit)
        if models:
            unit["usage"], unit["model_coverage"] = _rollup(unit["models"])
            # Preserve missing model fields at the process boundary as well.
            for field, coverage in unit["model_coverage"].items():
                if not coverage["complete"]:
                    unit["usage"][field] = None
            unit["usage_source"] = "result.modelUsage (all reported models)"
        else:
            unit["usage"] = normalize_usage(result.get("usage"), "claude")
            unit["usage_source"] = "result.usage"
            warnings.append("Claude modelUsage missing; result.usage may omit helper-model work")
        cost = _number(result, "total_cost_usd", money=True)
        if cost is not None:
            unit["usage"]["api_equivalent_cost_usd"] = cost
    else:
        warnings.append("Claude result missing; provisional assistant usage is not a terminal receipt")
    return _report("claude_print", [unit], warnings, resolved_models=sorted(resolved),
                   tool_calls=dict(Counter(tools.values())))


def normalize_claude_receipt(receipt):
    """Normalize one result without retaining text, tools, argv, or credentials."""
    if receipt is None:
        return parse_claude_jsonl("")
    receipt = _object(receipt, "Claude receipt")
    if receipt.get("type") != "result":
        raise UsageError("Claude receipt must be a result event")
    return parse_claude_jsonl(json.dumps(receipt))


def reconcile_claude_receipts(durable_report, captures):
    """Compare, never add, process receipts and the durable ledger.

    Each capture has a unique capture_id, optional agent_id, and receipt (None
    for a killed/failed process without a result). The caller must inventory
    every process, including compaction, rather than pass only successful ones.
    """
    from harness_bench_sessions import invocation_units

    units, warnings = invocation_units(captures, normalize_claude_receipt)
    if not captures:
        warnings.append("no process captures; full-process usage is unknown")
    native = _report("claude_process_receipts", units, warnings)
    durable = durable_report["usage"]
    difference = {
        field: native["usage"][field] - durable[field]
        if native["usage"][field] is not None and durable[field] is not None else None
        for field in FIELDS
    }
    root = durable_report.get("root_agent_id")
    selected = set(durable_report.get("agent_ids", []))
    root_units = [u for u in units if root is not None and u["agent_id"] == root]
    descendants = [u for u in units if u["agent_id"] in selected and u["agent_id"] != root]
    unattributed = [u for u in units if u["agent_id"] not in selected]
    root_usage, root_coverage = _rollup(root_units)
    child_usage, child_coverage = _rollup(descendants)
    if unattributed:
        native["warnings"].append("some process receipts cannot be attributed to root/descendants")
    return {"full_process_usage": native["usage"], "coverage": native["coverage"],
            "units": units, "native_minus_durable": difference,
            "root_usage": root_usage, "root_coverage": root_coverage,
            "descendant_usage": child_usage, "descendant_coverage": child_coverage,
            "unattributed_processes": len(unattributed), "warnings": native["warnings"],
            "cost_basis": native["cost_basis"],
            "comparison_note": "Separate sources, not additive. Differences are diagnostic, not corrected billing; inventory completeness must be checked."}


def _ordered_events(events):
    by_seq, ids = {}, {}
    for event in events:
        event = _object(event, "durable event")
        seq = _number(event, "seq")
        if seq is None:
            raise UsageError("durable event is missing seq")
        _object(event.get("kind"), "event kind")
        if seq in by_seq and by_seq[seq] != event:
            raise UsageError("conflicting durable events with the same seq")
        identity = event.get("id")
        if identity in ids and ids[identity] != seq:
            raise UsageError("duplicate durable event ID at different sequences")
        if identity is not None:
            ids[identity] = seq
        by_seq[seq] = event
    return [by_seq[seq] for seq in sorted(by_seq)]


def _subtree(events, root):
    children = {}
    for event in events:
        kind, agent = event["kind"], event.get("agent_id")
        parent = kind.get("parent") if kind["type"] == "agent.created" else event.get("parent_agent_id")
        if parent and agent:
            children.setdefault(parent, set()).add(agent)
        if kind["type"] == "agent.restarted":
            children.setdefault(kind["from"], set()).add(kind["to"])
    selected, pending = set(), [root]
    while pending:
        agent = pending.pop()
        if agent not in selected:
            selected.add(agent)
            pending.extend(children.get(agent, ()))
    return selected


def _new_turn(agent, turn, effort, event):
    return {"agent_id": agent, "turn": turn, "start_seq": event["seq"], "end_seq": None,
            "terminal": False, "status": "incomplete", "effort": effort,
            "usage": _empty_usage(), "usage_updates": 0, "attempts": [], "tool_calls": {}}


def _update_usage(unit, kind, warnings):
    usage = normalize_usage(kind.get("turn_usage"), "aether")
    previous = unit["usage"]
    for field in FIELDS:
        old, new = previous[field], usage[field]
        if old is not None and new is not None and new < old and field != "api_equivalent_cost_usd":
            raise UsageError(f"Aether cumulative turn counter decreased: {field}")
        if old is not None and new is None:
            warnings.append(f"Aether cumulative update dropped {field}; terminal coverage remains unknown")
    unit["usage"] = usage
    unit["usage_updates"] += 1
    raw = kind.get("turn_usage") or {}
    unit["cost_complete"] = raw.get("cost_complete")
    unit["cost_basis"] = ("estimated" if raw.get("cost_complete") is False
                          and raw.get("estimated_cost_usd") is not None else "billed")


def _check_cumulative_totals(units, totals, warnings):
    by_agent = {}
    for unit in units:
        by_agent.setdefault(unit["agent_id"], []).append(unit)
    for agent, total in totals.items():
        own = by_agent.get(agent, [])
        for field in FIELDS:
            reported = total[field]
            values = [u["usage"][field] for u in own]
            if reported is None or any(value is None for value in values):
                continue
            observed = sum(values)
            equal = math.isclose(observed, reported, rel_tol=1e-9, abs_tol=1e-9) \
                if field == "api_equivalent_cost_usd" else observed == reported
            if not equal:
                raise UsageError(f"Aether turn sum disagrees with cumulative agent total for {field}; incomplete capture or accounting mismatch")
    if set(by_agent) - set(totals):
        warnings.append("some agents have no absolute usage total to cross-check against turn accounting")


def _price_unit(unit, table, provider, models, basis="estimated", *, require_cache_write=False):
    """Estimate an unpriced unit at list price, retaining any partial billed floor."""
    assert isinstance(basis, str) and basis.startswith("estimated"), "list-price basis must be labelled estimated"
    if _cost_priced(unit) or len(models) != 1:
        return
    price = resolve_price(table, provider, models[0])
    if price is None:
        return
    # Without the inclusive-input breakdown, uncached input cannot be priced.
    if require_cache_write and unit["usage"]["cache_write_tokens"] is None:
        return
    amount = estimate_cost(unit["usage"], price)
    if amount is None:
        return
    floor = unit["usage"]["api_equivalent_cost_usd"]
    if floor is not None:
        unit["billed_floor_usd"] = floor
    unit["usage"]["api_equivalent_cost_usd"] = amount
    unit["cost_basis"] = basis
    unit["priced_model"] = price["model"]


def _record_session_model(identities, active, agent, kind):
    """Record an engine-resolved model for the agent and, if open, its turn."""
    model = _object(kind.get("payload"), "engine session").get("model")
    if model is None:
        return
    identity = identities.setdefault(agent, {"requested_model": None, "resolved_models": []})
    if model not in identity["resolved_models"]:
        identity["resolved_models"].append(model)
    unit = active.get(agent)
    if unit is not None and model not in unit.setdefault("session_models", []):
        unit["session_models"].append(model)


def _apply_turn_event(unit, event, warnings, calls, totals):
    """Apply one in-turn event to its unit; True means the turn closed here.

    invocation.completed is a message, often with placeholder zero usage: only
    the cumulative turn ledger is authoritative, so it is deliberately ignored.
    """
    kind = event["kind"]
    typ = kind["type"]
    assert typ in TURN_EVENT_TYPES, f"non-turn event routed to turn handler: {typ}"
    if typ == "snapshot.created":
        snapshot = kind.get("snapshot")
        if not any(a["snapshot"] == snapshot for a in unit["attempts"]):
            unit["attempts"].append({"snapshot": snapshot, "model": kind.get("model"),
                                     "status": "observed", "usage": None})
    elif typ == "agent.usage_updated":
        _update_usage(unit, kind, warnings)
        if kind.get("total") is not None:
            totals[unit["agent_id"]] = normalize_usage(kind["total"], "aether")
    elif typ == "agent.turn_completed":
        if unit["turn"] is not None and kind.get("turn") != unit["turn"]:
            raise UsageError("Aether turn completion does not match active turn")
        unit.update(terminal=True, end_seq=event["seq"], status=kind.get("next", "completed"))
        return True
    elif typ == "invocation.failed":
        for attempt in unit["attempts"]:
            if attempt["snapshot"] == kind.get("snapshot"):
                attempt["status"] = "cancelled" if kind.get("cancelled") else "failed"
    elif typ == "tool.requested":
        call = _object(kind.get("call"), "tool call")
        call_id = call.get("id")
        if not call_id:
            raise UsageError("tool.requested is missing call ID")
        if call_id not in calls:
            calls[call_id] = call.get("name", "unknown")
            unit["tool_calls"][calls[call_id]] = unit["tool_calls"].get(calls[call_id], 0) + 1
    elif typ == "tool.retried":
        unit["tool_retries"] = unit.get("tool_retries", 0) + 1
    return False


def _finalize_units(units, state, warnings, price_table):
    """Add units for silent agents, cross-check totals, then price what is unpriced."""
    for agent in sorted(state["observed_agents"] - state["created"]):
        warnings.append(f"agent creation absent for {agent}; earlier spend/lineage may be missing")
    for agent in sorted(state["selected"] - {u["agent_id"] for u in units}):
        units.append({"agent_id": agent, "turn": None, "terminal": False,
                      "status": "no_observed_turn", "usage": _empty_usage()})
    for unit in units:
        if not unit["terminal"]:
            warnings.append("open/missing Aether turn boundary: usage is provisional, not a final receipt")
        if not unit.get("usage_updates"):
            warnings.append("Aether turn has no cumulative usage receipt; invocation messages are not terminal billing receipts")
    _check_cumulative_totals(units, state["totals"], warnings)
    for unit in units:
        requested = state["identities"].get(unit["agent_id"], {}).get("requested_model") or {}
        if price_table is not None:
            _price_unit(unit, price_table, requested.get("provider"), unit.get("session_models", []))
        if unit.get("cost_complete") is False and not _cost_priced(unit):
            warnings.append("Aether API-equivalent cost is an observed floor with unpriced work")


def parse_aether_events(events, root_agent_id, *, initial_efforts=None, price_table=None):
    """Normalize complete-run durable events, including descendants and restarts.

    initial_efforts maps agent IDs to creation-time config.effort, NOT final row
    values. SnapshotCreated omits params.effort: capture get_snapshot(id).params
    separately to verify effective effort, including Codex's medium default.
    """
    if price_table is not None:
        validate_price_table(price_table)
    events = _ordered_events(events)
    selected = _subtree(events, root_agent_id)
    warnings = list(AETHER_WARNINGS)
    efforts, units, active, identities, calls = dict(initial_efforts or {}), [], {}, {}, {}
    created, observed_agents, totals = set(), set(), {}
    for event in events:
        agent, kind = event.get("agent_id"), event["kind"]
        if agent not in selected:
            continue
        observed_agents.add(agent)
        typ = kind["type"]
        if typ == "agent.created":
            created.add(agent)
            identities[agent] = {"requested_model": kind.get("model"), "resolved_models": []}
        elif typ == "agent.config_updated":
            efforts[agent] = _object(kind.get("config"), "agent config").get("effort")
        elif typ == "agent.started":
            if agent in active:
                warnings.append("Aether turn has no closing boundary before the next start")
            unit = _new_turn(agent, kind.get("turn"), efforts.get(agent), event)
            unit["session_models"] = []
            active[agent] = unit
            units.append(unit)
        elif typ == "custom" and kind.get("name") == "engine.session":
            _record_session_model(identities, active, agent, kind)
        elif typ in TURN_EVENT_TYPES:
            unit = active.get(agent)
            if unit is None:
                unit = _new_turn(agent, kind.get("turn"), efforts.get(agent), event)
                active[agent] = unit
                units.append(unit)
                warnings.append("Aether records lack agent.started; capture may be incomplete")
            if _apply_turn_event(unit, event, warnings, calls, totals):
                active.pop(agent)
    state = {"selected": selected, "created": created, "observed_agents": observed_agents,
             "totals": totals, "identities": identities}
    _finalize_units(units, state, warnings, price_table)
    root_units = [u for u in units if u["agent_id"] == root_agent_id]
    child_units = [u for u in units if u["agent_id"] != root_agent_id]
    root_usage, root_coverage = _rollup(root_units)
    child_usage, child_coverage = _rollup(child_units)
    return _report("aether_durable", units, warnings, root_agent_id=root_agent_id,
                   agent_ids=sorted(selected), identities=identities,
                   agent_cumulative_usage=totals,
                   root_usage=root_usage, root_coverage=root_coverage,
                   descendant_usage=child_usage, descendant_coverage=child_coverage,
                   tool_calls=dict(Counter(calls.values())))
