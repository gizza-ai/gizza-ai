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

  2. FAQ formatting. `tools/generator/assets/runtime/tool.css` styles FAQ as `<details>`/`<summary>`
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

  8. Site branding. Pages must be generic: the site injects branding at render
     time (`SiteConfig` `title_suffix`/header/footer — `tools/generator/src/site.rs`).
     A literal `gizza.ai`/`gizza-ai.pages.dev` string in `page/meta.toml`,
     `content.md`, `custom.js` or `custom.css` would leak the brand into the
     public, MIT-licensed page sources themselves.

Prose usability standards (see `.claude/skills/improve-tool/SKILL.md`) were being
skipped silently because nothing failed the build. This turns the mechanically
checkable ones into a gate.

STRICT checks (5-7) — hard-fail ONLY in per-slug mode (how the build/improve skills
run it, so every NEW/improved tool meets them); repo-wide they are printed as an
aggregated advisory so CI stays green until the corpus is backfilled:

  5. Placeholders. Every page/meta.toml `[[input]]` that renders as a text/number
     field (i.e. not an enum <select> or boolean checkbox per the manifest) must
     have a non-empty `placeholder` — the placeholder is the page's worked example.

  6. FAQ depth. A tool page must answer ≥3 real questions as `<details>` accordions.

  7. Meta description length. page/meta.toml `description` is the SERP snippet —
     require 50-170 chars (truncation starts ~160; shorter than 50 wastes the slot).

Usage:
  scripts/check-tool-hygiene.py                 # repo-wide: checks 1-4+8 gate, 5-7 advisory
  scripts/check-tool-hygiene.py <slug> [<slug>] # per-slug STRICT: checks 1-8 all gate

Exit code 0 = clean, 1 = violations found (prints each), 2 = usage error.
"""
from __future__ import annotations

import json
import re
import sys
import tomllib
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
BRAND_RE = re.compile(r"gizza\.ai|gizza-ai\.pages\.dev", re.IGNORECASE)


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
                f"improve-tool usability standard #8) so tools/generator/assets/runtime/tool.css styles it."
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

    # 8. Site branding. Pages must be generic: the site injects branding at
    #    render time (SiteConfig title_suffix/header/footer). Domain strings
    #    in page/ would leak the brand into the public, MIT-licensed pages.
    for name in ("meta.toml", "content.md", "custom.js", "custom.css"):
        f = slug_dir / "page" / name
        if f.is_file():
            m = BRAND_RE.search(f.read_text(encoding="utf-8", errors="replace"))
            if m:
                problems.append(
                    f"{slug}: site branding {m.group(0)!r} in page/{name} "
                    "— pages must be generic (branding is injected by the site config)"
                )

    return problems


def strict_checks(slug_dir: Path) -> list[tuple[str, str]]:
    """Checks 5-7 (placeholders / FAQ depth / description length) as
    (category, message) pairs. Gate per-slug; advisory repo-wide — see header."""
    slug = slug_dir.name
    out: list[tuple[str, str]] = []

    meta_path = slug_dir / "page" / "meta.toml"
    meta = None
    if meta_path.is_file():
        try:
            meta = tomllib.loads(meta_path.read_text(encoding="utf-8", errors="replace"))
        except tomllib.TOMLDecodeError as e:
            out.append(("meta", f"{slug}: page/meta.toml does not parse as TOML: {e}"))

    if meta is not None:
        desc = str(meta.get("description", ""))
        if not 50 <= len(desc) <= 170:
            out.append((
                "description",
                f"{slug}: page/meta.toml description is {len(desc)} chars — keep it 50-170 "
                f"(it is the SERP snippet; ~160 is where truncation starts).",
            ))
        props = manifest_props(slug_dir / "manifest.json") or {}
        for inp in meta.get("input", []):
            if inp.get("source") != "field":
                continue
            name = str(inp.get("name", "?"))
            prop = props.get(name)
            if isinstance(prop, dict) and (prop.get("enum") or prop.get("type") == "boolean"):
                continue  # renders as a <select>/checkbox — no placeholder slot
            if not str(inp.get("placeholder", "")).strip():
                out.append((
                    "placeholder",
                    f"{slug}: page/meta.toml [[input]] \"{name}\" renders as a text/number field "
                    f"but has NO placeholder — the placeholder is the page's worked example.",
                ))

    content = slug_dir / "page" / "content.md"
    if content.is_file():
        text = content.read_text(encoding="utf-8", errors="replace")
        m = FAQ_HEADING_RE.search(text)
        n = faq_section(text, m).count("<details") if m else text.count("<details")
        if n < 3:
            out.append((
                "faq",
                f"{slug}: page FAQ has {n} <details> entries — answer ≥3 real user questions "
                f"(predictable confusion points, limits, privacy).",
            ))

    return out


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
    per_slug = bool(argv)

    all_problems: list[str] = []
    advisory: list[tuple[str, str]] = []
    for d in dirs:
        all_problems.extend(check_block(d))
        strict = strict_checks(d)
        if per_slug:
            all_problems.extend(msg for _, msg in strict)
        else:
            advisory.extend(strict)

    if advisory:
        by_cat: dict[str, list[str]] = {}
        for cat, msg in advisory:
            by_cat.setdefault(cat, []).append(msg)
        counts = ", ".join(f"{cat}: {len(msgs)}" for cat, msgs in sorted(by_cat.items()))
        print(f"tool-hygiene: {len(advisory)} ADVISORY finding(s) repo-wide ({counts}) — these")
        print("gate per-slug runs (new/improved tools) but not the repo-wide sweep. Samples:")
        for cat, msgs in sorted(by_cat.items()):
            for msg in msgs[:3]:
                print(f"  ⚠ {msg}")
            if len(msgs) > 3:
                print(f"  ⚠ … and {len(msgs) - 3} more \"{cat}\" findings")
        print()

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
