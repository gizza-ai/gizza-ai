# shopping-list-builder — competitor analysis (2026-08-29)

Scan run **before** implementing `blocks/shopping-list-builder`. Everything below is
**paraphrased** from public marketing/FAQ copy and observed UI; no competitor copy, branding,
wording or trademarks are reproduced or reused. Out-of-model items are *listed*, not built.

## Duplicate check (done first)

- `blocks/recipe-extract` — a **network** tool: fetches ONE recipe page URL, reads its
  schema.org/Recipe markup, and renders that single recipe (title, ingredients, steps, times,
  nutrition) as markdown/text/json with an optional `scale`. It has no notion of *multiple*
  recipes, no cross-recipe merging, no unit-family summing and no grocery-category grouping.
  Complementary, not overlapping: `recipe-extract` is the "get one recipe" step; this tool is the
  "combine N ingredient lists into one shop" step.
- `blocks/list-dedupe-merge`, `blocks/chunk-list`, `blocks/list-converter`,
  `blocks/list-set-diff`, `blocks/file-list-sorter` — generic string-list set operations. None
  parse a quantity, none know units, none sum, none categorise.
- `blocks/unit-converter` — converts ONE value between units; no list, no aggregation.
- `docs/tool-skiplist.txt` has no `shop*` / `grocer*` / `ingredient*` entry.

**Verdict: not a duplicate — proceed.**

## Competitors reviewed

| # | Tool | Reachable | Model |
|---|------|-----------|-------|
| 1 | PlateAndShare — "Recipe to Shopping List Generator" | yes | paste ingredient lines + "recipe blocks", browser-side |
| 2 | Dessertisans — "Total Ingredient Calculator" | yes | paste one big recipe blob, live-updating totals |
| 3 | CalculatorGrid — "Grocery List Generator" | yes | manual item entry + prices/budget, category auto-sort |
| 4 | MyRecipeCart — "Grocery List Generator" (replacement for tool.world, which returned HTTP 404) | yes | paste recipe **URLs**, server fetches + merges |

FamilyPlate and ai-mealplan.com surfaced in search but are account/meal-plan products rather than
a single-purpose free tool, so they were used only as feature signal, not profiled.

### 1. PlateAndShare — Recipe to Shopping List Generator

- Input: free-text, "one ingredient per line", explicitly documented as `quantity unit name`.
- Multiple recipes are modelled as **recipe blocks** added one at a time; each block carries its
  own **serving multiplier** you tune before generating.
- Merging: duplicate ingredients merge and **matching units are summed** into one line.
- Categories: broad grocery buckets (produce / dairy / pantry / protein) chosen by **keyword
  matching**, with an explicit **Other** fallback for anything unmatched.
- Outputs: copy as plain text, print, export JSON, share link.
- No stated caps (no character/line/ingredient limit documented).

### 2. Dessertisans — Total Ingredient Calculator

- Input: a single "Full Recipe" textarea, pre-filled with a **worked sample** (a multi-component
  dessert: buttercream + meringue + pastry cream) so the tool shows output on load.
- Recomputes live on every keystroke.
- Ignores method/instruction prose and keeps only ingredient lines.
- Output: a flat bulleted list of `total quantity + unit + ingredient` (e.g. a grams total for
  milk and one for butter).
- **Documented limitation, stated plainly:** an ingredient only groups if the unit **and** the
  spelling match exactly — no unit conversion, no scaling, no servings control. Users are told to
  normalise their own text first.

### 3. CalculatorGrid — Grocery List Generator

- Input: item-by-item entry (name + price), plus meal-plan type and budget context.
- Auto-sorts into store sections (produce, dairy, frozen, pantry staples).
- Outputs: on-screen list, print, save-as-image, social share; reusable templates.
- Nine-question FAQ (how fast, what goes on a list, printing, saving money, app comparison,
  meal-plan integration, family sharing, shopping frequency, list ideas).
- Prices/budget are its differentiator; no quantity parsing or unit maths.

### 4. MyRecipeCart — Grocery List Generator

- Input: **recipe URLs** ("unlimited recipes at once"); server-side fetch + parse.
- Merges duplicates and totals quantities across recipes into one consolidated list.
- Marketed on breadth of supported recipe sites; free, no subscription.
- No documented category grouping, no servings control, no export detail.

## Table-stakes → in-model / out-of-model

