#!/usr/bin/env python3
"""Build deterministic, gold-free enrichment inputs from raw benchmarks."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Callable, NoReturn


JsonValue = None | bool | int | float | str | list["JsonValue"] | dict[str, "JsonValue"]

FORBIDDEN_KEY_TOKENS = frozenset(
    {
        "qa",
        "question",
        "answer",
        "evidence",
        "gold",
        "label",
        "category",
        "evaluator",
        "result",
        "retrieval",
        "prediction",
    }
)
ALLOWED_QUESTION_KEYS = frozenset({"question_id", "question_date"})
LONGMEM_TOP_KEYS = (
    "question_id",
    "question_date",
    "haystack_session_ids",
    "haystack_dates",
    "haystack_sessions",
)
LONGMEM_TURN_KEYS = ("role", "content")
LOCOMO_TOP_KEYS = ("sample_id", "speaker_a", "speaker_b", "sessions")
LOCOMO_SESSION_KEYS = ("session_id", "date", "turns")
LOCOMO_TURN_KEYS = ("dia_id", "speaker", "text")
SESSION_KEY = re.compile(r"^session_(\d+)$")


class SanitizerError(Exception):
    """A safe-to-report sanitizer failure without source values."""


def fail(message: str) -> NoReturn:
    raise SanitizerError(message)


def require_object(value: JsonValue, context: str) -> dict[str, JsonValue]:
    if not isinstance(value, dict):
        fail(f"{context} must be an object")
    return value


def require_list(value: JsonValue, context: str) -> list[JsonValue]:
    if not isinstance(value, list):
        fail(f"{context} must be an array")
    return value


def require_string(value: JsonValue, context: str) -> str:
    if not isinstance(value, str):
        fail(f"{context} must be a string")
    return value


def require_rows(raw: JsonValue, wrapper_keys: tuple[str, ...]) -> list[JsonValue]:
    if isinstance(raw, list):
        return raw
    root = require_object(raw, "input root")
    for key in wrapper_keys:
        if key in root:
            return require_list(root[key], "input rows")
    fail("input root does not contain a supported row array")


def sanitize_longmemeval(raw: JsonValue) -> list[JsonValue]:
    output: list[JsonValue] = []
    for row_index, raw_row in enumerate(require_rows(raw, ("data", "instances", "questions"))):
        row = require_object(raw_row, f"row {row_index}")
        session_ids = require_list(row.get("haystack_session_ids"), f"row {row_index} session IDs")
        dates = require_list(row.get("haystack_dates"), f"row {row_index} dates")
        sessions = require_list(row.get("haystack_sessions"), f"row {row_index} sessions")
        if not (len(session_ids) == len(dates) == len(sessions)):
            fail(f"row {row_index} haystack arrays are not aligned")

        clean_sessions: list[JsonValue] = []
        for session_index, raw_session in enumerate(sessions):
            turns = require_list(raw_session, f"row {row_index} session {session_index}")
            clean_turns: list[JsonValue] = []
            for turn_index, raw_turn in enumerate(turns):
                turn = require_object(raw_turn, f"row {row_index} session {session_index} turn {turn_index}")
                clean_turns.append(
                    {
                        "role": require_string(turn.get("role"), "LongMemEval turn role"),
                        "content": require_string(turn.get("content"), "LongMemEval turn content"),
                    }
                )
            clean_sessions.append(clean_turns)

        output.append(
            {
                "question_id": require_string(row.get("question_id"), "LongMemEval question_id"),
                "question_date": require_string(row.get("question_date"), "LongMemEval question_date"),
                "haystack_session_ids": [
                    require_string(item, "LongMemEval session ID") for item in session_ids
                ],
                "haystack_dates": [require_string(item, "LongMemEval date") for item in dates],
                "haystack_sessions": clean_sessions,
            }
        )
    validate_source(output, "longmemeval-s")
    return output


def sanitize_locomo(raw: JsonValue) -> list[JsonValue]:
    output: list[JsonValue] = []
    for row_index, raw_row in enumerate(require_rows(raw, ("data", "samples", "items"))):
        row = require_object(raw_row, f"row {row_index}")
        conversation = require_object(
            row.get("conversation", row.get("conversations")), f"row {row_index} conversation"
        )
        numbered_sessions: list[tuple[int, str, list[JsonValue]]] = []
        for key, value in conversation.items():
            match = SESSION_KEY.fullmatch(key)
            if match:
                turns = require_list(value, f"row {row_index} {key}")
                numbered_sessions.append((int(match.group(1)), key, turns))
        numbered_sessions.sort(key=lambda item: item[0])

        clean_sessions: list[JsonValue] = []
        for _, session_id, turns in numbered_sessions:
            clean_turns: list[JsonValue] = []
            for turn_index, raw_turn in enumerate(turns):
                turn = require_object(raw_turn, f"row {row_index} {session_id} turn {turn_index}")
                clean_turns.append(
                    {
                        "dia_id": require_string(turn.get("dia_id"), "LoCoMo turn dia_id"),
                        "speaker": require_string(turn.get("speaker"), "LoCoMo turn speaker"),
                        "text": require_string(turn.get("text"), "LoCoMo turn text"),
                    }
                )
            clean_sessions.append(
                {
                    "session_id": session_id,
                    "date": require_string(
                        conversation.get(f"{session_id}_date_time"), "LoCoMo session date"
                    ),
                    "turns": clean_turns,
                }
            )

        output.append(
            {
                "sample_id": require_string(row.get("sample_id"), "LoCoMo sample_id"),
                "speaker_a": require_string(conversation.get("speaker_a"), "LoCoMo speaker_a"),
                "speaker_b": require_string(conversation.get("speaker_b"), "LoCoMo speaker_b"),
                "sessions": clean_sessions,
            }
        )
    validate_source(output, "locomo")
    return output


def validate_forbidden_keys(value: JsonValue) -> None:
    if isinstance(value, dict):
        for key, nested in value.items():
            folded = key.casefold()
            tokens = re.findall(r"[a-z0-9]+", folded)
            normalized_tokens = {token[:-1] if token.endswith("s") else token for token in tokens}
            if folded not in ALLOWED_QUESTION_KEYS and normalized_tokens & FORBIDDEN_KEY_TOKENS:
                fail("source-only output contains a forbidden field")
            validate_forbidden_keys(nested)
    elif isinstance(value, list):
        for nested in value:
            validate_forbidden_keys(nested)


def require_exact_keys(value: dict[str, JsonValue], allowed: tuple[str, ...], context: str) -> None:
    if tuple(value.keys()) != allowed:
        fail(f"{context} does not match its output allowlist")


def validate_source(value: JsonValue, dataset: str) -> None:
    validate_forbidden_keys(value)
    rows = require_list(value, "source-only root")
    for row_index, raw_row in enumerate(rows):
        row = require_object(raw_row, f"source row {row_index}")
        if dataset == "longmemeval-s":
            require_exact_keys(row, LONGMEM_TOP_KEYS, "LongMemEval row")
            session_ids = require_list(row["haystack_session_ids"], "LongMemEval session IDs")
            dates = require_list(row["haystack_dates"], "LongMemEval dates")
            sessions = require_list(row["haystack_sessions"], "LongMemEval sessions")
            if not (len(session_ids) == len(dates) == len(sessions)):
                fail("LongMemEval source arrays are not aligned")
            for session in sessions:
                for raw_turn in require_list(session, "LongMemEval session"):
                    turn = require_object(raw_turn, "LongMemEval turn")
                    require_exact_keys(turn, LONGMEM_TURN_KEYS, "LongMemEval turn")
                    require_string(turn["role"], "LongMemEval turn role")
                    require_string(turn["content"], "LongMemEval turn content")
        elif dataset == "locomo":
            require_exact_keys(row, LOCOMO_TOP_KEYS, "LoCoMo row")
            for raw_session in require_list(row["sessions"], "LoCoMo sessions"):
                session = require_object(raw_session, "LoCoMo session")
                require_exact_keys(session, LOCOMO_SESSION_KEYS, "LoCoMo session")
                for raw_turn in require_list(session["turns"], "LoCoMo turns"):
                    turn = require_object(raw_turn, "LoCoMo turn")
                    require_exact_keys(turn, LOCOMO_TURN_KEYS, "LoCoMo turn")
                    for key in LOCOMO_TURN_KEYS:
                        require_string(turn[key], f"LoCoMo turn {key}")
        else:
            fail("unsupported dataset mode")


def serialize_json(value: JsonValue) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2, separators=(",", ": ")) + "\n").encode(
        "utf-8"
    )


def atomic_write(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb", dir=path.parent, prefix=f".{path.name}.", suffix=".tmp", delete=False
        ) as temporary:
            temporary_name = temporary.name
            temporary.write(content)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, path)
    except OSError as error:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except OSError:
                pass
        raise SanitizerError("atomic output write failed") from error


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def success_record(
    dataset: str,
    rows: list[JsonValue],
    input_path: Path,
    input_bytes: bytes,
    output_path: Path,
    output_bytes: bytes,
) -> str:
    return (
        f"dataset={dataset} rows={len(rows)} input_path={input_path} "
        f"input_sha256={sha256(input_bytes)} output_path={output_path} "
        f"output_sha256={sha256(output_bytes)}"
    )


def read_json(path: Path) -> tuple[JsonValue, bytes]:
    try:
        raw_bytes = path.read_bytes()
        return json.loads(raw_bytes.decode("utf-8")), raw_bytes
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SanitizerError("input read or JSON decoding failed") from error


class SanitizerSelfTests(unittest.TestCase):
    def test_longmemeval_removes_nested_gold_and_preserves_unicode(self) -> None:
        text = "exact café 東京\u2028line\u2029end"
        raw: JsonValue = [{
            "question_id": "q1", "question_date": "2024/01/02 (Tue) 03:04",
            "question": "secret", "answer": "secret", "gold": {"label": "secret"},
            "haystack_session_ids": ["s1"], "haystack_dates": ["2024/01/01"],
            "haystack_sessions": [[{
                "role": "user", "content": text, "has_answer": True,
                "metadata": {"evidence": "secret"},
            }]],
        }]
        clean = sanitize_longmemeval(raw)
        self.assertEqual(clean[0]["haystack_sessions"][0][0]["content"], text)  # type: ignore[index]
        self.assertNotIn("secret", json.dumps(clean, ensure_ascii=False))
        self.assertIn(text.encode("utf-8"), serialize_json(clean))

    def test_locomo_orders_sessions_and_removes_nested_gold(self) -> None:
        text = "naïve 😀\u2028kept"
        raw: JsonValue = [{
            "sample_id": "p1", "qa": [{"answer": "secret"}],
            "conversation": {
                "speaker_a": "A", "speaker_b": "B",
                "session_10_date_time": "later", "session_10": [{
                    "dia_id": "d10", "speaker": "A", "text": "later", "gold": "secret"
                }],
                "session_2_date_time": "earlier", "session_2": [{
                    "dia_id": "d2", "speaker": "B", "text": text,
                    "metadata": {"prediction": "secret"},
                }],
            },
        }]
        clean = sanitize_locomo(raw)
        sessions = clean[0]["sessions"]  # type: ignore[index]
        self.assertEqual([item["session_id"] for item in sessions], ["session_2", "session_10"])  # type: ignore[index]
        self.assertEqual(sessions[0]["turns"][0]["text"], text)  # type: ignore[index]
        self.assertNotIn("secret", json.dumps(clean, ensure_ascii=False))

    def test_locomo_rejects_non_array_session(self) -> None:
        raw: JsonValue = [{
            "sample_id": "p1",
            "conversation": {
                "speaker_a": "A",
                "speaker_b": "B",
                "session_1_date_time": "date",
                "session_1": {"turns": []},
            },
        }]

        with self.assertRaisesRegex(SanitizerError, r"row 0 session_1 must be an array"):
            sanitize_locomo(raw)

    def test_recursive_validator_rejects_forbidden_fields(self) -> None:
        with self.assertRaises(SanitizerError):
            validate_forbidden_keys({"safe": [{"nested": {"has_answer": False}}]})
        with self.assertRaises(SanitizerError):
            validate_forbidden_keys({"safe": [{"nested": {"gold_answer": "secret"}}]})
        with self.assertRaises(SanitizerError):
            validate_forbidden_keys({"safe": {"retrieval_results": []}})
        validate_forbidden_keys({"question_id": "q1", "question_date": "date"})

    def test_success_record_identifies_dataset_mode(self) -> None:
        record = success_record(
            "locomo",
            [],
            Path("input.json"),
            b"input",
            Path("output.json"),
            b"output",
        )
        self.assertTrue(record.startswith("dataset=locomo rows=0 "))


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SanitizerSelfTests)
    result = unittest.TextTestRunner(stream=sys.stderr, verbosity=0).run(suite)
    print(f"tests_run={result.testsRun} failures={len(result.failures)} errors={len(result.errors)}")
    return 0 if result.wasSuccessful() else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    sanitize = subparsers.add_parser("sanitize", help="build a source-only JSON file")
    sanitize.add_argument("--dataset", choices=("longmemeval-s", "locomo"), required=True)
    sanitize.add_argument("--input", type=Path, required=True)
    sanitize.add_argument("--output", type=Path, required=True)
    subparsers.add_parser("self-test", help="run synthetic sanitizer tests")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "self-test":
        return run_self_tests()

    sanitizers: dict[str, Callable[[JsonValue], list[JsonValue]]] = {
        "longmemeval-s": sanitize_longmemeval,
        "locomo": sanitize_locomo,
    }
    try:
        raw, input_bytes = read_json(args.input)
        clean = sanitizers[args.dataset](raw)
        output_bytes = serialize_json(clean)
        atomic_write(args.output, output_bytes)
    except SanitizerError as error:
        print(f"error={error} input_path={args.input} output_path={args.output}", file=sys.stderr)
        return 1

    print(
        success_record(
            args.dataset,
            clean,
            args.input,
            input_bytes,
            args.output,
            output_bytes,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
