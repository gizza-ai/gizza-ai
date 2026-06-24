# emoji-search — competitor analysis (2026-06-23)

Tool: `blocks/emoji-search` — search a curated, embedded emoji dataset (263
glyphs) by keyword, `:shortcode:`, or category; returns ranked matches as
labelled lines or bare glyphs. Pure-compute, fully offline (no network), three
surfaces: chat skill, CLI, standalone page.

## Surfaces verified (Phase 1)

- **chat block**: `wafer build` validated + instantiated `target/block.wasm`
  (331.9 KiB). Drift-guard schema test passes (`schema_json_matches_authored_chat_schema`).
- **CLI**: `gizza tool emoji-search query=… [limit=…] [glyphs_only=…]` — verified
  keyword (`query=happy`), shortcode (`query=:rocket:` → 🚀), category +
  glyphs-only (`query=flags glyphs_only=true`), and the empty-query error path.
- **page** (`/tools/emoji-search/`): 3 Playwright specs pass — shortcode search,
  glyphs-only checkbox, and `?query=…&limit=…` deep-link prefill.
- **unit tests**: 10 core tests (ranking, colon-stripping, keyword, category,
  limit clamp, empty-query reject, no-match note, glyphs-only).

## Top competitors surveyed

1. **Emojipedia** (emojipedia.org) — the reference encyclopedia: per-emoji
   pages, vendor renderings (Apple/Google/Samsung/…), Unicode version, shortcode
   lists, copy button, full keyword search.
2. **Get Emoji** (getemoji.com) — one long copy-friendly page grouped by
   category; click-to-copy; no real search box, browse-only.
3. **Emoji.gg** (emoji.gg) — Discord/Slack custom-emoji directory + search;
   download/upload custom emoji (out of scope: those are uploaded images, not
   Unicode).
4. **OpenMoji / Twemoji search UIs** — search the open emoji sets by name and
   tag; SVG/PNG asset download.
5. **GitHub/Slack `:shortcode:` autocomplete** — the canonical shortcode UX:
   type `:`, fuzzy-match the shortcode, insert the glyph.

## Capability diff (fit-to-model = pure offline Rust + text/page output)

| Capability | Competitors | emoji-search | Status |
|---|---|---|---|
| Keyword search by meaning | all | yes (name + keywords) | ✅ in model |
| `:shortcode:` search | Slack/GitHub/Emojipedia | yes (colons stripped) | ✅ |
| Category browse | Get Emoji, Emojipedia | yes (query a category name) | ✅ |
| Ranked / best-match-first results | most | yes (exact > keyword > prefix > substring) | ✅ |
| Copy bare glyphs | Get Emoji, all | yes (`glyphs_only`) | ✅ |
| Result-count limit | n/a | yes (`limit`, 1–100) | ✅ (UX add) |
| Offline / private (no network) | rare | yes (embedded dataset) | ✅ differentiator |
| Per-vendor renderings (Apple/Google art) | Emojipedia | no — renders the host's font | ❌ out of model (needs vendor image assets) |
| Custom Discord/Slack uploaded emoji | Emoji.gg | no | ❌ out of scope (uploaded images, not Unicode) |
| SVG/PNG asset download | OpenMoji/Twemoji | no — text/glyph output only | ❌ out of model (the page format is text) |
| Skin-tone / ZWJ variant pickers | Emojipedia | partial — a few ZWJ glyphs included, no interactive modifier picker | ❌ out of model (interactive UI, not a compute tool) |
| Full ~3,800-emoji Unicode set | Emojipedia | no — 263 curated common glyphs | ⚠️ partial (curated breadth; covers the common long-tail) |

## Gaps closed this pass

The tool was built feature-complete against the in-model capability set, so no
in-model gap remained open after Phase 1:

- **ranked relevance** (exact shortcode/name 100 > keyword 80 > category 60 >
  name/shortcode prefix 70 > word-start prefix 50 > keyword prefix 45 >
  substring 30; ties → name asc) so the obvious match (`:rocket:` → 🚀,
  `heart` → ❤️) is first, not buried.
- **colon-insensitive shortcode** matching (`:smile:` ≡ `smile`).
- **category listing** by name across 7 buckets (smileys, people, animals, food,
  activities, symbols, flags).
- **glyphs-only** copy mode for pasting a row of emoji.
- **263-entry curated dataset** spanning all 7 categories incl. common ZWJ
  sequences (rainbow/pirate flags, face-in-clouds) and country flags.

## Out-of-model (documented, not built)

- Per-vendor emoji artwork (Apple/Google/Samsung renderings) — needs bundled
  image assets per vendor; the tool renders the host font's glyph.
- SVG/PNG emoji-asset download — the page output format is text/glyphs, not a
  media envelope; an image-bytes output would have no page render mode.
- Interactive skin-tone / ZWJ modifier picker — an interactive UI, not a
  deterministic compute surface.
- Custom uploaded (Discord/Slack) emoji — these are user images, not Unicode.
- The full ~3,800-glyph Unicode set — deliberately curated to the common,
  high-utility glyphs; breadth can grow by extending `core/src/data.rs`.

NEVER copied competitor copy, branding, shortcode lists verbatim, or trademarks.
The dataset, shortcodes, and ranking are authored here.
