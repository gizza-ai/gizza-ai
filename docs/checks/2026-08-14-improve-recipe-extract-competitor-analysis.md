# recipe-extract competitor analysis (2026-08-14)

Tool: `recipe-extract` — fetch a public recipe page and return clean structured recipe data: ingredients, steps, time, yield and nutrition, without blog filler.

## Sources checked

| Competitor | Notes observed | In-model decisions for this block | Out-of-model / not built |
| --- | --- | --- | --- |
| RecipeExtractor.com | URL input; extracts ingredients, steps, cook times and servings; also offers save/share/print, cookbook, shopping lists, translation, substitutions and video/social links. | Required URL param; Markdown/text/JSON outputs; ingredients, instructions, times, yield, source and optional nutrition. | Accounts, cookbook, shopping list, translation, substitutions, video/social extraction and share links require app storage, UI workflows, AI/video models, or private site features. |
| RecipeStripper | Explains JSON-LD and microdata as primary sources; mentions fallback tiers, clear bot-protection errors, ingredient quantities, steps, servings scaler and cook mode. | Implement JSON-LD, `@graph`, array `@type`, HowToStep/HowToSection, microdata fallback, clear no-markup/stub errors, ingredient scaling, nutrition toggle and download-size cap. | Heuristic HTML parser, AI fallback, inline ingredient matching inside each step, bot bypass and cook mode are outside the current pure-Rust/network block scope. |
| Drizzlelemons recipe extractor | URL/name input; clean ingredients/instructions/time/servings/image/source; scaling, unit conversion, personal collections, step-by-step cook mode and keep-awake controls. | Output includes title, author, source, image, ingredients, steps, prep/cook/total time, yield, format enum and ingredient scale control. | Dish-name generation, account collections, unit conversion, AI customization, step timers and keep-awake are app/model features rather than this CLI/chat block. |

## Table-stakes mapped to implementation

| Capability / UX pattern | Decision |
| --- | --- |
| Paste a recipe URL | `url` is required and validated as absolute `http://` or `https://`. |
| Remove blog story, ads, comments | Parser reads schema.org `Recipe` data only (JSON-LD first, microdata fallback). |
| Ingredients and numbered steps | Core extracts `recipeIngredient` and `recipeInstructions`, including `HowToStep` and `HowToSection`. |
| Time and yield | Extracts prep/cook/total ISO durations and humanizes them; numeric yield becomes servings. |
| JSON/structured output | `format=json` returns the structured record; `markdown` and `text` are cook-readable surfaces. |
| Scale servings / quantities | `scale` multiplies leading ingredient quantities, including fractions, mixed fractions, unicode fractions and ranges. |
| Nutrition visibility | `include_nutrition` boolean defaults true and can suppress nutrition. |
| Clear failures | Errors distinguish no Recipe markup from markup with no ingredients/steps; `max_bytes` rejects overly large pages instead of truncating. |
| Browser page controls | Not applicable: this is a network block requiring `wafer-run/network`; a browser WASM page cannot fetch arbitrary third-party recipe URLs through CORS. |

## Verification focus

The implementation should be verified with native unit tests for JSON-LD, microdata, sectioned instructions, exact Markdown/text/JSON rendering, validation failures and quantity scaling; a canonical `target/block.wasm`; CLI descriptor/manifest sync; and at least one CLI surface check for schema/help or execution behavior. Live recipe-page success can vary due bot protection, so failures should stay explicit and non-partial.
