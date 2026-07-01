#!/usr/bin/env python3
"""Tool-hygiene gate — hard-fails when a block ships a defect the page renderer
can't fix on its own, or an unfinished scaffold placeholder. Checks, all traced
to real regressions:

  1. enum→manifest drift. The page form (`tools/generator/src/control.rs`) reads
     `manifest.json` → `tool.parameters.properties`, NOT the live descriptor, and
     renders a `<select>` ONLY when the param carries an `enum` there. So a
     `Param::enumv(...)` in `src/lib.rs` whose `manifest.json` lost the `enum`
     silently renders a plain TEXT BOX instead of a dropdown. This gate requires
     every real `enumv` param to have a matching `enum` in the manifest (and, when
     the variant list is parseable, the SAME variants).

  2. FAQ formatting. `site/tool.css` styles FAQ as `<details>`/`<summary>`
     accordions (`.tool-content details ...`), but only if `page/content.md`
     actually uses that markup. A FAQ written as plain `## FAQ` markdown renders
     as bare headings. This gate requires any content.md with a FAQ section to use
     `<details>` accordions.

  3. Unfinished scaffold. A committed `TODO` in `manifest.json`/`wafer.toml`/
     `page/meta.toml`, the `TODO: SEO copy` stub in `page/content.md`, or the
     scaffold `TODO: one-line summary.` left in the `src/lib.rs` wafer_block
     macro (what chat/the runtime shows) — all mean scaffold placeholders shipped
     instead of real metadata/copy.

  4. Summary drift. The wafer_block macro summary, `manifest.json` `summary`, and
     `wafer.toml` `summary` must agree (a trailing period is ignored).

Prose usability standards (see `.claude/skills/improve-tool/SKILL.md`) were being
skipped silently because nothing failed the build. This turns the mechanically
checkable ones into a gate.

Usage:
  scripts/check-tool-hygiene.py                 # check every blocks/<slug>/
  scripts/check-tool-hygiene.py <slug> [<slug>] # check only these tools

Exit code 0 = clean, 1 = violations found (prints each), 2 = usage error.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BLOCKS = ROOT / "blocks"

# `Param::enumv("name", ["a", "b", ...])` — name plus (optionally) the literal
# variant list. Non-greedy across the comment-stripped, whitespace-joined blob so
# multi-line calls match. The variant list may be absent from the capture if it's
# built from a const/helper rather than an inline array.
ENUMV_RE = re.compile(r'Param::enumv\(\s*"([^"]+)"\s*(?:,\s*(?:&?\[([^\]]*)\])?)?')
STR_LIT_RE = re.compile(r'"([^"]*)"')
FAQ_HEADING_RE = re.compile(r"^#{1,6}\s*(faq|frequently asked)", re.IGNORECASE | re.MULTILINE)
MACRO_SUMMARY_RE = re.compile(r'wafer_block\((?:.|\n)*?\bsummary\s*=\s*"((?:\\.|[^"\\])*)"')
WAFER_SUMMARY_RE = re.compile(r'^\s*summary\s*=\s*"((?:\\.|[^"\\])*)"', re.MULTILINE)


def norm_summary(s: str) -> str:
    """Normalize a summary for comparison — trailing period/whitespace is noise."""
    return (s or "").strip().rstrip(".").strip()


def faq_section(text: str, m: "re.Match[str]") -> str:
    """The FAQ section body: from the FAQ heading `m` to the next heading of the
    same-or-higher level (or EOF). Scoping the <details> check here means an
    unrelated <details> elsewhere in content.md can't mask a plain-markdown FAQ."""
    level = len(re.match(r"#+", m.group(0)).group(0))
    rest = text[m.end():]
    nxt = re.search(rf"^#{{1,{level}}}\s", rest, re.MULTILINE)
    return rest[: nxt.start()] if nxt else rest


_RUST_ESCAPES = {"\\": "\\", '"': '"', "'": "'", "n": "\n", "t": "\t", "r": "\r", "0": "\0"}


def unescape_rust(s: str) -> str:
    """Resolve the common Rust string escapes so a source token like `\\x`
    (two backslashes in the .rs file) compares equal to the manifest's
    JSON-decoded `\\x` (one backslash). Without this, any enum variant containing
    a backslash or quote false-flags as STALE."""
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


def strip_line_comments(src: str) -> str:
    """Drop whole-line `//`/`///` doc comments (the scaffold's descriptor doc
    comment contains a `Param::enumv("mode", ...)` EXAMPLE that must not count as a
    real param) and join the rest so multi-line calls match as one blob."""
    kept = [ln for ln in src.splitlines() if not ln.lstrip().startswith("//")]
    return " ".join(kept)


def descriptor_enums(src_lib: Path) -> dict[str, set[str] | None]:
    """name -> set of variants (or None when the variant list isn't an inline
    literal we can parse). Only real (non-comment) `Param::enumv` calls."""
    blob = strip_line_comments(src_lib.read_text(encoding="utf-8", errors="replace"))
    out: dict[str, set[str] | None] = {}
    for m in ENUMV_RE.finditer(blob):
        name = m.group(1)
        arr = m.group(2)
        variants = {unescape_rust(v) for v in STR_LIT_RE.findall(arr)} if arr is not None else None
        # If the same name appears twice, prefer a parseable variant set.
        if name not in out or (out[name] is None and variants is not None):
            out[name] = variants
    return out


