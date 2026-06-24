# color-shades-generator — competitor analysis (2026-06-23)

## The tool

`color-shades-generator` turns one base color (`#hex`, `rgb()`, or `hsl()`) into a
ramp of related colors in HEX, RGB and HSL. Modes:

- **scale** — Tailwind-style 50,100,…,900,**950** named ramp (11 weights), base
  pinned at its nearest weight and flagged `is_base`.
- **tints** — N steps lightening toward white.
- **shades** — N steps darkening toward black.
- **tones** — N steps desaturating toward neutral gray.

`count` (2–12) controls the series modes; `scale` always returns the 11 Tailwind weights.

Surfaces verified: chat block (`wafer build` validates + drift-guard schema test),
CLI (`gizza tool color-shades-generator`, all 4 modes + error), page (3 Playwright tests).

## Top competitors surveyed

1. **uicolors.app / generate** — Tailwind color generator; 50-950 (11 weights),
   HEX, clipboard export of a Tailwind config.
2. **tints.dev** — HSL-tweakable 11-color palette generator + API for Tailwind.
3. **tailwindcolorshades (javisperez)** — classic Tailwind shade generator from a
   base color.
4. **colorkit.co / color-shades-generator** — generic shades generator, multiple steps.
5. **ilovehue.co / shade-generator** — explicit **shades / tints / tones** distinction
   (the color-theory framing: shade = +black, tint = +white, tone = +gray).

## Gap diff + ranking (fit-to-model)

| Capability | Competitors | This tool (before) | Action |
|---|---|---|---|
| Tints / shades / tones distinction | ilovehue, generic tools | yes (3 modes) | already covered — a differentiator vs Tailwind-only tools |
| Tailwind named scale | all Tailwind tools | yes (scale mode) | covered |
| **50-950 (11 weights, incl. 950)** | uicolors, tints.dev (modern Tailwind v3.3+/v4) | **only 50-900 (10)** | **CLOSED** — added the `950` weight (target L≈0.11) |
| Base pinned at its weight | uicolors | yes (`is_base`) | covered |
| HEX + RGB + HSL output | most | yes | covered |
| Adjustable step count | colorkit | yes (2–12, series modes) | covered |
| Multiple input notations (#hex/rgb/hsl) | varies | yes | covered |

## Closed this round

- **Added the `950` weight** so `scale` now matches the modern Tailwind v3.3+/v4
  11-weight scale (50,100,…,900,950) instead of the older 10-weight (50-900) set.
  Updated core ramp + unit tests, chat/CLI schema description, manifest, page title,
  page copy, and the Playwright spec. This was the single most impactful in-model gap
  (every current Tailwind generator emits 950).

## Out-of-model / intentionally not built

- **OKLCH / perceptually-uniform output** (some competitors offer it as an algorithm
  toggle): would need a perceptual-color crate and a wider output schema. HSL ramps are
  the documented, widely-used standard and already produce a clean Tailwind-shaped scale;
  the marginal value doesn't justify the added dependency surface. Listed, not built.
- **One-click "copy as Tailwind config" / clipboard export**: a hosted-SPA UX affordance.
  The gizza page already renders the full ramp as selectable text and chat/CLI return it
  as structured JSON, both directly copyable, so the underlying capability is present.

## No-copy compliance

No competitor copy, branding, trademarks, or exact palette values were reproduced. The
50-950 weight set is the public Tailwind CSS scale convention (a de-facto standard), and
all lightness targets here are this tool's own values.
