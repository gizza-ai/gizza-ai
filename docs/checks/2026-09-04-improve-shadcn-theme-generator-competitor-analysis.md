# shadcn-theme-generator — competitor scan + build decisions (2026-09-04)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is a
paraphrase of publicly documented behaviour; no competitor copy, branding or trademarked
wording was copied into this repo. Product names appear only as factual references to the
sites that were read.

## Duplicate check (done first)

`ls blocks/ | grep -Ei 'theme|shadcn|css|tailwind|palette|token|design|hsl|oklch'` surfaced
`color-palette-generator`, `color-shades-generator`, `css-color-converter`,
`css-gradient-generator`, `color-contrast-checker`, `randomize-palette`. Reading their
`core/src/lib.rs`:

- `color-shades-generator` emits a **numeric ramp** (Tailwind `50…950`, or N tints/shades/tones)
  as hex/rgb/hsl steps. No semantic tokens, no light/dark pairing, no CSS output.
- `color-palette-generator` emits **color-theory swatches** (complementary, triadic, …).
- `css-color-converter` converts **one** color between notations.
- `color-contrast-checker` scores **one** foreground/background pair.

None of them produce a named-token, light **and** dark, paste-ready CSS variable block. The
existing skiplist entry for `tailwind-palette-generator` (line 624) is explicitly about the
`50–950` ramp — a different output shape. **Not a duplicate; built.**

## Sources read (4)

| # | Source | What it is |
|---|---|---|
| 1 | `ui.shadcn.com/docs/theming` | The upstream spec — canonical variable names + format |
| 2 | `shadcndesign.com/theme-generator` | Full-featured hosted generator |
| 3 | `designrevision.com/tools/shadcn-theme-generator` | One-seed, contrast-checked generator |
| 4 | `jvinhit.github.io/tools/shadcn-theme-generator` | Minimal seed + radius + mode generator |

All four were reachable; no substitutions were needed.

## Table-stakes findings

### Token set (source 1 — the spec everything else targets)

`:root` and `.dark` each define: `--background`/`--foreground`, `--card`/`--card-foreground`,
`--popover`/`--popover-foreground`, `--primary`/`--primary-foreground`,
`--secondary`/`--secondary-foreground`, `--muted`/`--muted-foreground`,
`--accent`/`--accent-foreground`, `--destructive`, `--border`, `--input`, `--ring`,
`--chart-1`…`--chart-5`, `--sidebar`, `--sidebar-foreground`, `--sidebar-primary`,
`--sidebar-primary-foreground`, `--sidebar-accent`, `--sidebar-accent-foreground`,
`--sidebar-border`, `--sidebar-ring`, plus a non-color `--radius`. Current guidance uses
**oklch** notation; Tailwind v3-era themes used bare `H S% L%` triplets consumed through
`hsl(var(--token))`.

### Parameters / controls competitors expose

| Capability | Seen in | Verdict | Where it landed |
|---|---|---|---|
| One seed/primary color | 2, 3, 4 | **in-model** | `primary` (required) |
| Second seed for the accent | 2 (color-family + brand hex) | **in-model** | `accent` (optional; blank = neutral accent, matching upstream) |
| Neutral/gray family choice | 2 (Tailwind family picker) | **in-model** | `neutral` = slate\|gray\|zinc\|neutral\|stone |
| Output notation (oklch / hsl / hex) | 3 | **in-model** | `format` = oklch\|hsl\|hex |
| Tailwind v4 vs v3 output shape | 3 | **in-model** | `tailwind` = v4\|v3 (v4 adds the `@theme inline` map; v3 emits bare HSL triplets in `@layer base`) |
| Border radius token | 2 (None/S/M/L/Full presets), 4 (px) | **in-model** | `radius` (rem, 0–2, slider + preset chips) |
| Light + dark generated together | 2, 3, 4 | **in-model** | `mode` = both\|light\|dark |
| Chart colors | 2 (chart preview), spec | **in-model** | `charts` boolean |
| Sidebar tokens | 2, spec | **in-model** | `sidebar` boolean |
| WCAG-AA contrast-checked foregrounds | 3 (explicit AA claim) | **in-model** | foregrounds are picked by measured contrast ratio; every pair is reported with its ratio + AA verdict, and failures are surfaced as warnings |
| Copy-to-clipboard / paste into `globals.css` | 2, 3, 4 | **in-model** | page output is the finished CSS block as text (generic Copy/Download affordances already exist on tool pages) |

### Out-of-model (listed, deliberately NOT built)

These need a live DOM/preview runtime or design assets that a deterministic pure-Rust block
cannot provide. They are recorded here rather than half-built:

- Live component preview (landing/dashboard/chart mock re-skinned in real time) — sources 2, 3, 4.
- Body/heading **font pickers** with linked pairing — source 2. A theme's font tokens are not
  colors and would need a webfont catalogue.
- "Randomize" exploration button and reset — source 2. Interactive UI state, not a computation.
- Theme marketplace / curated preset gallery — sources 2, 3. Content, not a function.
- Per-token manual editing after generation — source 3. That is an editor surface.
- Direct write into a user's `globals.css` / CLI registry install — no filesystem access.

## Worked example recorded during the scan

Seed `#6366f1` (the indigo used as the example on source 4), `neutral = zinc`,
`format = oklch`, `tailwind = v4`, `radius = 0.625`:

- light `--primary` keeps the seed exactly (`oklch(0.585 0.233 277.117)`), and
  `--primary-foreground` resolves to the near-white neutral because it wins the contrast
  comparison against the near-black one.
- dark `--primary` is lifted in lightness so it still reads on the dark surface, and
  `--primary-foreground` flips to the near-black neutral.
- `--radius: 0.625rem` is emitted verbatim in `:root`.

## Decisions taken from the scan

1. **Seed fidelity beats "prettier" recolouring.** Sources 3 and 4 both keep the user's color as
   `--primary` in light mode. We do the same and only adjust lightness for the dark block,
   documenting the rule on the page — a generator that silently changes a brand color is
   worse than one that explains itself.
2. **Contrast is computed, not asserted.** Source 3 claims AA; we emit the actual ratio for
   every foreground/background pair and a `warnings` list when a pair lands under 4.5:1, so
   the claim is checkable.
3. **Both Tailwind eras ship.** v4 is the default (matches current upstream), but v3's bare
   `H S% L%` form is still what a large installed base pastes, so it is a first-class enum
   value rather than a footnote.
4. **Neutral family is a parameter, not a hard-code.** Upstream ships five base greys; pinning
   one would make the tool wrong for four out of five projects.
5. **Presets as chips, not as UI state.** Instead of a "randomize" button (out-of-model), the
   page ships `[[example]]` chips for the common starting points, which is the declarative
   preset mechanism this generator supports.
