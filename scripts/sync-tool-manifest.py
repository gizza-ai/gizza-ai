#!/usr/bin/env python3
"""Generate a block's manifest.json `tool.*` + summaries from the LIVE descriptor.

Why: the page form (`tools/generator/src/control.rs`) reads `manifest.json` →
`tool.parameters`, NOT the descriptor inside the wasm — historically that section was
hand-synced, and it drifted (enums lost → text boxes instead of <select>s; stale
param descriptions). This script makes the manifest a GENERATED artifact:

  tool.parameters  ←  `gizza list --json-out` `parameters`   (the descriptor's schema_json())
  tool.description ←  `gizza list --json-out` `description` (the skill() description)
  summary          ←  `#[wafer_block(summary = "…")]` in src/lib.rs, propagated to
                      manifest.json `summary` AND wafer.toml `summary`

Prereq: the tool must be in the INSTALLED CLI — run `cargo install --path cli --force`
after `wafer build` and BEFORE this script. Run this BEFORE the page generator (which
reads the manifest). `scripts/check-tool-hygiene.py` stays as the CI backstop.

The live descriptors come from ONE `gizza list --json-out` per run. Every `gizza`
invocation boots the whole runtime (it loads every embedded block), so a call per
block would cost a full boot per block: at 1440 blocks that put a 60-block CI
drift shard past its time limit.

Usage:
  scripts/sync-tool-manifest.py <slug> [<slug>...]   # rewrite the manifest(s)
  scripts/sync-tool-manifest.py --check <slug> ...   # exit 1 if out of sync, write nothing

Exit code 0 = synced/clean, 1 = --check found drift or a tool failed, 2 = usage error.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BLOCKS = ROOT / "blocks"

# Same source-parsing rules as scripts/check-tool-hygiene.py (kept in sync by
# check-tool-hygiene.test.sh): the macro summary is what chat/the runtime shows.
MACRO_SUMMARY_RE = re.compile(r'wafer_block\((?:.|\n)*?\bsummary\s*=\s*"((?:\\.|[^"\\])*)"')
WAFER_SUMMARY_RE = re.compile(r'^(\s*summary\s*=\s*)"(?:\\.|[^"\\])*"', re.MULTILINE)

_RUST_ESCAPES = {"\\": "\\", '"': '"', "'": "'", "n": "\n", "t": "\t", "r": "\r", "0": "\0"}


def unescape_rust(s: str) -> str:
    out: list[str] = []
    i = 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s) and s[i + 1] in _RUST_ESCAPES:
            out.append(_RUST_ESCAPES[s[i + 1]])
            i += 2
        else:
            out.append(s[i])
            i += 1
    return "".join(out)


def gizza(args: list[str]) -> str:
    try:
        # One call boots the runtime and loads all of the CLI's blocks.
        r = subprocess.run(["gizza", *args], capture_output=True, text=True, timeout=300)
    except FileNotFoundError:
        sys.exit("error: `gizza` not on PATH — run `cargo install --path cli --force` first")
    if r.returncode != 0:
        raise RuntimeError(
            f"`gizza {' '.join(args)}` failed (exit {r.returncode}): {r.stderr.strip() or r.stdout.strip()}\n"
            f"  (is the block built + the CLI reinstalled? `wafer build` then `cargo install --path cli --force`)"
        )
    return r.stdout


def live_tools() -> dict[str, dict]:
    """Every tool the installed CLI lists, keyed by full block name."""
    tools = json.loads(gizza(["list", "--json-out"]))
    return {tool["name"]: tool for tool in tools}


def sync_block(slug: str, check_only: bool, live: dict[str, dict]) -> tuple[bool, list[str]]:
    """Returns (changed, messages). Raises RuntimeError on a hard failure."""
    slug_dir = BLOCKS / slug
    manifest_path = slug_dir / "manifest.json"
    if not manifest_path.is_file():
        raise RuntimeError(f"blocks/{slug}/manifest.json not found")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    if "tool" not in manifest:
        return False, [f"{slug}: no `tool` section in manifest.json — nothing to sync (non-tool block)"]

    name = manifest.get("name", f"gizza-ai/{slug}")
    tool = live.get(name)
    if tool is None:
        raise RuntimeError(
            f"`gizza list` does not list {name}\n"
            f"  (is the block built + the CLI reinstalled? `wafer build` then `cargo install --path cli --force`)"
        )
    msgs: list[str] = []
    changed = False

    # tool.parameters ← live schema_json()
    params = tool["parameters"]
    if manifest["tool"].get("parameters") != params:
        changed = True
        msgs.append(f"{slug}: tool.parameters ← live descriptor schema")
        manifest["tool"]["parameters"] = params

    # tool.description ← the skill() description
    desc = tool["description"].strip()
    if desc and manifest["tool"].get("description") != desc:
        changed = True
        msgs.append(f"{slug}: tool.description ← live descriptor description")
        manifest["tool"]["description"] = desc

    # summary ← wafer_block macro, propagated to manifest.json + wafer.toml
    src_lib = slug_dir / "src" / "lib.rs"
    summary = None
    if src_lib.is_file():
        sm = MACRO_SUMMARY_RE.search(src_lib.read_text(encoding="utf-8", errors="replace"))
        if sm:
            summary = unescape_rust(sm.group(1))
    if summary:
        if manifest.get("summary") != summary:
            changed = True
            msgs.append(f"{slug}: manifest summary ← macro summary")
            manifest["summary"] = summary
        wafer_path = slug_dir / "wafer.toml"
        if wafer_path.is_file():
            wafer_text = wafer_path.read_text(encoding="utf-8")
            new_text, n = WAFER_SUMMARY_RE.subn(
                lambda mm: mm.group(1) + json.dumps(summary, ensure_ascii=False), wafer_text, count=1
            )
            if n and new_text != wafer_text:
                changed = True
                msgs.append(f"{slug}: wafer.toml summary ← macro summary")
                if not check_only:
                    wafer_path.write_text(new_text, encoding="utf-8")
    else:
        msgs.append(f"{slug}: WARN no wafer_block macro summary found in src/lib.rs — summaries not synced")

    if changed and not check_only:
        manifest_path.write_text(
            json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    return changed, msgs


def main(argv: list[str]) -> int:
    check_only = "--check" in argv
    slugs = [a for a in argv if not a.startswith("-")]
    if not slugs:
        print(__doc__, file=sys.stderr)
        return 2

    try:
        live = live_tools()
    except (RuntimeError, json.JSONDecodeError, subprocess.TimeoutExpired) as e:
        print(f"  ✗ gizza list --json-out: {e}", file=sys.stderr)
        return 1

    any_drift = False
    failed = False
    for slug in slugs:
        try:
            changed, msgs = sync_block(slug, check_only, live)
        except (RuntimeError, json.JSONDecodeError) as e:
            print(f"  ✗ {slug}: {e}", file=sys.stderr)
            failed = True
            continue
        for msg in msgs:
            print(f"  {'~' if changed else '='} {msg}")
        if changed:
            any_drift = True
            print(f"  {'DRIFT (not written, --check)' if check_only else 'synced'}: blocks/{slug}/")
        else:
            print(f"  ok: blocks/{slug}/ already in sync")

    if failed:
        return 1
    return 1 if (check_only and any_drift) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
