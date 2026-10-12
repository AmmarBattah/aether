#!/usr/bin/env python3
"""Compaction recall experiment.

A synthetic brief with N unique facts is fed to an agent over several turns.
Then the context is compacted and the agent is asked questions whose answers
live only in the folded part of the conversation. Recall is scored by exact
match. Three conditions:

  aether      Aether's structured checkpoint compaction (forced via `compact`).
  full        Native Claude with the whole transcript still in context (upper bound).
  summary     Native Claude with a naive free-text summary replacing the transcript
              (what ad-hoc compaction looks like).

Usage:
  evals/.venv/bin/python evals/compaction_recall.py --model anthropic/claude-opus-5-5 [--facts 40] [--questions 20]
                                                    [--binary target/release/aether] [--conditions aether,full,summary]
"""

import argparse
import json
import os
import random
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from aether_client import AetherClient, isolated_env, start_daemon, stop_daemon  # noqa: E402

RESULTS = os.path.join(HERE, "results")

SERVICES = ["billing", "ingest", "search", "notify", "gateway", "ledger", "scheduler", "reports", "auth", "media"]
FIELDS = [
    ("deployment region", ["eu-west-3", "us-east-2", "ap-south-1", "ca-central-1", "sa-east-1", "eu-north-1"]),
    ("on-call owner", ["Priya", "Tomasz", "Ines", "Kwame", "Mei", "Olaf", "Rania", "Diego"]),
    ("database engine", ["Postgres 16", "MySQL 8", "CockroachDB", "SQLite", "DynamoDB", "Cassandra"]),
    ("release cadence", ["weekly", "fortnightly", "monthly", "daily", "quarterly"]),
    ("SLO target", ["99.5%", "99.9%", "99.95%", "99.99%"]),
    ("internal port", ["8081", "9090", "7000", "8443", "5050", "6060", "7171"]),
    ("feature flag prefix", ["ff_", "flag-", "exp.", "toggle_", "gate:"]),
    ("ticket queue", ["OPS-1", "OPS-7", "PLAT-3", "SRE-9", "CORE-2"]),
]


def build_brief(n_facts, seed):
    rng = random.Random(seed)
    facts = []
    used = set()
    while len(facts) < n_facts:
        service = rng.choice(SERVICES)
        field, values = rng.choice(FIELDS)
        if (service, field) in used:
            continue
        used.add((service, field))
        facts.append((service, field, rng.choice(values)))
    sentences = [f"The {field} for the {service} service is {value}." for service, field, value in facts]
    rng.shuffle(sentences)
    return facts, sentences


