#!/usr/bin/env bash
# Self-test for scripts/check-tool-hygiene.py — asserts the gate FAILS on a block
# with a drifted manifest + plain-markdown FAQ, and PASSES once both are fixed.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
slug="zzhygiene-scratch"
dir="$root/blocks/$slug"
cleanup() { rm -rf "$dir"; }
trap cleanup EXIT INT TERM
rm -rf "$dir"
mkdir -p "$dir/src" "$dir/page"

# A real (non-comment) enum param, plus an unfilled scaffold macro summary.
cat > "$dir/src/lib.rs" <<'EOF'
#[wafer_block(summary = "TODO: one-line summary.")]
fn descriptor() {
    let _ = Param::enumv("mode", ["encode", "decode"]).default("encode");
}
EOF

# BAD manifest: `mode` present but with NO enum (the drift that renders a text box),
# plus a leftover scaffold TODO.
cat > "$dir/manifest.json" <<'EOF'
{ "summary": "TODO: one-line summary.", "tool": { "parameters": { "properties": {
  "mode": { "type": "string", "default": "encode" }
} } } }
EOF

# BAD content: FAQ as plain markdown, no <details>.
cat > "$dir/page/content.md" <<'EOF'
## About
Prose.

## FAQ

**Is it free?** Yes.
EOF

if python3 "$root/scripts/check-tool-hygiene.py" "$slug" >/dev/null 2>&1; then
  echo "FAIL: gate passed a drifted+plain-FAQ block (should have failed)" >&2
  exit 1
fi
out="$(python3 "$root/scripts/check-tool-hygiene.py" "$slug" 2>&1 || true)"
grep -q 'TEXT BOX' <<<"$out" || { echo "FAIL: missing enum-drift violation" >&2; exit 1; }
grep -q 'FAQ section written as plain markdown' <<<"$out" || { echo "FAIL: missing FAQ violation" >&2; exit 1; }
grep -q "scaffold 'TODO' placeholder" <<<"$out" || { echo "FAIL: missing TODO violation" >&2; exit 1; }
grep -q "wafer_block summary is still the scaffold placeholder" <<<"$out" || { echo "FAIL: missing macro-summary violation" >&2; exit 1; }

# FIX all and expect a clean pass (per-slug mode is STRICT: checks 5-7 gate too,
# so the compliant fixture needs a meta.toml with placeholders + an in-range
# description, and >=3 FAQ accordions).
cat > "$dir/src/lib.rs" <<'EOF'
#[wafer_block(summary = "Encode or decode data.")]
fn descriptor() {
    let _ = Param::enumv("mode", ["encode", "decode"]).default("encode");
}
EOF
# manifest summary matches the macro summary (consistency check #4). `text` has no
# enum/boolean type -> renders as a text field -> needs a placeholder (check #5).
cat > "$dir/manifest.json" <<'EOF'
{ "summary": "Encode or decode data.", "tool": { "parameters": { "properties": {
  "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode" },
  "text": { "type": "string" }
} } } }
EOF
cat > "$dir/page/meta.toml" <<'EOF'
slug        = "zzhygiene-scratch"
description = "Encode or decode data instantly in your browser — free, private, and offline-capable."

[[input]]
name        = "text"
label       = "Text"
source      = "field"
placeholder = "hello world"

[[input]]
name        = "mode"
label       = "Mode"
source      = "field"
placeholder = ""
EOF
cat > "$dir/page/content.md" <<'EOF'
## About
Prose.

## FAQ

<details>
<summary>Is it free?</summary>

Yes.

</details>

<details>
<summary>Does my data leave the browser?</summary>

No.

</details>

<details>
<summary>Is there a size limit?</summary>

Inputs over 10 MB may be slow.

</details>
EOF

python3 "$root/scripts/check-tool-hygiene.py" "$slug" >/dev/null 2>&1 || {
  echo "FAIL: gate rejected a compliant block" >&2
  python3 "$root/scripts/check-tool-hygiene.py" "$slug" >&2 || true
  exit 1; }

# Repo-wide mode must NOT gate on the strict checks (advisory only): break a
# strict rule and confirm per-slug fails while the same block passes repo-wide
# scanning. (Repo-wide still gates checks 1-4.)
sed -i 's/placeholder = "hello world"/placeholder = ""/' "$dir/page/meta.toml"
out3="$(python3 "$root/scripts/check-tool-hygiene.py" "$slug" 2>&1 || true)"
grep -q 'NO placeholder' <<<"$out3" || { echo "FAIL: strict mode missed a missing placeholder" >&2; exit 1; }
sed -i 's/placeholder = ""$/placeholder = "hello world"/' "$dir/page/meta.toml"
# ^ also gives `mode` a placeholder — harmless, it renders as a <select> (enum)

# Too-short SERP description fails strict.
sed -i 's/^description = .*/description = "Too short."/' "$dir/page/meta.toml"
out4="$(python3 "$root/scripts/check-tool-hygiene.py" "$slug" 2>&1 || true)"
grep -q 'SERP snippet' <<<"$out4" || { echo "FAIL: strict mode missed a bad description length" >&2; exit 1; }
sed -i 's/^description = .*/description = "Encode or decode data instantly in your browser — free, private, and offline-capable."/' "$dir/page/meta.toml"

# Fewer than 3 FAQ accordions fails strict.
cat > "$dir/page/content.md" <<'EOF'
## About
Prose.

## FAQ

<details>
<summary>Is it free?</summary>

Yes.

</details>
EOF
out5="$(python3 "$root/scripts/check-tool-hygiene.py" "$slug" 2>&1 || true)"
grep -q 'answer ≥3 real user questions' <<<"$out5" || { echo "FAIL: strict mode missed a thin FAQ" >&2; exit 1; }
cat > "$dir/page/content.md" <<'EOF'
## About
Prose.

## FAQ

<details>
<summary>Is it free?</summary>

Yes.

</details>

<details>
<summary>Does my data leave the browser?</summary>

No.

</details>

<details>
<summary>Is there a size limit?</summary>

Inputs over 10 MB may be slow.

</details>
EOF
python3 "$root/scripts/check-tool-hygiene.py" "$slug" >/dev/null 2>&1 || {
  echo "FAIL: gate rejected the re-fixed block" >&2; exit 1; }

# A summary that disagrees with the macro must fail (check #4).
cat > "$dir/manifest.json" <<'EOF'
{ "summary": "A completely different summary.", "tool": { "parameters": { "properties": {
  "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode" }
} } } }
EOF
out2="$(python3 "$root/scripts/check-tool-hygiene.py" "$slug" 2>&1 || true)"
grep -q 'summary differs' <<<"$out2" || { echo "FAIL: gate missed a summary inconsistency" >&2; exit 1; }

echo "check-tool-hygiene.test.sh OK"