def manifest_props(manifest: Path) -> dict | None:
    try:
        data = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return (
        data.get("tool", {})
        .get("parameters", {})
        .get("properties")
    )


def check_block(slug_dir: Path) -> list[str]:
    slug = slug_dir.name
    problems: list[str] = []

    src_lib = slug_dir / "src" / "lib.rs"
    manifest = slug_dir / "manifest.json"
    if src_lib.is_file() and manifest.is_file():
        enums = descriptor_enums(src_lib)
        if enums:
            props = manifest_props(manifest)
            if props is None:
                problems.append(
                    f"{slug}: has enum param(s) but manifest.json tool.parameters.properties is missing/unreadable"
                )
            else:
                for name, variants in enums.items():
                    prop = props.get(name)
                    man_enum = prop.get("enum") if isinstance(prop, dict) else None
                    if not isinstance(man_enum, list) or not man_enum:
                        problems.append(
                            f"{slug}: descriptor Param::enumv(\"{name}\") but manifest.json "
                            f"properties.{name} has no enum → manifest is out of sync with the descriptor "
                            f"(a page tool renders a TEXT BOX instead of a <select>). "
                            f"Sync manifest.json tool.parameters to the descriptor."
                        )
                    elif variants is not None and set(man_enum) != variants:
                        problems.append(
                            f"{slug}: enum param \"{name}\" is STALE — descriptor {sorted(variants)} "
                            f"vs manifest {sorted(map(str, man_enum))}. Re-sync manifest.json."
                        )

    content = slug_dir / "page" / "content.md"
    if content.is_file():
        text = content.read_text(encoding="utf-8", errors="replace")
        m = FAQ_HEADING_RE.search(text)
        if m and "<details" not in faq_section(text, m):
            problems.append(
                f"{slug}: page/content.md has a FAQ section written as plain markdown — "
                f"convert it to <details>/<summary>/<p> accordions (see blocks/age-calculator, "
                f"improve-tool usability standard #8) so site/tool.css styles it."
            )
        if "TODO: SEO copy" in text:
            problems.append(f"{slug}: page/content.md still has the scaffold 'TODO: SEO copy' stub — write real copy.")

    # scaffold-TODO leftovers in metadata files (a `TODO` is never legitimate here,
    # unlike prose): a shipped placeholder means the tool wasn't finished.
    for rel in ("manifest.json", "wafer.toml", "page/meta.toml"):
        fp = slug_dir / rel
        if fp.is_file() and "TODO" in fp.read_text(encoding="utf-8", errors="replace"):
            problems.append(f"{slug}: {rel} still contains a scaffold 'TODO' placeholder — fill it in.")

    # The #[wafer_block(summary=...)] macro summary is what the runtime/chat shows.
    # Match the exact scaffold phrase so ordinary `// TODO` code comments don't trip.
    if src_lib.is_file() and "TODO: one-line summary." in src_lib.read_text(encoding="utf-8", errors="replace"):
        problems.append(f"{slug}: src/lib.rs wafer_block summary is still the scaffold placeholder — write a real one-line summary.")

    # Summary consistency: the wafer_block macro summary (what chat/the runtime shows),
    # manifest.json `summary`, and wafer.toml `summary` must agree (a trailing period is
    # ignored) and must not carry the vestigial "… skill" scaffold suffix.
    summaries: dict[str, str] = {}
    if src_lib.is_file():
        m = MACRO_SUMMARY_RE.search(src_lib.read_text(encoding="utf-8", errors="replace"))
        if m:
            summaries["src macro"] = unescape_rust(m.group(1))
    if manifest.is_file():
        try:
            s = json.loads(manifest.read_text(encoding="utf-8")).get("summary")
        except (OSError, json.JSONDecodeError):
            s = None
        if isinstance(s, str):
            summaries["manifest.json"] = s
    wafer = slug_dir / "wafer.toml"
    if wafer.is_file():
        m = WAFER_SUMMARY_RE.search(wafer.read_text(encoding="utf-8", errors="replace"))
        if m:
            summaries["wafer.toml"] = unescape_rust(m.group(1))
    if summaries and len({norm_summary(v) for v in summaries.values()}) > 1:
        detail = ", ".join(f"{k}={v!r}" for k, v in summaries.items())
        problems.append(f"{slug}: summary differs across {detail} — keep the three in sync (a trailing period is fine).")

    return problems


def main(argv: list[str]) -> int:
    if argv:
        dirs = []
        for slug in argv:
            d = BLOCKS / slug
            if not d.is_dir():
                print(f"error: blocks/{slug}/ not found", file=sys.stderr)
                return 2
            dirs.append(d)
    else:
        dirs = sorted(p for p in BLOCKS.iterdir() if p.is_dir())

    all_problems: list[str] = []
    for d in dirs:
        all_problems.extend(check_block(d))

    if all_problems:
        print(f"tool-hygiene: {len(all_problems)} violation(s) across {len(dirs)} tool(s):\n")
        for p in all_problems:
            print(f"  ✗ {p}")
        print("\nFAIL — fix the above before committing (see scripts/check-tool-hygiene.py header).")
        return 1

    print(f"tool-hygiene: OK — {len(dirs)} tool(s) clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
