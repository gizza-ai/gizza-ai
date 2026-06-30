# risk-matrix — competitor analysis & surface checks (2026-06-29)

**Tool:** `risk-matrix` — turn a risk register (`name, likelihood, impact`) into a colored likelihood × impact SVG matrix with green/amber/red zones and a numbered legend. Pure Rust SVG generation; output is `image/svg+xml`.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core unit tests | `cargo test --workspace` in `blocks/risk-matrix/` | ✅ 7 core + 1 drift-guard schema test pass |
| Chat block | `wafer build` in `blocks/risk-matrix/` | ✅ OK, 322.8 KiB, instantiates |
| Page wasm | `wasm-pack build .../web` | N/A — SVG image-bytes tool has no standalone page/web crate |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ generator completed; no page rendered for this no-page image tool |
| CLI | `gizza tool risk-matrix items=… title='Project Risks'` | ✅ wrote `Project-Risks.svg`; SVG starts with `<svg` and includes supplied title/items |
| Playwright page | page spec | N/A — no standalone page for image-byte output tools |

## Competitor landscape

Common ways teams create risk matrices:

1. **Spreadsheets (Excel / Google Sheets templates)** — ubiquitous, editable grids with conditional colors, but manual formatting and sharing overhead.
2. **Project-management risk register templates** — often include likelihood/impact columns and a matrix view, usually tied to an account or workspace.
3. **Cybersecurity GRC/risk tools** — polished matrices and register workflows, but server-backed and not a lightweight local utility.
4. **Diagramming tools (Lucidchart, Miro, draw.io)** — visually flexible, but require manually placing each risk and updating labels.
5. **Python/R notebooks / matplotlib snippets** — scriptable and reproducible for technical users, but not convenient for ad-hoc planning.

## Capability diff

| Capability | Competitors | gizza risk-matrix |
| --- | --- | --- |
| 5×5 likelihood-impact matrix | templates, GRC tools | ✅ default |
| Custom matrix size | some scripts/templates | ✅ 2..10 |
| Risk register paste input | spreadsheets/templates | ✅ one line per item |
| Named items in each cell | all serious tools | ✅ numbered markers + legend |
| Multiple items per cell | spreadsheets/GRC tools | ✅ stacked marker layout |
| Green/amber/red zones | all | ✅ threshold-driven bands |
| Adjustable band thresholds | scripts/some tools | ✅ `amber_at`, `red_at` |
| Custom likelihood/impact labels | templates/GRC tools | ✅ comma-separated axis labels |
| Export/share artifact | diagram tools/templates | ✅ standalone SVG file |
| Local/private/no account | spreadsheets/offline scripts | ✅ browser/chat/CLI local wasm |

## In-model gaps closed / confirmed

The shipped surface focuses on the common static risk-matrix deliverable: users paste a small risk register, get a clean SVG with risk bands, axis labels, markers, and a legend. It avoids requiring a stateful spreadsheet/grid UI while still supporting the most important planning knobs: matrix size, custom labels, and band thresholds.

## Out-of-model / intentionally not built

- Full editable spreadsheet/risk-register workflow: stateful editing, sorting, ownership, and history are outside the single-shot gizza tool model.
- Accounts, collaboration, approvals, and audit trails: useful in GRC systems, but require a backend and user management.
- Drag-and-drop marker placement: this tool derives placement from likelihood/impact ratings instead.

No competitor copy, branding, or trademarks were used.
