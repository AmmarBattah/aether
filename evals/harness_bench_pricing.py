"""Reproducible API-equivalent prices from a pinned binary's model inventory."""

import math
import re

PROVIDERS = {"claude-code": "anthropic", "codex": "openai"}
RATES = {"uncached_input_tokens": "input_per_mtok", "output_tokens": "output_per_mtok",
         "cache_read_tokens": "cache_read_per_mtok", "cache_write_tokens": "cache_write_per_mtok"}


def validate_price_table(table):
    if not isinstance(table, dict) or not isinstance(table.get("models"), list):
        raise ValueError("price_table requires a models list")
    if not re.fullmatch(r"[0-9a-f]{64}", str(table.get("binary_sha256", ""))):
        raise ValueError("price_table requires the pinned binary SHA-256")
    seen = set()
    for row in table["models"]:
        if not isinstance(row, dict) or not isinstance(row.get("model"), dict):
            raise ValueError("price_table model must contain a ModelRef")
        ref = row["model"]
        if any(not isinstance(ref.get(k), str) or not ref[k] for k in ("provider", "model")):
            raise ValueError("price_table ModelRef requires provider and model")
        identity = (ref["provider"], ref["model"])
        if identity in seen:
            raise ValueError("duplicate price_table ModelRef")
        seen.add(identity)
        capabilities = row.get("capabilities")
        if not isinstance(capabilities, dict):
            raise ValueError("price_table model requires capabilities")
        aliases = row.get("aliases", [])
        if not isinstance(aliases, list) or any(not isinstance(a, str) or not a for a in aliases):
            raise ValueError("price_table aliases must be nonempty strings")
        if row.get("name") is not None and not isinstance(row["name"], str):
            raise ValueError("price_table model name must be text")
        cost = capabilities.get("cost")
        if cost is None:
            continue
        if not isinstance(cost, dict):
            raise ValueError("price_table cost must be a ModelCost object")
        for rate in RATES.values():
            value = cost.get(rate, 0 if rate.startswith("cache_") else None)
            if (isinstance(value, bool) or not isinstance(value, (int, float))
                    or not math.isfinite(value) or value < 0):
                raise ValueError(f"invalid price_table rate: {rate}")
    return table


def resolve_price(table, provider, model):
    """Resolve IDs, explicit aliases, or unique engine/API display-name pairs.

    Current list_models omits aliases; engine IDs such as `sonnet` can still
    be joined to the API inventory by their shared, unambiguous display name.
    """
    if not table or not isinstance(model, str) or not model or model == "default":
        return None
    if "/" in model:
        provider, model = model.split("/", 1)
    if model == "default":
        return None
    api_provider = PROVIDERS.get(provider, provider)
    api = [r for r in table["models"] if r["model"]["provider"] == api_provider]
    matches = [r for r in api if model == r["model"]["model"]]
    if not matches:
        matches = [r for r in api if model in r.get("aliases", [])]
    if not matches:
        names = {r.get("name") for r in table["models"]
                 if r["model"]["provider"] == provider
                 and (model == r["model"]["model"] or model in r.get("aliases", []))}
        names.discard(None)
        matches = [r for r in api if r.get("name") in names]
    if len(matches) != 1 or matches[0].get("capabilities", {}).get("cost") is None:
        return None
    return matches[0]


def estimate_cost(usage, model):
    """Price known counters only; missing fields matter unless their rate is zero."""
    if usage["uncached_input_tokens"] is None or usage["output_tokens"] is None:
        return None
    cost = model["capabilities"]["cost"]
    total = 0.0
    for field, rate in RATES.items():
        price = cost.get(rate, 0)
        value = usage[field]
        if value is None and price:
            return None
        total += (value or 0) / 1_000_000 * price
    if not math.isfinite(total):
        raise ValueError("API-equivalent cost overflow")
    return total
