#!/usr/bin/env python3
"""Copy Cursor chats into .ai/data using the Wardenclyffe session records.

Two modes:

  python .ai/machine/copy_chats.py --sync
      Stream agent transcripts for this repo. Unchanged files are skipped.

  python .ai/machine/copy_chats.py --hook <event>
      Cursor hook. Reads one JSON payload on stdin, spools it, files it.

Fail-open: a hook always prints a JSON object and exits 0.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import sys
import tempfile
import uuid
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / ".ai" / "modules" / "chats" / "data"
PROJECT_KEY = "shippin-tech"

SECRET_KEY_RE = re.compile(
    r"(password|passwd|secret|token|api[_-]?key|authorization|credential|private[_-]?key)",
    re.IGNORECASE,
)
SECRET_VALUE_RE = re.compile(
    r"(?i)\b(bearer\s+\S{8,}|sk-[a-z0-9]{16,}|ghp_[a-z0-9]{16,}|xox[baprs]-[a-z0-9-]{16,})"
)


def now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def redact(value: Any, redactions: list[str]) -> Any:
    if isinstance(value, dict):
        out: dict[str, Any] = {}
        for key, item in value.items():
            if SECRET_KEY_RE.search(str(key)):
                out[key] = "[REDACTED]"
                redactions.append(str(key))
            else:
                out[key] = redact(item, redactions)
        return out
    if isinstance(value, list):
        return [redact(item, redactions) for item in value]
    if isinstance(value, str) and SECRET_VALUE_RE.search(value):
        redactions.append("secret_like_value")
        return SECRET_VALUE_RE.sub("[REDACTED]", value)
    return value


def append_jsonl(path: Path, record: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    line = json.dumps(record, separators=(",", ":"), ensure_ascii=False) + "\n"
    with path.open("a", encoding="utf-8") as handle:
        handle.write(line)


def session_dir(session_key: str) -> Path:
    clean = re.sub(r"[^A-Za-z0-9._-]", "_", session_key).strip("._") or "session"
    return DATA / "sessions" / clean[:120]


def event_record(
    session_key: str,
    hook_event: str,
    payload: Any,
    source: str,
    event_id: str,
) -> dict[str, Any]:
    redactions: list[str] = []
    sanitized = redact(payload, redactions)
    stamped = now()
    return {
        "event_id": event_id,
        "project_key": PROJECT_KEY,
        "session_key": session_key,
        "hook_event": hook_event,
        "event_kind": source,
        "source": source,
        "payload": sanitized if isinstance(sanitized, dict) else {"value": sanitized},
        "redactions": sorted(set(redactions)),
        "captured_at": stamped,
        "received_at": stamped,
    }


def text_parts(content: Any) -> list[tuple[str, Any]]:
    if isinstance(content, str):
        return [("text", content)]
    if not isinstance(content, list):
        return []
    parts: list[tuple[str, Any]] = []
    for block in content:
        if isinstance(block, str):
            parts.append(("text", block))
            continue
        if not isinstance(block, dict):
            continue
        kind = str(block.get("type") or "")
        if kind in {"text", "thinking", "reasoning"}:
            parts.append((kind, block.get("text") or block.get("thinking") or ""))
        elif kind in {"tool_use", "tool_call"}:
            parts.append(("tool", block))
    return parts


def file_line(base: Path, name: str, record: dict[str, Any]) -> None:
    append_jsonl(base / name, record)


def consume_transcript_line(base: Path, session_key: str, source: str, raw: str, event_id: str) -> dict[str, int]:
    counts = {"events": 0, "messages": 0, "tools": 0, "reasoning": 0}
    try:
        row = json.loads(raw)
    except json.JSONDecodeError:
        row = {"raw": raw[:8192], "parse_error": True}
    if not isinstance(row, dict):
        row = {"value": row}

    kind = str(row.get("type") or row.get("role") or "line")
    record = event_record(session_key, kind, row, source, event_id)
    file_line(base, "events.jsonl", record)
    counts["events"] = 1

    role = str(row.get("role") or "")
    message = row.get("message") if isinstance(row.get("message"), dict) else {}
    for part_kind, part in text_parts(message.get("content")):
        if part_kind == "text" and isinstance(part, str) and part.strip() and role in {"user", "assistant"}:
            file_line(
                base,
                "messages.jsonl",
                {
                    "project_key": PROJECT_KEY,
                    "session_key": session_key,
                    "event_id": event_id,
                    "role": role,
                    "content": redact(part, record["redactions"]),
                    "created_at": record["captured_at"],
                },
            )
            counts["messages"] += 1
        elif part_kind in {"thinking", "reasoning"} and isinstance(part, str) and part.strip():
            file_line(
                base,
                "reasoning.jsonl",
                {
                    "project_key": PROJECT_KEY,
                    "session_key": session_key,
                    "event_id": event_id,
                    "kind": "thought",
                    "content": redact(part, record["redactions"]),
                    "created_at": record["captured_at"],
                },
            )
            counts["reasoning"] += 1
        elif part_kind == "tool" and isinstance(part, dict):
            redactions: list[str] = []
            file_line(
                base,
                "tools.jsonl",
                {
                    "project_key": PROJECT_KEY,
                    "session_key": session_key,
                    "event_id": event_id,
                    "tool_name": part.get("name"),
                    "status": "observed",
                    "input": redact(part.get("input") or {}, redactions),
                    "created_at": record["captured_at"],
                },
            )
            counts["tools"] += 1
    return counts


def load_index() -> dict[str, Any]:
    path = DATA / "index.json"
    if not path.is_file():
        return {"sources": {}}
    try:
        loaded = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {"sources": {}}
    if not isinstance(loaded, dict) or not isinstance(loaded.get("sources"), dict):
        return {"sources": {}}
    return loaded


def save_index(index: dict[str, Any]) -> None:
    path = DATA / "index.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")


def seen_path(base: Path) -> Path:
    return base / "seen.txt"


def load_seen(base: Path) -> set[str]:
    path = seen_path(base)
    if not path.is_file():
        return set()
    return {line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()}


def remember(base: Path, event_id: str, seen: set[str]) -> None:
    seen.add(event_id)
    with seen_path(base).open("a", encoding="utf-8") as handle:
        handle.write(event_id + "\n")


def transcript_roots() -> list[Path]:
    roots: list[Path] = []
    projects = Path.home() / ".cursor" / "projects"
    if not projects.is_dir():
        return roots
    for child in projects.iterdir():
        name = child.name.lower()
        if "shippin-tech" in name or "mail-shippin" in name:
            transcripts = child / "agent-transcripts"
            if transcripts.is_dir():
                roots.append(transcripts)
    return roots


def sync_transcripts() -> dict[str, int]:
    totals = {"files": 0, "events": 0, "messages": 0, "tools": 0, "reasoning": 0, "skipped": 0}
    index = load_index()
    sources: dict[str, Any] = index["sources"]
    for root in transcript_roots():
        for path in sorted(root.glob("*/*.jsonl")):
            totals["files"] += 1
            key = str(path.resolve())
            size = path.stat().st_size
            prior = sources.get(key) if isinstance(sources.get(key), dict) else {}
            offset = int(prior.get("offset") or 0)
            if offset > size:
                offset = 0
            if offset == size:
                totals["skipped"] += 1
                continue
            session_key = path.parent.name
            base = session_dir(session_key)
            seen = load_seen(base) if offset else set()
            if offset == 0 and seen_path(base).is_file():
                # File was replaced. Keep old lines; hashes stop duplicates.
                seen = load_seen(base)
            with path.open("rb") as handle:
                handle.seek(offset)
                while True:
                    blob = handle.readline()
                    if not blob:
                        break
                    offset = handle.tell()
                    raw = blob.decode("utf-8", errors="replace").strip()
                    if not raw:
                        continue
                    event_id = hashlib.sha256(f"{key}\n{raw}".encode("utf-8")).hexdigest()
                    if event_id in seen:
                        continue
                    counts = consume_transcript_line(base, session_key, "cursor-transcript", raw, event_id)
                    remember(base, event_id, seen)
                    for name in ("events", "messages", "tools", "reasoning"):
                        totals[name] += counts[name]
            sources[key] = {"offset": offset, "size": size, "session_key": session_key, "updated_at": now()}
    save_index(index)
    return totals


def spool_and_file(event_name: str, payload: dict[str, Any]) -> None:
    session_key = ""
    for key in ("conversation_id", "session_id", "composer_id", "generation_id"):
        value = payload.get(key)
        if value:
            session_key = str(value)
            break
    if not session_key:
        digest = hashlib.sha256(str(payload.get("cwd") or ROOT).encode("utf-8")).hexdigest()[:16]
        session_key = f"cursor-{digest}"
    record = event_record(
        session_key,
        event_name,
        payload,
        "cursor-hook",
        f"{session_key}-{event_name}-{uuid.uuid4().hex}",
    )
    append_jsonl(DATA / "spool" / "events.jsonl", record)
    base = session_dir(session_key)
    file_line(base, "events.jsonl", record)
    if event_name in {"beforeSubmitPrompt", "afterAgentResponse"}:
        text = payload.get("prompt") or payload.get("text") or payload.get("response")
        if isinstance(text, str) and text.strip():
            role = "user" if event_name == "beforeSubmitPrompt" else "assistant"
            file_line(
                base,
                "messages.jsonl",
                {
                    "project_key": PROJECT_KEY,
                    "session_key": session_key,
                    "event_id": record["event_id"],
                    "role": role,
                    "content": redact(text, []),
                    "created_at": record["captured_at"],
                },
            )


def self_test() -> None:
    global DATA
    original = DATA
    DATA = Path(tempfile.mkdtemp(prefix="shippin-chat-copy-"))
    try:
        sample = (
            '{"role":"user","message":{"content":[{"type":"text","text":"hello bearer '
            'sk-abcdefghijklmnopqrstuvwxyz0123456789"}]}}\n'
        )
        event_id = "test-event"
        base = session_dir("self-test")
        consume_transcript_line(base, "self-test", "cursor-transcript", sample.strip(), event_id)
        stored = (base / "messages.jsonl").read_text(encoding="utf-8")
        if "sk-abcdefghijklmnopqrstuvwxyz0123456789" in stored:
            raise SystemExit("secret was copied")
        if "[REDACTED]" not in stored or '"role":"user"' not in stored:
            raise SystemExit("message was not filed")
        if not (base / "events.jsonl").is_file():
            raise SystemExit("event was not filed")
        print(json.dumps({"ok": True, "data": str(DATA)}))
    finally:
        DATA = original


def main() -> None:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--sync", action="store_true")
    mode.add_argument("--hook")
    mode.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    if args.sync:
        print(json.dumps(sync_transcripts()))
        return

    raw = ""
    try:
        raw = sys.stdin.read() or "{}"
        payload = json.loads(raw)
        if not isinstance(payload, dict):
            payload = {"value": payload}
    except json.JSONDecodeError:
        payload = {"raw": raw[:8192], "parse_error": True}
    try:
        spool_and_file(args.hook, payload)
    except OSError:
        pass
    response: dict[str, Any] = {}
    if args.hook == "beforeSubmitPrompt":
        response["permission"] = "allow"
    print(json.dumps(response))


if __name__ == "__main__":
    main()
