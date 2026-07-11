#!/usr/bin/env python3
"""Build and strictly validate deterministic source-only graph snapshots."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Sequence


DATASETS = ("longmemeval-s", "locomo")
CANONICAL_EXPECTATIONS = {
    "longmemeval-s": {
        "snapshots": 500,
        "source_items": 500,
        "affected_rows": 76,
        "future_sessions_excluded": 1_475,
        "no_visible_session_rows": 1,
    },
    "locomo": {
        "snapshots": 10,
        "source_items": 10,
    },
}
WORKFLOW_ID = "deterministic-exact-source-replay-v1"
REPLAY_ID = "source-turn-replay-v1"
FORBIDDEN_KEYS = {
    "answer", "answers", "answer_session_ids", "category", "categories",
    "evidence", "evidence_dialog_ids", "evaluator", "evaluation", "gold",
    "gold_label", "gold_labels", "has_answer", "label", "labels", "question",
    "questions", "qa", "qas", "result", "results", "score", "scores",
    "session_summary", "session_summaries", "summaries",
    "observation", "observations", "event", "events", "image", "images",
    "caption", "captions",
}
ENTITY_TYPES = {"person", "user", "assistant", "project", "concept", "tool", "document", "place", "organization", "other"}
DERIVED_TYPES = {"reflection", "user_preference", "assistant_preference", "commitment", "open_loop", "character_signal", "relationship_note", "project_note", "claim", "correction"}
RELATIONS = {"has_observation", "observed_in", "mentions", "involves", "about", "derived_from", "part_of_thread", "supports", "contradicts", "supersedes", "resolves", "creates_open_loop", "fulfills_commitment", "associated_with"}
OBJECT_LISTS = {"entity": "entities", "memory_thread": "threads", "derived_memory": "derived_memories", "memory_link": "links"}


class ValidationError(ValueError):
    pass


@dataclass(frozen=True)
class Turn:
    observation_id: str
    speaker: str
    text: str


@dataclass(frozen=True)
class Session:
    session_id: str
    raw_date: str
    normalized_date: str
    date_key: datetime
    turns: tuple[Turn, ...]


@dataclass(frozen=True)
class Item:
    item_id: str
    cutoff_type: str
    cutoff_value: str
    cutoff_key: datetime | None
    speakers: tuple[str, ...]
    sessions: tuple[Session, ...]


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def _string(value: Any, field: str) -> str:
    _require(isinstance(value, str) and bool(value.strip()), f"{field} must be a non-empty string")
    return value


def _text(value: Any, field: str) -> str:
    _require(isinstance(value, str), f"{field} must be a string")
    return value


def _scan_forbidden(value: Any, path: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            _require(isinstance(key, str), f"{path} contains a non-string key")
            if key.casefold() in FORBIDDEN_KEYS:
                raise ValidationError(f"forbidden key at {path}.{key}")
            _scan_forbidden(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _scan_forbidden(child, f"{path}[{index}]")


def _exact_keys(value: dict[str, Any], allowed: set[str], path: str) -> None:
    extras = sorted(set(value) - allowed)
    _require(not extras, f"{path} contains unsupported keys: {', '.join(extras)}")


def _parse_date(value: Any, field: str) -> tuple[str, datetime]:
    raw = _string(value, field)
    candidate = raw.strip()
    normalized = candidate[:-1] + "+00:00" if candidate.endswith("Z") else candidate
    official_longmem = re.fullmatch(
        r"(\d{4})/(\d{2})/(\d{2}) \([A-Za-z]{3}\) (\d{2}):(\d{2})",
        candidate,
    )
    if official_longmem:
        year, month, day, hour, minute = map(int, official_longmem.groups())
        parsed = datetime(year, month, day, hour, minute, tzinfo=timezone.utc)
    else:
        parsed = None
    if parsed is None:
        try:
            parsed = datetime.fromisoformat(normalized)
        except ValueError:
            for fmt in ("%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d/%m/%Y", "%I:%M %p on %d %B, %Y", "%I:%M %p on %d %b, %Y"):
                try:
                    parsed = datetime.strptime(candidate, fmt)
                    break
                except ValueError:
                    continue
            if parsed is None:
                raise ValidationError(f"{field} is not a supported date/time")
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return raw, parsed.astimezone(timezone.utc)


def _rfc3339(value: datetime) -> str:
    return value.astimezone(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def _load_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ValidationError(f"cannot read source-only JSON: {exc}") from exc


def _root_rows(value: Any, dataset: str) -> list[dict[str, Any]]:
    if isinstance(value, dict):
        allowed = {"rows"} if dataset == "longmemeval-s" else {"samples"}
        _exact_keys(value, allowed, "$")
        value = value[next(iter(allowed))]
    _require(isinstance(value, list), "source root must be an array")
    _require(all(isinstance(row, dict) for row in value), "every source row must be an object")
    return value


def _longmem_items(value: Any) -> list[Item]:
    rows = _root_rows(value, "longmemeval-s")
    items: list[Item] = []
    for row_index, row in enumerate(rows):
        path = f"$[{row_index}]"
        _exact_keys(row, {"question_id", "question_date", "haystack_session_ids", "haystack_dates", "haystack_sessions"}, path)
        item_id = _string(row.get("question_id"), f"{path}.question_id")
        cutoff_raw, cutoff = _parse_date(row.get("question_date"), f"{path}.question_date")
        ids, dates, contents = row.get("haystack_session_ids"), row.get("haystack_dates"), row.get("haystack_sessions")
        _require(isinstance(ids, list) and isinstance(dates, list) and isinstance(contents, list), f"{path} session fields must be arrays")
        _require(len(ids) == len(dates) == len(contents), f"{path} aligned session arrays differ in length")
        sessions: list[Session] = []
        for session_index, (session_id_value, date_value, turns_value) in enumerate(zip(ids, dates, contents)):
            session_path = f"{path}.haystack_sessions[{session_index}]"
            session_id = _string(session_id_value, f"{path}.haystack_session_ids[{session_index}]")
            raw_date, date_key = _parse_date(date_value, f"{path}.haystack_dates[{session_index}]")
            _require(isinstance(turns_value, list), f"{session_path} must be an array")
            turns: list[Turn] = []
            for turn_index, turn in enumerate(turns_value, 1):
                _require(isinstance(turn, dict), f"{session_path}[{turn_index - 1}] must be an object")
                _exact_keys(turn, {"role", "content"}, f"{session_path}[{turn_index - 1}]")
                role = _string(turn.get("role"), f"{session_path}[{turn_index - 1}].role")
                text = _text(turn.get("content"), f"{session_path}[{turn_index - 1}].content")
                turns.append(Turn(f"{session_id}:turn:{turn_index}", role, text))
            sessions.append(Session(session_id, raw_date, _rfc3339(date_key), date_key, tuple(turns)))
        items.append(Item(item_id, "question_date", cutoff_raw, cutoff, tuple(), tuple(sessions)))
    return items


def _locomo_items(value: Any) -> list[Item]:
    rows = _root_rows(value, "locomo")
    items: list[Item] = []
    for row_index, row in enumerate(rows):
        path = f"$[{row_index}]"
        _exact_keys(row, {"sample_id", "speaker_a", "speaker_b", "sessions"}, path)
        item_id = _string(row.get("sample_id"), f"{path}.sample_id")
        speakers = (
            _string(row.get("speaker_a"), f"{path}.speaker_a"),
            _string(row.get("speaker_b"), f"{path}.speaker_b"),
        )
        sessions_value = row.get("sessions")
        _require(isinstance(sessions_value, list) and sessions_value, f"{path}.sessions must be a non-empty array")
        sessions: list[Session] = []
        seen_dialog: set[str] = set()
        for session_index, session in enumerate(sessions_value):
            session_path = f"{path}.sessions[{session_index}]"
            _require(isinstance(session, dict), f"{session_path} must be an object")
            _exact_keys(session, {"session_id", "date", "turns"}, session_path)
            session_id = _string(session.get("session_id"), f"{session_path}.session_id")
            raw_date, date_key = _parse_date(session.get("date"), f"{session_path}.date")
            turns_value = session.get("turns")
            _require(isinstance(turns_value, list), f"{session_path}.turns must be an array")
            turns: list[Turn] = []
            for turn_index, turn in enumerate(turns_value):
                turn_path = f"{session_path}.turns[{turn_index}]"
                _require(isinstance(turn, dict), f"{turn_path} must be an object")
                _exact_keys(turn, {"dia_id", "speaker", "text"}, turn_path)
                dia_id = _string(turn.get("dia_id"), f"{turn_path}.dia_id")
                _require(dia_id not in seen_dialog, f"duplicate LoCoMo dia_id {dia_id}")
                seen_dialog.add(dia_id)
                speaker = _string(turn.get("speaker"), f"{turn_path}.speaker")
                text = _text(turn.get("text"), f"{turn_path}.text")
                turns.append(Turn(dia_id, speaker, text))
            sessions.append(Session(session_id, raw_date, _rfc3339(date_key), date_key, tuple(turns)))
        _require(all(sessions[i].date_key <= sessions[i + 1].date_key for i in range(len(sessions) - 1)), f"{path}.sessions are not chronological")
        items.append(Item(item_id, "final_session", sessions[-1].session_id, None, speakers, tuple(sessions)))
    return items


def _parse_source(value: Any, dataset: str) -> list[Item]:
    _scan_forbidden(value)
    return _longmem_items(value) if dataset == "longmemeval-s" else _locomo_items(value)


def _token(kind: str, *parts: str) -> str:
    digest = hashlib.sha256("\0".join((kind, *parts)).encode("utf-8")).hexdigest()[:20]
    return f"{kind}:{digest}"


def _namespace(dataset: str, item_id: str) -> str:
    prefix = "lme" if dataset == "longmemeval-s" else "locomo"
    return f"{prefix}:{item_id}"


def _visible(item: Item) -> tuple[Session, ...]:
    if item.cutoff_key is None:
        return item.sessions
    return tuple(session for session in item.sessions if session.date_key <= item.cutoff_key)


def _effective_visible(item: Item) -> tuple[Session, ...]:
    winners: dict[str, tuple[int, Session]] = {}
    for index, session in enumerate(_visible(item)):
        winners[session.session_id] = (index, session)
    return tuple(session for _, session in sorted(winners.values(), key=lambda value: value[0]))


def _snapshot(dataset: str, item: Item) -> dict[str, Any]:
    namespace = _namespace(dataset, item.item_id)
    visible = _effective_visible(item)
    speakers = sorted({turn.speaker for session in visible for turn in session.turns} | set(item.speakers))
    entities = []
    entity_ids: dict[str, str] = {}
    for speaker in speakers:
        entity_id = _token("entity", item.item_id, speaker)
        entity_ids[speaker] = entity_id
        entities.append({"external_id": entity_id, "entity_type": "person", "name": speaker, "aliases": [], "canonical_key": f"speaker:{hashlib.sha256(speaker.encode('utf-8')).hexdigest()}", "summary": None})
    threads = []
    memories = []
    links = []
    for session in visible:
        thread_id = _token("thread", item.item_id, session.session_id)
        threads.append({"external_id": thread_id, "title": f"Session {session.session_id}", "summary": f"Source session {session.session_id}", "status": "active", "last_touched_at": session.normalized_date, "salience_score": 0.5, "canonical_key": f"session:{hashlib.sha256(session.session_id.encode('utf-8')).hexdigest()}"})
        for turn in session.turns:
            if not turn.text.strip():
                continue
            memory_id = _token("memory", item.item_id, turn.observation_id)
            memories.append({"external_id": memory_id, "derived_type": "reflection", "text": turn.text, "source_episode_external_ids": [session.session_id], "source_observation_external_ids": [turn.observation_id], "thread_external_ids": [thread_id], "entity_external_ids": [entity_ids[turn.speaker]], "confidence": 1.0, "salience_score": 0.5, "stability": "medium", "is_current": True, "supersedes_external_ids": [], "metadata": {"producer": WORKFLOW_ID}})
            for relation, object_type, target in (("part_of_thread", "memory_thread", thread_id), ("about", "entity", entity_ids[turn.speaker])):
                links.append({"external_id": _token("link", memory_id, relation, target), "from": {"object_type": "derived_memory", "external_id": memory_id}, "relation": relation, "to": {"object_type": object_type, "external_id": target}, "confidence": 1.0, "rationale": None})
    graph = {"namespace": namespace, "entities": entities, "threads": threads, "derived_memories": memories, "links": links}
    return {"snapshot_id": f"{namespace}@{item.cutoff_type}", "namespace": namespace, "dataset_item_id": item.item_id, "cutoff": {"type": item.cutoff_type, "value": item.cutoff_value}, "graph": graph}


def _canonical_line(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _atomic_write(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp_name, path)
    except BaseException:
        try:
            os.unlink(temp_name)
        except FileNotFoundError:
            pass
        raise


def _counts(snapshots: Sequence[dict[str, Any]]) -> dict[str, int]:
    result = {"snapshots": len(snapshots), "entities": 0, "threads": 0, "derived_memories": 0, "links": 0}
    for snapshot in snapshots:
        graph = snapshot["graph"]
        for key in ("entities", "threads", "derived_memories", "links"):
            result[key] += len(graph[key])
    return result


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    try:
        with path.open("r", encoding="utf-8", newline="") as handle:
            for line_number, line in enumerate(handle, 1):
                _require(line.endswith("\n"), f"artifact line {line_number} is not newline terminated")
                _require("\u2028" not in line and "\u2029" not in line, f"artifact line {line_number} contains an unescaped Unicode line separator")
                try:
                    value = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise ValidationError(f"invalid artifact JSONL line {line_number}") from exc
                _require(isinstance(value, dict), f"artifact line {line_number} must be an object")
                rows.append(value)
    except (OSError, UnicodeError) as exc:
        raise ValidationError(f"cannot read artifact JSONL: {exc}") from exc
    return rows


def _validate_snapshot(snapshot: dict[str, Any], dataset: str, item: Item) -> None:
    _scan_forbidden(snapshot)
    _exact_keys(snapshot, {"snapshot_id", "namespace", "dataset_item_id", "cutoff", "graph"}, "snapshot")
    expected_namespace = _namespace(dataset, item.item_id)
    _require(snapshot.get("dataset_item_id") == item.item_id, "snapshot dataset_item_id mismatch")
    _require(snapshot.get("namespace") == expected_namespace, "snapshot namespace mismatch")
    _require(snapshot.get("snapshot_id") == f"{expected_namespace}@{item.cutoff_type}", "snapshot_id mismatch")
    cutoff = snapshot.get("cutoff")
    _require(isinstance(cutoff, dict), "snapshot cutoff must be an object")
    _exact_keys(cutoff, {"type", "value"}, "snapshot.cutoff")
    _require(cutoff == {"type": item.cutoff_type, "value": item.cutoff_value}, "snapshot cutoff mismatch")
    graph = snapshot.get("graph")
    _require(isinstance(graph, dict), "snapshot graph must be an object")
    _exact_keys(graph, {"namespace", "entities", "threads", "derived_memories", "links"}, "snapshot.graph")
    _require(graph.get("namespace") == expected_namespace, "graph namespace mismatch")
    for key in ("entities", "threads", "derived_memories", "links"):
        _require(isinstance(graph.get(key), list), f"graph.{key} must be an array")
    typed_ids: set[tuple[str, str]] = set()
    endpoints: set[tuple[str, str]] = set()
    for object_type, list_name in OBJECT_LISTS.items():
        for obj in graph[list_name]:
            _require(isinstance(obj, dict), f"graph.{list_name} entry must be an object")
            external_id = _string(obj.get("external_id"), f"graph.{list_name}.external_id")
            _require((object_type, external_id) not in typed_ids, f"duplicate typed ID {object_type}:{external_id}")
            typed_ids.add((object_type, external_id))
            if object_type != "memory_link":
                endpoints.add((object_type, external_id))
    visible = _effective_visible(item)
    episodes = {session.session_id: session for session in visible}
    observations = {turn.observation_id: (session.session_id, turn.text) for session in visible for turn in session.turns}
    for entity in graph["entities"]:
        _require(entity.get("entity_type") in ENTITY_TYPES, "invalid entity_type enum")
    thread_ids = {obj["external_id"] for obj in graph["threads"]}
    entity_ids = {obj["external_id"] for obj in graph["entities"]}
    memory_ids = {obj["external_id"] for obj in graph["derived_memories"]}
    for memory in graph["derived_memories"]:
        _require(memory.get("derived_type") in DERIVED_TYPES, "invalid derived_type enum")
        _require(memory.get("stability") in {"low", "medium", "high"}, "invalid stability enum")
        source_episodes = memory.get("source_episode_external_ids")
        source_observations = memory.get("source_observation_external_ids")
        _require(isinstance(source_episodes, list) and isinstance(source_observations, list) and (source_episodes or source_observations), "derived memory has no source provenance")
        _require(all(source in episodes for source in source_episodes), "unresolved source episode provenance")
        _require(all(source in observations for source in source_observations), "unresolved source observation provenance")
        _require(len(source_observations) == 1, "exact-source derived memory must cite one observation")
        cited_episode, cited_text = observations[source_observations[0]]
        _require(cited_episode in source_episodes, "source episode/observation provenance mismatch")
        _require(memory.get("text") == cited_text, "derived text is not exactly equal to cited visible source")
        _require(set(memory.get("thread_external_ids", [])) <= thread_ids, "unresolved derived-memory thread reference")
        _require(set(memory.get("entity_external_ids", [])) <= entity_ids, "unresolved derived-memory entity reference")
        _require(set(memory.get("supersedes_external_ids", [])) <= memory_ids, "unresolved supersedes reference")
    for link in graph["links"]:
        _require(link.get("relation") in RELATIONS, "invalid relation enum")
        for side in ("from", "to"):
            endpoint = link.get(side)
            _require(isinstance(endpoint, dict), f"link.{side} must be an object")
            _exact_keys(endpoint, {"object_type", "external_id"}, f"link.{side}")
            pair = (endpoint.get("object_type"), endpoint.get("external_id"))
            _require(pair in endpoints, f"unresolved graph endpoint {pair[0]}:{pair[1]}")
    _require(snapshot == _snapshot(dataset, item), "snapshot differs from deterministic source replay")


def _expected_manifest_counts(dataset: str, snapshots: Sequence[dict[str, Any]], items: Sequence[Item]) -> dict[str, int]:
    affected = sum(1 for item in items if len(_visible(item)) != len(item.sessions)) if dataset == "longmemeval-s" else 0
    future = sum(len(item.sessions) - len(_visible(item)) for item in items) if dataset == "longmemeval-s" else 0
    no_visible = sum(not _visible(item) for item in items) if dataset == "longmemeval-s" else 0
    return {**_counts(snapshots), "source_items": len(items), "affected_rows": affected, "future_sessions_excluded": future, "no_visible_session_rows": no_visible}


def _expected_manifest(dataset: str, source: Path, artifact: Path, snapshots: Sequence[dict[str, Any]], items: Sequence[Item]) -> dict[str, Any]:
    return {"schema_version": 1, "dataset": dataset, "replay_id": REPLAY_ID, "cutoff_policy": "question_date_inclusive" if dataset == "longmemeval-s" else "final_session", "workflow_id": WORKFLOW_ID, "source": {"path": str(source), "sha256": _sha256(source)}, "artifact": {"path": str(artifact), "sha256": _sha256(artifact)}, "counts": _expected_manifest_counts(dataset, snapshots, items)}


def _report(dataset: str, counts: dict[str, int], findings: int) -> bytes:
    lines = ["# Snapshot validation report", "", f"- dataset: {dataset}", f"- findings: {findings}"]
    lines.extend(f"- {key}: {counts[key]}" for key in sorted(counts))
    return ("\n".join(lines) + "\n").encode("utf-8")


def _validate_canonical_counts(dataset: str, counts: dict[str, int]) -> None:
    for key, expected in CANONICAL_EXPECTATIONS[dataset].items():
        _require(counts.get(key) == expected, f"canonical {dataset} {key} must be {expected}, got {counts.get(key)}")


def generate(dataset: str, source: Path, artifact: Path, manifest: Path, report: Path, *, enforce_canonical: bool = True) -> None:
    items = _parse_source(_load_json(source), dataset)
    _require(len({item.item_id for item in items}) == len(items), "duplicate source item ID")
    snapshots = [_snapshot(dataset, item) for item in sorted(items, key=lambda value: value.item_id)]
    source_counts = _expected_manifest_counts(dataset, snapshots, items)
    if enforce_canonical:
        _validate_canonical_counts(dataset, source_counts)
    _atomic_write(artifact, b"".join(_canonical_line(snapshot) for snapshot in snapshots))
    for snapshot, item in zip(snapshots, sorted(items, key=lambda value: value.item_id)):
        _validate_snapshot(snapshot, dataset, item)
    manifest_value = _expected_manifest(dataset, source, artifact, snapshots, items)
    _atomic_write(manifest, json.dumps(manifest_value, ensure_ascii=True, sort_keys=True, indent=2).encode("utf-8") + b"\n")
    _atomic_write(report, _report(dataset, manifest_value["counts"], 0))


def validate(dataset: str, source: Path, artifact: Path, manifest: Path, report: Path | None = None, *, enforce_canonical: bool = True) -> None:
    items = _parse_source(_load_json(source), dataset)
    _require(len({item.item_id for item in items}) == len(items), "duplicate source item ID")
    by_id = {item.item_id: item for item in items}
    snapshots = _read_jsonl(artifact)
    if enforce_canonical:
        _validate_canonical_counts(dataset, _expected_manifest_counts(dataset, snapshots, items))
    _require([row.get("dataset_item_id") for row in snapshots] == sorted(by_id), "artifact snapshot ordering or coverage mismatch")
    for snapshot in snapshots:
        item_id = snapshot.get("dataset_item_id")
        _require(item_id in by_id, "snapshot references unknown source item")
        _validate_snapshot(snapshot, dataset, by_id[item_id])
    expected = _expected_manifest(dataset, source, artifact, snapshots, items)
    actual = _load_json(manifest)
    _scan_forbidden(actual)
    _require(actual == expected, "manifest identifiers, paths, hashes, or counts mismatch")
    if report is not None:
        try:
            actual_report = report.read_bytes()
        except OSError as exc:
            raise ValidationError(f"cannot read validation report: {exc}") from exc
        _require(actual_report == _report(dataset, expected["counts"], 0), "validation report content mismatch")


def self_test() -> None:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        lme_source = root / "lme.json"
        lme = [
            {"question_id": "q-unicode", "question_date": "2023/05/31 (Wed) 00:00", "haystack_session_ids": ["s1", "s2"], "haystack_dates": ["2023/05/30 (Tue) 23:40", "2023/06/01 (Thu) 00:00"], "haystack_sessions": [[{"role": "user", "content": ""}, {"role": "assistant", "content": " \t "}, {"role": "user", "content": "alpha\u2028βeta"}], [{"role": "assistant", "content": "future"}]]},
            {"question_id": "z-duplicate", "question_date": "2024-01-04", "haystack_session_ids": ["dup", "middle", "dup"], "haystack_dates": ["2024-01-01", "2024-01-02", "2024-01-03"], "haystack_sessions": [[{"role": "user", "content": "losing payload"}], [{"role": "assistant", "content": "middle payload"}], [{"role": "user", "content": "winning payload"}]]},
        ]
        _atomic_write(lme_source, json.dumps(lme, ensure_ascii=False).encode("utf-8"))
        artifact, manifest, report = root / "lme.jsonl", root / "lme.manifest.json", root / "lme.report.md"
        generate("longmemeval-s", lme_source, artifact, manifest, report, enforce_canonical=False)
        first = artifact.read_bytes()
        validate("longmemeval-s", lme_source, artifact, manifest, report, enforce_canonical=False)
        original_report = report.read_bytes()
        _atomic_write(report, original_report + b"stale\n")
        stale_report = report.read_bytes()
        try:
            validate("longmemeval-s", lme_source, artifact, manifest, report, enforce_canonical=False)
        except ValidationError:
            pass
        else:
            raise AssertionError("altered validation report was accepted")
        _require(report.read_bytes() == stale_report, "validation rewrote an altered report")
        _atomic_write(report, original_report)
        generated = _read_jsonl(artifact)[0]
        _require(generated["namespace"] == "lme:q-unicode" and generated["snapshot_id"] == "lme:q-unicode@question_date", "LongMemEval runtime namespace mismatch")
        lme_memories = generated["graph"]["derived_memories"]
        _require(len(lme_memories) == 1 and lme_memories[0]["text"] == "alpha\u2028βeta", "blank LongMemEval turns were not skipped or Unicode was not preserved")
        _require(len(generated["graph"]["links"]) == 2, "blank LongMemEval turns emitted links")
        _require(generated["graph"]["threads"][0]["last_touched_at"] == "2023-05-30T23:40:00Z", "official LongMemEval date was not normalized to RFC3339")
        counts = _load_json(manifest)["counts"]
        _require(counts["affected_rows"] == 1 and counts["future_sessions_excluded"] == 1, "future exclusion totals failed")
        duplicate_snapshot = _read_jsonl(artifact)[1]
        _require([thread["title"] for thread in duplicate_snapshot["graph"]["threads"]] == ["Session middle", "Session dup"], "effective duplicate-session order is not based on winning positions")
        duplicate_memories = duplicate_snapshot["graph"]["derived_memories"]
        _require([memory["text"] for memory in duplicate_memories] == ["middle payload", "winning payload"], "last visible duplicate session did not win")
        _require(duplicate_memories[1]["source_episode_external_ids"] == ["dup"] and duplicate_memories[1]["source_observation_external_ids"] == ["dup:turn:1"], "winning duplicate provenance mismatch")
        duplicate_typed_ids = [(kind, obj["external_id"]) for kind, objects in (("thread", duplicate_snapshot["graph"]["threads"]), ("memory", duplicate_memories), ("link", duplicate_snapshot["graph"]["links"])) for obj in objects]
        _require(len(duplicate_typed_ids) == len(set(duplicate_typed_ids)), "effective duplicate sessions produced duplicate typed IDs")
        generate("longmemeval-s", lme_source, artifact, manifest, report, enforce_canonical=False)
        _require(first == artifact.read_bytes(), "deterministic rerun changed artifact bytes")
        rejected_artifact = root / "canonical-rejected.jsonl"
        try:
            generate("longmemeval-s", lme_source, rejected_artifact, root / "canonical-rejected.manifest.json", root / "canonical-rejected.report.md")
        except ValidationError:
            pass
        else:
            raise AssertionError("canonical generation accepted truncated LongMemEval source")
        _require(not rejected_artifact.exists(), "canonical generation wrote an artifact before rejecting truncated source")
        try:
            validate("longmemeval-s", lme_source, artifact, manifest)
        except ValidationError:
            pass
        else:
            raise AssertionError("canonical validation accepted self-consistent truncated LongMemEval files")
        malformed = _read_jsonl(artifact)
        malformed[0]["graph"]["derived_memories"][0]["source_observation_external_ids"] = ["missing"]
        _atomic_write(artifact, _canonical_line(malformed[0]))
        bad_manifest = _expected_manifest("longmemeval-s", lme_source, artifact, malformed, _parse_source(lme, "longmemeval-s"))
        _atomic_write(manifest, json.dumps(bad_manifest).encode("utf-8"))
        try:
            validate("longmemeval-s", lme_source, artifact, manifest, enforce_canonical=False)
        except ValidationError:
            pass
        else:
            raise AssertionError("malformed provenance was accepted")
        locomo_source = root / "locomo.json"
        # Exact source-only row schema emitted by build_source_only.py.
        locomo = [{"sample_id": "p1", "speaker_a": "A", "speaker_b": "B", "sessions": [{"session_id": "session_1", "date": "2024-01-01", "turns": [{"dia_id": "D1:1", "speaker": "A", "text": ""}, {"dia_id": "D1:2", "speaker": "B", "text": "  "}]}, {"session_id": "session_2", "date": "2024-01-02", "turns": [{"dia_id": "D2:1", "speaker": "B", "text": "世界\u2029exact"}]}]}]
        _atomic_write(locomo_source, json.dumps(locomo).encode("utf-8"))
        la, lm, lr = root / "locomo.jsonl", root / "locomo.manifest.json", root / "locomo.report.md"
        generate("locomo", locomo_source, la, lm, lr, enforce_canonical=False)
        validate("locomo", locomo_source, la, lm, lr, enforce_canonical=False)
        try:
            validate("locomo", locomo_source, la, lm)
        except ValidationError:
            pass
        else:
            raise AssertionError("canonical validation accepted self-consistent truncated LoCoMo files")
        try:
            generate("locomo", locomo_source, root / "locomo-rejected.jsonl", root / "locomo-rejected.manifest.json", root / "locomo-rejected.report.md")
        except ValidationError:
            pass
        else:
            raise AssertionError("canonical generation accepted truncated LoCoMo source")
        locomo_snapshot = _read_jsonl(la)[0]
        _require(locomo_snapshot["namespace"] == "locomo:p1" and locomo_snapshot["snapshot_id"] == "locomo:p1@final_session", "LoCoMo runtime namespace mismatch")
        _require(locomo_snapshot["cutoff"] == {"type": "final_session", "value": "session_2"}, "LoCoMo final cutoff failed")
        locomo_memories = locomo_snapshot["graph"]["derived_memories"]
        _require(len(locomo_memories) == 1 and locomo_memories[0]["text"] == "世界\u2029exact", "blank LoCoMo turns were not skipped or Unicode was not preserved")
        _require(len(locomo_snapshot["graph"]["links"]) == 2, "blank LoCoMo turns emitted links")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in ("generate", "validate"):
        child = subparsers.add_parser(command)
        child.add_argument("dataset", choices=DATASETS)
        child.add_argument("--source", type=Path, required=True, help="source-only JSON path")
        child.add_argument("--artifact", type=Path, required=True)
        child.add_argument("--manifest", type=Path, required=True)
        child.add_argument("--report", type=Path, required=command == "generate")
    subparsers.add_parser("self-test")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "self-test":
            self_test()
        elif args.command == "generate":
            generate(args.dataset, args.source, args.artifact, args.manifest, args.report)
        else:
            validate(args.dataset, args.source, args.artifact, args.manifest, args.report)
    except (ValidationError, OSError) as exc:
        print(f"error: {exc}", file=__import__("sys").stderr)
        return 2
    print(json.dumps({"command": args.command, "status": "ok"}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
