## What this tool does

Enter a **foreground (text)** colour and a **background** colour and this tool
computes their WCAG 2.x contrast ratio and tells you whether the pair passes the
accessibility thresholds for body text, large text, and UI components. Nothing is
sent to a server — it runs locally in your browser, works offline, and needs no
sign-up.

Colours can be **hex** (`#1a2b3c`, `#fff`, or a bare `1a2b3c`), an **rgb** triple
(`rgb(26, 43, 60)` or `26,43,60`), an **hsl** triple (`hsl(210, 40%, 17%)`), or a
**CSS colour name** (`navy`, `tomato`, `rebeccapurple`).

## How the ratio works

The contrast ratio is computed from each colour's WCAG *relative luminance* as
`(L_lighter + 0.05) / (L_darker + 0.05)`. It ranges from **1:1** (two identical
colours) to **21:1** (black on white) and is independent of which colour you put
first.

## WCAG thresholds

| Use case | AA | AAA |
| --- | --- | --- |
| **Normal text** (under ~18pt / 14pt bold) | 4.5:1 | 7:1 |
| **Large text** (~18pt+, or 14pt+ bold) | 3:1 | 4.5:1 |
| **UI components & graphics** (icons, borders, focus rings) | 3:1 | — |

A pair "passes AA" for normal text when its ratio is at least 4.5:1, and "passes
AAA" at 7:1 or higher. Large text and non-text UI elements only need 3:1.

## Output formats

- **text** (default) — a readable report listing the ratio and a Pass/Fail for
  each level.
- **json** — a compact object (`ratio`, `aa_normal`, `aa_large`, `aaa_normal`,
  `aaa_large`, `ui_components`, `summary`) you can drop into a build or a script.
- **suggest** — the full report **plus** a nearby accessible colour. When your
  foreground fails, it finds the closest colour with the same hue and saturation
  (only the lightness is nudged) that reaches the **Suggest target** you pick.

## Suggesting an accessible colour

Set the **Output format** to `suggest` and choose a **Suggest target**:

| Target | Reaches |
| --- | --- |
| **aa** (default) | 4.5:1 — AA for normal text |
| **aaa** | 7:1 — AAA for normal text |
| **large** | 3:1 — AA for large text and UI components |

The tool keeps your colour's hue and saturation and only shifts its lightness, so
the suggestion stays on-brand while clearing the threshold. If no shade of that
hue can reach the target against your background, it says so — change the
background instead.

## Examples

| Foreground | Background | Ratio | Normal AA | Normal AAA |
| --- | --- | --- | --- | --- |
| `#000000` | `#ffffff` | 21:1 | Pass | Pass |
| `#767676` | `#ffffff` | 4.54:1 | Pass | Fail |
| `#aaaaaa` | `#ffffff` | 2.32:1 | Fail | Fail |
| `#ffffff` | `#1a73e8` | 4.5:1 | Pass | Fail |

## FAQ

**Is it free and private?** Yes — your colours never leave your device, and it
keeps working offline once the page has loaded.

**What counts as "large" text?** WCAG defines large text as roughly 18pt (24px)
and up, or 14pt (about 18.66px) and up when bold. Large text only needs a 3:1
ratio for AA.

**Does the order of the two colours matter?** No. The ratio is symmetric — the
tool always divides by the darker colour's luminance, so swapping foreground and
background gives the same number.

**Which WCAG version is this?** The 4.5 / 3 / 7 thresholds and the luminance
formula are shared by WCAG 2.0, 2.1, and 2.2.
