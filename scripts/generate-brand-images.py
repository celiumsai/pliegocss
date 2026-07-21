# SPDX-License-Identifier: Apache-2.0
"""Generate the canonical PliegoCSS image collection through DigitalOcean.

The script intentionally accepts the OAuth token only through DIGITALOCEAN_TOKEN,
never from a command-line argument. It writes no token-bearing response data.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
PROMPT_DIR = ROOT / "brand" / "image-prompts"
IMAGE_DIR = ROOT / "brand" / "images"
ENDPOINT = "https://inference.do-ai.run/v1/images/generations"
MODEL_ID = "openai-gpt-image-2"
MANIFEST_MODEL = "openai-gpt-image-2"


@dataclass(frozen=True)
class AssetSpec:
    role: str
    master: str
    prompt_file: str
    size: str
    subject: str | None = None


ASSETS = (
    AssetSpec(
        "cascade-chamber",
        "cascade-chamber.png",
        "01-cascade-chamber.md",
        "1536x1024",
    ),
    AssetSpec(
        "semantic-fold",
        "semantic-fold.png",
        "02-semantic-fold.md",
        "1536x1024",
    ),
    AssetSpec(
        "evidence-archive",
        "evidence-archive.png",
        "03-evidence-archive.md",
        "1536x1024",
    ),
    AssetSpec(
        "material-study-carbon",
        "material-study-carbon.png",
        "04-material-studies.md",
        "1024x1024",
        "Carbon engineered paper with one exact square fold and a cobalt edge.",
    ),
    AssetSpec(
        "material-study-paper",
        "material-study-paper.png",
        "04-material-studies.md",
        "1024x1024",
        "Paper-white drafting film pierced by a single cyan lineage filament.",
    ),
    AssetSpec(
        "material-study-cobalt",
        "material-study-cobalt.png",
        "04-material-studies.md",
        "1024x1024",
        "A cobalt anodized plate aligned over a carbon semantic grid.",
    ),
)


def prompt_block(path: Path) -> str:
    source = path.read_text(encoding="utf-8")
    match = re.search(r"```text\n(.*?)\n```", source, re.DOTALL)
    if match is None:
        raise ValueError(f"no canonical text prompt found in {path}")
    return match.group(1).strip()


def final_prompt(spec: AssetSpec) -> str:
    prompt = prompt_block(PROMPT_DIR / spec.prompt_file)
    if spec.subject is not None:
        prompt = (
            f"Primary request: {spec.subject}\n"
            "Treat this as one independent image, not a contact sheet or a set. "
            "Render exactly one decisive material composition.\n"
            f"{prompt}"
        )
    return (
        f"{prompt}\n"
        "Brand fidelity: follow the PliegoCSS visual system exactly. Preserve "
        "square joins, engineered-paper tactility, restrained cobalt structure, "
        "and a single cyan lineage cue. The result must feel like an authored "
        "premium editorial artifact, never generic AI developer imagery."
    )


def generate(token: str, spec: AssetSpec, prompt: str) -> tuple[bytes, dict]:
    payload = json.dumps(
        {
            "model": MODEL_ID,
            "prompt": prompt,
            "n": 1,
            "size": spec.size,
            "quality": "high",
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        ENDPOINT,
        data=payload,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "User-Agent": "PliegoCSS-brand-image-generator/1",
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=600) as response:
        body = json.loads(response.read().decode("utf-8"))
        request_id = response.headers.get("request-id") or response.headers.get(
            "x-request-id"
        )
    data = body.get("data")
    if not isinstance(data, list) or len(data) != 1:
        raise RuntimeError(f"unexpected image response for {spec.role}")
    encoded = data[0].get("b64_json")
    if not isinstance(encoded, str) or not encoded:
        raise RuntimeError(f"response for {spec.role} did not contain b64_json")
    metadata = {
        "generationId": request_id or f"created-{body.get('created', int(time.time()))}",
        "revisedPrompt": data[0].get("revised_prompt"),
        "usage": body.get("usage"),
    }
    return base64.b64decode(encoded, validate=True), metadata


def write_manifest(receipts: list[dict]) -> None:
    manifest = {"schemaVersion": 1, "assets": receipts}
    path = IMAGE_DIR / "manifest.json"
    path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--roles",
        nargs="*",
        default=[asset.role for asset in ASSETS],
        help="subset of canonical roles to generate",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="replace existing masters; omitted by default to prevent accidental spend",
    )
    args = parser.parse_args()

    token = os.environ.get("DIGITALOCEAN_TOKEN")
    if not token:
        parser.error("DIGITALOCEAN_TOKEN is required")

    requested = set(args.roles)
    unknown = requested - {asset.role for asset in ASSETS}
    if unknown:
        parser.error(f"unknown roles: {', '.join(sorted(unknown))}")

    IMAGE_DIR.mkdir(parents=True, exist_ok=True)
    receipts: list[dict] = []
    existing_manifest = IMAGE_DIR / "manifest.json"
    if existing_manifest.exists() and not args.force:
        parser.error("manifest.json already exists; pass --force to regenerate the collection")

    for spec in ASSETS:
        master_path = IMAGE_DIR / spec.master
        if spec.role not in requested:
            continue
        if master_path.exists() and not args.force:
            parser.error(f"{master_path.name} exists; pass --force to replace it")

        prompt = final_prompt(spec)
        print(f"Generating {spec.role} with {MODEL_ID} at {spec.size}...", flush=True)
        try:
            image_bytes, metadata = generate(token, spec, prompt)
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            print(f"DigitalOcean returned HTTP {error.code}: {detail[:800]}", file=sys.stderr)
            return 1

        temporary = master_path.with_suffix(".png.tmp")
        temporary.write_bytes(image_bytes)
        with Image.open(temporary) as image:
            image.verify()
        with Image.open(temporary) as image:
            width, height = image.size
            if image.format != "PNG":
                raise RuntimeError(f"{spec.role} response was {image.format}, expected PNG")
        if width < 1024 or height < 1024:
            raise RuntimeError(f"{spec.role} response is too small: {width}x{height}")
        temporary.replace(master_path)

        receipts.append(
            {
                "role": spec.role,
                "master": spec.master,
                "masterSha256": hashlib.sha256(image_bytes).hexdigest(),
                "model": MANIFEST_MODEL,
                "quality": "high",
                "width": width,
                "height": height,
                "promptFile": f"../image-prompts/{spec.prompt_file}",
                "finalPrompt": prompt,
                "generationId": metadata["generationId"],
                "seed": None,
                "reviewedAgainstBrandbook": False,
            }
        )
        print(f"Wrote {master_path.name} ({width}x{height}).", flush=True)

    if requested == {asset.role for asset in ASSETS}:
        write_manifest(receipts)
        print("Wrote provisional manifest.json; visual review is still required.", flush=True)
    else:
        partial = IMAGE_DIR / "manifest.partial.json"
        partial.write_text(
            json.dumps({"schemaVersion": 1, "assets": receipts}, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(f"Wrote {partial.name}; generate all roles before canonical review.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
