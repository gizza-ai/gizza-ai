# render-jinja2 — competitor analysis (2026-06-22)

Tool: `blocks/render-jinja2` — render a Jinja2 (Jinja) template against supplied
JSON or YAML data, in the browser (chat / CLI / page).

## Surfaces verified

- **chat block** — `wafer build` OK (1533.6 KiB), minijinja 2.21 + serde_yml 0.0.12 instantiate in wasm32-wasip1.
- **CLI** — `gizza tool render-jinja2 …` verified for: simple var, `{% for %}` loop, YAML data, strict-mode missing-var error (exit 1).
- **page** — Playwright `tool-page-render-jinja2.spec.ts`: 4/4 pass (variables, loop+conditional, YAML+filter via `<select>`, strict-mode error).
- **unit** — 14 core tests + descriptor drift-guard, all green.

## Top competitors surveyed

1. **j2live (j2live.ttl255.com)** — online Jinja2 renderer (Python Jinja2 backend). Inputs: template + YAML/JSON variables; toggles for trim_blocks/lstrip_blocks/keep_trailing_newline and undefined behavior (default/strict/debug).
2. **Jinja Live Parser / cryptic.io jinja2** — template + JSON context, single render box.
3. **onlineyamltools / templating playgrounds (Nunjucks playground)** — JS Nunjucks (a JS Jinja-alike), template + JSON, live preview.
4. **CyberChef "Jinja"-style / general templating** — not a true Jinja2 engine; Handlebars/Mustache only (we already ship `render-template` for that).
5. **Local CLI `jinja2` (jinja2-cli pip)** — `jinja2 tmpl.j2 data.yaml` → stdout; supports json/yaml/ini/env data formats and `--strict`.

## Capability diff (us vs. competitors)

| Capability | render-jinja2 | j2live | jinja2-cli | Nunjucks playground |
|---|---|---|---|---|
| `{{ var }}` substitution, nested paths | yes | yes | yes | yes |
| `{% for %}` / `{% if/elif/else %}` | yes | yes | yes | yes |
| Filters (`upper`, `join`, `round`, …) | yes (minijinja builtins) | yes | yes | yes |
| JSON data | yes | yes | yes | yes |
| YAML data | yes | yes | yes | no |
| Auto-detect JSON vs YAML | yes | no (explicit toggle) | no (by ext) | no |
| Strict / undefined-error mode | yes | yes | yes | partial |
| Runs fully local / private (no upload) | yes (wasm) | no (server) | local | local |
| INI / env data formats | no | no | yes | no |
| whitespace-control toggles (trim_blocks/lstrip_blocks) | partial (minijinja `{%- -%}` in-template) | yes (env flags) | yes | yes |

## Gaps + decisions

- **JSON + YAML covered** — matches the strongest web competitors; added auto-detect, which most do not have. Closed.
- **Strict mode** — present (`UndefinedBehavior::Strict`). Closed.
- **Filters / loops / conditionals / expressions** — minijinja `builtins` feature provides the standard Jinja filter/test set; verified upper/join in tests. Closed.
- **INI / env data formats (jinja2-cli)** — out of scope / low value for a browser tool; JSON+YAML cover the common cases. Not built (would need extra parsers; marginal benefit).
- **Environment whitespace toggles (trim_blocks / lstrip_blocks)** — minijinja exposes per-template whitespace control via the `{%-`/`-%}` markers inside the template, which covers the common need without extra params. A global env toggle is a possible future enhancement; left out to keep the param set small. Documented in-page.
- **No competitor copy/branding/trademarks were used** in title, description, tags, or page content — all original.

## Out-of-model / not built

- INI / TOML / env data formats (extra parsers, marginal benefit for the web use-case).
- Global whitespace-control env flags (covered in-template by `{%- -%}`).
- Template includes / `{% extends %}` (would need a multi-template upload surface; single-template tool by design).
