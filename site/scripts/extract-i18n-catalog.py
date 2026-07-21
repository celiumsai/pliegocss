#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Extract exact user-visible English strings from the generated SSG output."""

from __future__ import annotations

import json
import re
from html.parser import HTMLParser
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SITE = ROOT / "site" / "target" / "site"
DESTINATION = ROOT / "target" / "audit" / "site-en-visible-text.json"
SKIP = {"code", "path", "pre", "script", "style", "svg"}
ATTRIBUTES = {"alt", "aria-description", "aria-label", "placeholder", "title"}
GENERATED_TEXT_FIELDS = {"label", "message", "suggestion", "summary"}


class VisibleTextParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.stack: list[str] = []
        self.values: list[str] = []

    @property
    def ignored(self) -> bool:
        return any(tag in SKIP for tag in self.stack)

    def handle_starttag(
        self, tag: str, attributes: list[tuple[str, str | None]]
    ) -> None:
        self.stack.append(tag)
        if not self.ignored:
            self._capture_attributes(attributes)

    def handle_startendtag(
        self, tag: str, attributes: list[tuple[str, str | None]]
    ) -> None:
        if not self.ignored:
            self._capture_attributes(attributes)

    def handle_endtag(self, tag: str) -> None:
        if tag in self.stack:
            index = len(self.stack) - 1 - self.stack[::-1].index(tag)
            self.stack = self.stack[:index]

    def handle_data(self, data: str) -> None:
        if self.ignored:
            return
        value = " ".join(data.split())
        if value:
            self.values.append(value)

    def _capture_attributes(
        self, attributes: list[tuple[str, str | None]]
    ) -> None:
        for name, value in attributes:
            if name in ATTRIBUTES and value:
                self.values.append(value)


def is_english_page(path: Path) -> bool:
    relative = path.relative_to(SITE).as_posix()
    return relative != "es.html" and not relative.startswith("es/")


def collect_generated_text(value: object, values: set[str]) -> None:
    """Collect only generated fields that the browser renders as prose."""
    if isinstance(value, list):
        for item in value:
            collect_generated_text(item, values)
        return
    if not isinstance(value, dict):
        return
    for key, item in value.items():
        if key in GENERATED_TEXT_FIELDS and isinstance(item, str):
            values.add(" ".join(item.split()))
        collect_generated_text(item, values)


def collect_client_literals(values: set[str]) -> None:
    """Capture exact literals translated by tr() plus the live operations copy."""
    source = (ROOT / "site" / "client" / "main.js").read_text(encoding="utf-8")
    for match in re.finditer(
        r"""\btr\(\s*["']([^"'\\]*(?:\\.[^"'\\]*)*)["']\s*\)""",
        source,
    ):
        values.add(match.group(1))

    block = re.search(
        r"const opsCopy = (?P<value>\{.*?\n  \});\n  opsViews",
        source,
        flags=re.DOTALL,
    )
    if block:
        for match in re.finditer(
            r'"([^"\\]*(?:\\.[^"\\]*)*)"',
            block.group("value"),
        ):
            values.add(match.group(1))
    for block_name in ("staticItems",):
        block = re.search(
            rf"const {block_name} = (?P<value>\[.*?\n  \])\.map",
            source,
            flags=re.DOTALL,
        )
        if block:
            for match in re.finditer(
                r'"([^"\\]*(?:\\.[^"\\]*)*)"',
                block.group("value"),
            ):
                values.add(match.group(1))


def preserves_product_token(value: str) -> bool:
    return value in {
        "PliegoCSS",
        "PliegoRS",
        "CSS",
        "DTCG",
        "GSAP",
        "HTML",
        "JSON",
        "LSP",
        "Rust",
        "SARIF",
        "SHA-256",
        "TOML",
        "WASM",
    }


def main() -> None:
    values: set[str] = set()
    pages = [path for path in SITE.rglob("*.html") if is_english_page(path)]
    for path in pages:
        parser = VisibleTextParser()
        parser.feed(path.read_text(encoding="utf-8"))
        values.update(parser.values)
    for name in ("catalog.json", "laboratory.json"):
        path = SITE / "assets" / name
        if path.exists():
            collect_generated_text(json.loads(path.read_text(encoding="utf-8")), values)
    collect_client_literals(values)
    values = {
        value
        for value in values
        if re.search(r"[A-Za-z]", value)
        and not value.startswith("/")
        and not re.search(r"[{}[\]]", value)
        and (
            preserves_product_token(value)
            or len(value) >= 4
            and not re.fullmatch(r"[a-z0-9._-]+", value)
        )
    }
    catalog = sorted(values, key=lambda value: (value.lower(), value))
    DESTINATION.parent.mkdir(parents=True, exist_ok=True)
    DESTINATION.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(
        f"Extracted {len(catalog)} strings from {len(pages)} English pages "
        f"into {DESTINATION}."
    )


if __name__ == "__main__":
    main()
