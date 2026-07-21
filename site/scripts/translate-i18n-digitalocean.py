#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Translate the committed website string catalog through DigitalOcean inference.

The script never accepts a token on the command line and never writes one to
disk. Set DIGITALOCEAN_TOKEN only for the process that runs this script.
"""

from __future__ import annotations

import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "target" / "audit" / "site-en-visible-text.json"
DESTINATION = ROOT / "site" / "i18n" / "es.json"
ENDPOINT = "https://inference.do-ai.run/v1/chat/completions"
MODEL = "openai-gpt-5.4"
MAX_BATCH_CHARACTERS = 12_000

SYSTEM_PROMPT = """\
You are the senior Spanish localization editor for PliegoCSS, a Rust and CSS
developer tool. Translate each JSON object value from English to concise,
idiomatic Latin American Spanish.

Rules:
- Return one strict JSON object only, with exactly the same keys.
- Preserve PliegoCSS, PliegoRS, Rust, CSS, HTML, JSON, TOML, DTCG, SARIF, LSP,
  WASM, GSAP, Lenis, Three.js, GitHub, crates.io, SPDX, SHA-256 and API names.
- Preserve code, commands, CLI flags, file paths, URLs, email addresses,
  identifiers, CSS properties, utility tokens, values, hashes and version
  numbers byte-for-byte.
- Preserve interpolation markers such as {name} and punctuation used by code.
- Translate UI labels, prose, accessibility descriptions and legal prose.
- Use "tú" only where the English directly addresses the reader; otherwise use
  clear neutral technical prose.
- Do not add claims, legal obligations, warranties, jurisdictions or promises.
- Keep MEASURED / INHERITED / PENDING / UNCERTAIN consistent as
  MEDIDO / HEREDADO / PENDIENTE / INCIERTO.
"""


def batches(values: list[str]) -> list[list[str]]:
    result: list[list[str]] = []
    current: list[str] = []
    size = 0
    for value in values:
        encoded_size = len(json.dumps(value, ensure_ascii=False))
        if current and size + encoded_size > MAX_BATCH_CHARACTERS:
            result.append(current)
            current = []
            size = 0
        current.append(value)
        size += encoded_size
    if current:
        result.append(current)
    return result


def extract_json_object(value: str) -> dict[str, str]:
    value = value.strip()
    if value.startswith("```"):
        value = re.sub(r"^```(?:json)?\s*", "", value)
        value = re.sub(r"\s*```$", "", value)
    start = value.find("{")
    end = value.rfind("}")
    if start < 0 or end < start:
        raise ValueError("model response did not contain a JSON object")
    parsed = json.loads(value[start : end + 1])
    if not isinstance(parsed, dict) or not all(
        isinstance(key, str) and isinstance(item, str) for key, item in parsed.items()
    ):
        raise ValueError("model response was not a string-to-string object")
    return parsed


def translate(token: str, values: list[str]) -> dict[str, str]:
    request_object = {value: value for value in values}
    body = json.dumps(
        {
            "model": MODEL,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {
                    "role": "user",
                    "content": json.dumps(request_object, ensure_ascii=False),
                },
            ],
            "temperature": 0,
            "max_tokens": 16_000,
            "response_format": {"type": "json_object"},
        },
        ensure_ascii=False,
    ).encode("utf-8")
    request = urllib.request.Request(
        ENDPOINT,
        data=body,
        method="POST",
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(request, timeout=180) as response:
        payload = json.load(response)
    content = payload["choices"][0]["message"]["content"]
    return extract_json_object(content)


def main() -> int:
    token = os.environ.get("DIGITALOCEAN_TOKEN", "").strip()
    if not token:
        print("DIGITALOCEAN_TOKEN is required.", file=sys.stderr)
        return 2
    source_values = json.loads(SOURCE.read_text(encoding="utf-8"))
    existing = (
        json.loads(DESTINATION.read_text(encoding="utf-8"))
        if DESTINATION.exists()
        else {}
    )
    source_set = set(source_values)
    stale = sorted(set(existing) - source_set)
    for value in stale:
        existing.pop(value)
    pending = [value for value in source_values if value not in existing]
    work = batches(pending)
    print(
        f"Translating {len(pending)} new strings in {len(work)} batches "
        f"with {MODEL}; pruning {len(stale)} stale strings."
    )
    for index, batch in enumerate(work, start=1):
        last_error: Exception | None = None
        for attempt in range(1, 4):
            try:
                translated = translate(token, batch)
                expected = set(batch)
                actual = set(translated)
                if expected != actual:
                    missing = sorted(expected - actual)
                    extra = sorted(actual - expected)
                    raise ValueError(
                        f"key mismatch: {len(missing)} missing, {len(extra)} extra"
                    )
                existing.update(translated)
                DESTINATION.write_text(
                    json.dumps(existing, ensure_ascii=False, indent=2, sort_keys=True)
                    + "\n",
                    encoding="utf-8",
                )
                print(f"[{index}/{len(work)}] {len(batch)} strings")
                last_error = None
                break
            except (KeyError, ValueError, urllib.error.URLError) as error:
                last_error = error
                print(
                    f"[{index}/{len(work)}] attempt {attempt} failed: {error}",
                    file=sys.stderr,
                )
                time.sleep(attempt * 2)
        if last_error is not None:
            raise last_error
    print(f"Wrote {len(existing)} translations to {DESTINATION}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
