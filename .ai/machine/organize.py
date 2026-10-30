#!/usr/bin/env python3
"""Validate and append the .ai/data records.

The checker reads .ai/schema/*.json and applies required, type, enum,
minLength, minItems, items, and additionalProperties. A record the schema
rejects is a failed check, including records already on disk.

  python .ai/machine/organize.py --check
  python .ai/machine/organize.py --self-test
  python .ai/machine/organize.py journal --session KEY --type decision_made --content "..."
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import uuid
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / ".ai" / "modules"
SOCKET = Path(__file__).resolve().parent / "module.schema.json"


def now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def load_schema(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def fail(path: str, detail: str) -> None:
    raise ValueError(f"{path}: {detail}")


def check_node(schema: dict[str, Any], value: Any, path: str) -> None:
    expected = schema.get("type")
    if isinstance(expected, list):
        if not any(_is_type(value, item) for item in expected):
            fail(path, f"type {type(value).__name__} not in {expected}")
        if value is None:
            return
    elif expected and not _is_type(value, expected):
        fail(path, f"type {type(value).__name__} is not {expected}")
        return
    if isinstance(value, str):
        min_length = schema.get("minLength")
        if isinstance(min_length, int) and len(value) < min_length:
            fail(path, f"shorter than {min_length}")
    enum = schema.get("enum")
    if enum is not None and value not in enum:
        fail(path, f"{value!r} not in {enum}")
    if not isinstance(value, dict):
        if isinstance(value, list) and "items" in schema:
            min_items = schema.get("minItems")
            if isinstance(min_items, int) and len(value) < min_items:
                fail(path, f"fewer than {min_items} items")
            for index, item in enumerate(value):
                check_node(schema["items"], item, f"{path}[{index}]")
        return
    if schema.get("additionalProperties") is False:
        allowed = set(schema.get("properties") or {})
        extra = set(value) - allowed
        if extra:
            fail(path, f"unknown fields {sorted(extra)}")
    for name in schema.get("required") or []:
        if name not in value:
            fail(path, f"missing {name}")
    for name, prop in (schema.get("properties") or {}).items():
        if name in value:
            check_node(prop, value[name], f"{path}.{name}")


def _is_type(value: Any, expected: str) -> bool:
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "null":
        return value is None
    return False


def check_jsonl(path: Path, schema: dict[str, Any], errors: list[str]) -> int:
    if not path.is_file():
        return 0
    count = 0
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        count += 1
        where = f"{path}:{number}"
        try:
            row = json.loads(line)
            check_node(schema, row, where)
        except (json.JSONDecodeError, ValueError) as exc:
            errors.append(str(exc) if isinstance(exc, ValueError) else f"{where}: invalid json")
    return count


def module_dirs() -> list[Path]:
    if not MODULES.is_dir():
        raise SystemExit(f"missing {MODULES}")
    return sorted(path for path in MODULES.iterdir() if path.is_dir())


def check_tree() -> dict[str, int]:
    errors: list[str] = []
    counts = {"modules": 0, "lines": 0, "documents": 0}
    socket = load_schema(SOCKET)
    for directory in module_dirs():
        manifest_path = directory / "module.json"
        if not manifest_path.is_file():
            errors.append(f"{directory.name}: missing module.json")
            continue
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            check_node(socket, manifest, str(manifest_path))
        except (json.JSONDecodeError, ValueError) as exc:
            errors.append(str(exc))
            continue
        if manifest.get("id") != directory.name:
            errors.append(f"{directory.name}: id does not match the folder")
        if not (directory / "SKILL.md").is_file():
            errors.append(f"{directory.name}: missing SKILL.md")
        if not (directory / "schema").is_dir():
            errors.append(f"{directory.name}: missing schema/")
        if not (directory / "data").is_dir():
            errors.append(f"{directory.name}: missing data/")
        if not manifest.get("jsonl") and not manifest.get("json"):
            errors.append(f"{directory.name}: module.json declares no records")
        counts["modules"] += 1
        for spec in manifest.get("jsonl") or []:
            schema = load_schema(directory / spec["schema"])
            matched = list(directory.glob(spec["glob"]))
            for path in matched:
                added = check_jsonl(path, schema, errors)
                key = path.name.removesuffix(".jsonl")
                counts[key] = counts.get(key, 0) + added
                counts["lines"] += added
        for spec in manifest.get("json") or []:
            schema = load_schema(directory / spec["schema"])
            for path in directory.glob(spec["glob"]):
                counts["documents"] += 1
                try:
                    row = json.loads(path.read_text(encoding="utf-8"))
                    check_node(schema, row, str(path))
                    if spec.get("implemented_needs_result") and row.get("status") == "implemented":
                        for index, step in enumerate(row.get("verification") or []):
                            if not str(step.get("result") or "").strip():
                                errors.append(f"{path}: verification[{index}] has no result")
                except (json.JSONDecodeError, ValueError) as exc:
                    errors.append(str(exc))
        index_rel = manifest.get("index")
        if index_rel:
            index = directory / index_rel
            if not index.is_file():
                errors.append(f"missing {index}")
            else:
                try:
                    sources = json.loads(index.read_text(encoding="utf-8")).get("sources")
                    if not isinstance(sources, dict) or not sources:
                        errors.append(f"{index}: no sources")
                    else:
                        for source, meta in sources.items():
                            if not Path(source).is_file():
                                errors.append(f"index source missing: {source}")
                            if not isinstance(meta, dict) or "offset" not in meta or "session_key" not in meta:
                                errors.append(f"index source incomplete: {source}")
                except json.JSONDecodeError:
                    errors.append(f"{index}: invalid json")
    if errors:
        raise SystemExit("\n".join(errors))
    return counts


def journal_module() -> Path:
    path = MODULES / "journal"
    if not (path / "module.json").is_file():
        raise SystemExit("journal module is not plugged in")
    return path


def append_journal(session_key: str, entry_type: str, content: str, agent_id: str = "main") -> dict[str, Any]:
    module = journal_module()
    entry = {
        "entry_id": uuid.uuid4().hex,
        "project_key": "shippin-tech",
        "session_key": session_key,
        "agent_id": agent_id,
        "entry_type": entry_type,
        "content": content,
        "related_args": {},
        "related_result": {},
        "source": "organize",
        "created_at": now(),
    }
    check_node(load_schema(module / "schema" / "journal-entry.schema.json"), entry, "journal")
    path = module / "data" / "journal.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry, separators=(",", ":"), ensure_ascii=False) + "\n")
    return entry


def self_test() -> None:
    schema = load_schema(journal_module() / "schema" / "journal-entry.schema.json")
    bad = {
        "entry_id": "0123456789abcdef",
        "project_key": "shippin-tech",
        "session_key": "self-test",
        "agent_id": "main",
        "entry_type": "not-a-real-type",
        "content": "nope",
        "source": "organize",
        "created_at": now(),
    }
    try:
        check_node(schema, bad, "bad")
    except ValueError:
        pass
    else:
        raise SystemExit("invalid journal entry was accepted")
    probe = MODULES / "_probe"
    probe_schema = probe / "schema"
    probe_data = probe / "data"
    try:
        probe_schema.mkdir(parents=True)
        probe_data.mkdir()
        (probe / "SKILL.md").write_text("---\nname: _probe\ndescription: temporary plug proof\n---\n\n# Probe\n", encoding="utf-8")
        (probe_schema / "note.schema.json").write_text(
            json.dumps({"type": "object", "additionalProperties": False, "required": ["ok"], "properties": {"ok": {"type": "boolean"}}}),
            encoding="utf-8",
        )
        (probe / "module.json").write_text(
            json.dumps({"id": "_probe", "json": [{"glob": "data/*.json", "schema": "schema/note.schema.json"}]}),
            encoding="utf-8",
        )
        (probe_data / "note.json").write_text('{"ok": true}\n', encoding="utf-8")
        counts = check_tree()
        if counts.get("modules", 0) < 6 or counts.get("events", 0) < 1:
            raise SystemExit(f"modules were not discovered: {counts}")
    finally:
        if probe.exists():
            import shutil
            shutil.rmtree(probe)
    print(json.dumps({"ok": True, **{k: v for k, v in counts.items() if k != "lines"}}))


def main() -> None:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--self-test", action="store_true")
    mode.add_argument("--journal", action="store_true")
    parser.add_argument("--session", default="shippin-tech")
    parser.add_argument("--type", default="decision_made")
    parser.add_argument("--content", default="")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.check:
        print(json.dumps(check_tree()))
        return
    if not args.content.strip():
        raise SystemExit("journal content is empty")
    print(json.dumps(append_journal(args.session, args.type, args.content)))


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        sys.exit(0)