| # | Table-stake (seen at ≥1 competitor) | Decision | Where it lands |
|---|---|---|---|
| 1 | Paste ingredient lines, one per line, `qty unit name` | **in-model** | `ingredients` (required, multiline) |
| 2 | Combine **several** recipes in one run | **in-model** | `# Recipe name` header lines + `---` separators inside `ingredients` |
| 3 | Merge duplicate ingredients into one line | **in-model** | normalised merge key (case, plural, prep note, parenthetical) |
| 4 | Sum quantities of merged items | **in-model** | per-unit-family base-unit summing |
| 5 | Per-recipe **serving multiplier** (PlateAndShare) | **in-model** | `# Pancakes x2` header suffix |
| 6 | Global scaling / servings change | **in-model** | `scale` param (slider on the page) |
| 7 | Category grouping by keyword with an **Other** fallback | **in-model** | `group_by = category` + built-in keyword table, 9 buckets |
| 8 | Group by recipe instead / flat list | **in-model** | `group_by = recipe \| none` |
| 9 | Plain-text copy output | **in-model** | `format = text` |
| 10 | Structured/JSON export | **in-model** | `format = json` |
| 11 | Spreadsheet-friendly export | **in-model** (gap at 3/4 competitors) | `format = csv` |
| 12 | Printable checklist | **in-model** | `format = markdown` + `checkboxes = true` (GFM `- [ ]`) |
| 13 | Live recompute as you type (Dessertisans) | **in-model** | the shared page runtime already reruns on input |
| 14 | Pre-filled worked sample so output shows on load | **in-model** | field placeholders + `[[example]]` preset chips |
| 15 | Show which recipe each item came from | **in-model** (nobody ships it; clear win) | `show_sources = true` |
| 16 | Skip pantry staples you already own | **in-model** (FamilyPlate-class feature) | `exclude` list + a preset chip |
| 17 | **Unit conversion** so `1 tbsp + 1 tsp` merges (Dessertisans' stated limitation) | **in-model — the headline differentiator** | volume + weight families, `unit_system = keep\|metric\|us` |
| 18 | Fetch recipes from URLs (MyRecipeCart) | **out-of-model here** | already covered by `blocks/recipe-extract`; pipe its output in |
| 19 | Prices, budget totals, cost tracking (CalculatorGrid) | **out-of-model** | needs a priced product catalogue / regional pricing feed |
| 20 | Save-as-image, social share buttons | **out-of-model** | site-repo/chrome concern, not a block capability |
| 21 | Saved templates / accounts / family sharing | **out-of-model** | gizza is no-account, no-server, browser-local |
| 22 | Store-aisle ordering for a *specific* chain | **out-of-model** | needs per-retailer planogram data |
| 23 | Nutrition totals across the shop | **considered, rejected** | needs a food-composition database; `recipe-extract` already surfaces per-recipe nutrition |
| 24 | Volume ↔ weight conversion (1 cup flour → 120 g) | **considered, rejected** | ingredient-density table; silently wrong for the long tail. We keep the two families separate and say so on the page. |

## Defaults chosen (and why)

| Param | Default | Rationale |
|---|---|---|
| `scale` | `1` | no surprise maths; competitors default to 1× |
| `group_by` | `category` | the aisle-grouped list is the whole point (3/4 competitors group) |
| `unit_system` | `keep` | least surprising — sums render in the unit the user mostly typed |
| `exclude` | *(empty)* | opt-in; nothing silently disappears from a shopping list |
| `checkboxes` | `false` | plain bullets read better in chat/CLI; chip turns it on |
| `show_sources` | `false` | keeps the default list scannable |
| `format` | `markdown` | matches the headed/bulleted shape every competitor renders |

## UX controls to match

- **Slider** for `scale` (competitors expose multipliers as a stepper/slider, not a raw box).
- **Friendly `<select>` labels** via `[input.labels]` for `group_by` / `unit_system` / `format`
  (canonical values stay machine-friendly for chat/CLI).
- **Preset chips** (`[[example]]`) — every competitor ships either a pre-filled sample or
  templates. Ours: a two-recipe week, a printable checklist with pantry staples skipped, a
  metric-converted list, and a CSV export.
- **Real placeholders** on the textarea and every text/number field, showing a runnable input.
- **Stated limits on the page** — Dessertisans states its matching limitation up front; we state
  ours (no volume↔weight, caps, prep-note handling) rather than letting users hit them.

## Limits shipped (stated on the page, enforced in core)

- 200 000 characters of input, 5 000 lines, 2 000 distinct merged items — each with a specific
  error message naming the cap and the actual count.
- `scale` clamped to 0.1 – 20 with an explicit error outside the range.
- Volume and weight never cross-convert (no density table).
- Quantity ranges (`3-4 cloves`) take the **upper** bound — you want enough to cook.