def chunk(items, parts):
    size = max(1, (len(items) + parts - 1) // parts)
    return [items[i : i + size] for i in range(0, len(items), size)]


def score(answer, expected):
    a = answer.lower()
    return expected.lower() in a


QUESTION = "What is the {field} for the {service} service? Answer with just the value."
INTRO = ("I am going to share a project operations brief in several parts. Read each part carefully and reply with "
         "just 'noted' after each part; I will ask questions later.")


def run_aether(binary, model, brief_parts, questions):
    root = tempfile.mkdtemp(prefix="aether-recall-")
    env = isolated_env(root)
    ws = os.path.join(root, "workspace")
    os.makedirs(ws)
    start_daemon(binary, env, ws)
    usage = dict(tokens_feed=0, tokens_questions=0)
    try:
        c = AetherClient(env["AETHER_SOCKET"])
        agent = c.create_agent(role="analyst", model=model, long_lived=True)
        aid = agent["id"]
        c.subscribe(aid)
        c.send(aid, INTRO)
        c.wait_for_reply(aid)
        for i, part in enumerate(brief_parts, 1):
            c.send(aid, f"Part {i} of {len(brief_parts)}:\n" + "\n".join(part))
            c.wait_for_reply(aid)
        before = c.get_agent(aid)["stats"]["usage"]
        c.compact(aid)
        c.send(aid, "Thanks. Questions follow.")
        c.wait_for_reply(aid)
        ev = c.find_event(aid, "compaction.completed")
        after_compact = c.get_agent(aid)["stats"]["usage"]
        usage["tokens_feed"] = before["input_tokens"] + before["output_tokens"]
        results = []
        for q, expected in questions:
            c.send(aid, q)
            answer = c.wait_for_reply(aid)
            results.append(dict(question=q, expected=expected, answer=answer.strip()[:200], ok=score(answer, expected)))
        final = c.get_agent(aid)["stats"]["usage"]
        usage["tokens_questions"] = (final["input_tokens"] - after_compact["input_tokens"]) + (final["output_tokens"] - after_compact["output_tokens"])
        usage["compacted"] = ev is not None
        return results, usage
    finally:
        stop_daemon(binary, env)


def run_native(model_name, brief_parts, questions, condition):
    import anthropic

    client = anthropic.Anthropic()
    messages = []
    usage = dict(tokens_feed=0, tokens_questions=0)

    def ask(text):
        messages.append({"role": "user", "content": text})
        resp = client.messages.create(model=model_name, max_tokens=2000, messages=messages)
        messages.append({"role": "assistant", "content": resp.content})
        return resp, "".join(b.text for b in resp.content if b.type == "text")

    resp, _ = ask(INTRO)
    usage["tokens_feed"] += resp.usage.input_tokens + resp.usage.output_tokens
    for i, part in enumerate(brief_parts, 1):
        resp, _ = ask(f"Part {i} of {len(brief_parts)}:\n" + "\n".join(part))
        usage["tokens_feed"] += resp.usage.input_tokens + resp.usage.output_tokens
    if condition == "summary":
        resp, summary = ask("Summarize everything you were told so far into concise notes you could rely on later. Notes only.")
        usage["tokens_feed"] += resp.usage.input_tokens + resp.usage.output_tokens
        messages[:] = [
            {"role": "user", "content": "Here are your notes from an earlier conversation:\n" + summary + "\n\nQuestions follow."},
            {"role": "assistant", "content": "Understood."},
        ]
    results = []
    for q, expected in questions:
        resp, answer = ask(q)
        usage["tokens_questions"] += resp.usage.input_tokens + resp.usage.output_tokens
        results.append(dict(question=q, expected=expected, answer=answer.strip()[:200], ok=score(answer, expected)))
    return results, usage


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="anthropic/claude-opus-5-5")
    ap.add_argument("--facts", type=int, default=40)
    ap.add_argument("--parts", type=int, default=5)
    ap.add_argument("--questions", type=int, default=20)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--binary", default=os.path.join(HERE, "..", "target", "release", "aether"))
    ap.add_argument("--conditions", default="aether,full,summary")
    a = ap.parse_args()
    facts, sentences = build_brief(a.facts, a.seed)
    parts = chunk(sentences, a.parts)
    rng = random.Random(a.seed + 1)
    picked = rng.sample(facts, min(a.questions, len(facts)))
    questions = [(QUESTION.format(field=field, service=service), value) for service, field, value in picked]
    model_name = a.model.split("/", 1)[1] if "/" in a.model else a.model
    os.makedirs(RESULTS, exist_ok=True)
    summary_rows = []
    for condition in a.conditions.split(","):
        print(f"→ {condition}", flush=True)
        started = time.time()
        if condition == "aether":
            results, usage = run_aether(os.path.realpath(a.binary), a.model, parts, questions)
        else:
            results, usage = run_native(model_name, parts, questions, condition)
        recall = sum(r["ok"] for r in results) / len(results)
        row = dict(condition=condition, model=a.model, facts=a.facts, questions=len(results), recall=round(recall, 3),
                   seconds=round(time.time() - started, 1), **usage)
        summary_rows.append(row)
        with open(os.path.join(RESULTS, "compaction.jsonl"), "a") as f:
            f.write(json.dumps(dict(row, results=results, at=time.strftime("%Y-%m-%dT%H:%M:%S"))) + "\n")
        misses = [r for r in results if not r["ok"]]
        print(f"   recall {recall:.0%} · feed {usage['tokens_feed']} tok · questions {usage['tokens_questions']} tok · {row['seconds']}s")
        for r in misses[:5]:
            print(f"   miss: {r['question']} → {r['answer'][:60]!r} (expected {r['expected']})")
    print(f"\n{'condition':<10} {'recall':>7} {'feed tok':>9} {'q tok':>8} {'sec':>6}")
    for r in summary_rows:
        print(f"{r['condition']:<10} {r['recall']:>7.0%} {r['tokens_feed']:>9} {r['tokens_questions']:>8} {r['seconds']:>6}")


if __name__ == "__main__":
    main()
