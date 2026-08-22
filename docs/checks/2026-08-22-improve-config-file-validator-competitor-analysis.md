# config-file-validator competitor analysis (2026-08-22)

Backlog tool: `config-file-validator` — validate pasted JSON, YAML, TOML, INI, or XML config syntax and report line/column diagnostics.

## Competitor scan

Search query: `online config file validator JSON YAML TOML INI XML syntax line column`.

### 1. Format-specific TOML validators

Observed table stakes:
- A large paste box for the config text.
- A validate action that reports whether syntax is valid.
- Invalid syntax includes a parser message and line/column information.
- Some tools parse to JSON for easier inspection.

Fit decision:
- In model: paste text input, explicit format selector, line/column diagnostics, human report and JSON diagnostic output.
- Out of model: rich parsed-tree viewer. This gizza block focuses on syntax diagnostics rather than an interactive AST browser.

### 2. YAML validator / YAML lint pages

Observed table stakes:
- Paste YAML and validate it without setup.
- Report YAML parser errors with line context.
- Examples commonly cover indentation, missing colons, tabs and malformed sequences.
- Some tools support loading from a URL.

Fit decision:
- In model: YAML parser, multi-document YAML streams, targeted hints for indentation/tabs/missing colon, context lines around errors.
- Out of model: URL loading, because gizza page tools should validate supplied text locally and avoid network fetching for private configs.

### 3. General configuration file validators

Observed table stakes:
- A format selector or auto-detection.
- Support for several config formats in one UI.
- Summary of valid/invalid status plus details per issue.
- Optional best-practice/security checks.

Fit decision:
- In model: auto/json/yaml/toml/ini/xml selector, valid/invalid summary, strict warnings for portable syntax issues, JSON output for automation.
- Out of model: product-specific security and schema validation for Nginx, Apache, Docker Compose or Kubernetes. Those need domain schemas and policy rules beyond syntax parsing.

## Descriptor and UX decisions

Implemented in-model controls:
- `input` textarea for pasted config text.
- `format` enum: `auto`, `json`, `yaml`, `toml`, `ini`, `xml`.
- `strict` checkbox for duplicate keys and portability warnings.
- `report_format` enum: `report`, `json`.
- `context_lines` integer from 0 to 10.

Examples/presets cover:
- YAML indentation error.
- JSON trailing comma.
- Valid TOML machine-readable output.
- XML unclosed tag.

Limits documented:
- 1 MiB input cap.
- 100 diagnostic cap.
- syntax validation only, not product-specific schema validation.
- auto-detect may prefer a stricter format when snippets overlap.
