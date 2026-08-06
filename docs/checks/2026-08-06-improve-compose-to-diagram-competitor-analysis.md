# compose-to-diagram competitor analysis — 2026-08-06

## Sources scanned

- Docker Compose Viz (Mermaid) web/GitHub page.
- Docker Compose Mermaid Generator VS Code marketplace listing.
- Infrasketch Docker Compose diagram tutorial and related compose visualization articles.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitors | Model fit | Decision |
| --- | --- | --- | --- |
| Paste or load Compose YAML and emit Mermaid | Dedicated Compose-to-Mermaid tools center on Mermaid text as the primary artifact | In model | Return raw Mermaid by default and add Markdown fenced output for README workflows. |
| Show services and dependency edges | All tools emphasize service relationships from `depends_on` | In model | Parse list and map `depends_on`, preserving `condition` labels. |
| Show networks | Visualizers group or link services by network | In model | Offer `subgraph`, `node`, and `off` network modes. |
| Show published ports | Architecture diagrams often expose ingress ports | In model | Draw port nodes and support common short and long Compose port syntax. |
| Show volumes | Infrastructure visualizers often mark persistent storage | In model | Draw named volumes and bind mounts with mount path labels. |
| Direction/layout controls | Mermaid-oriented tools let users choose graph direction | In model | Add `TD`, `LR`, `BT`, `RL` direction enum. |
| Markdown/README output | Tutorials focus on pasting into docs | In model | Add `output=markdown` as a fenced `mermaid` block. |
| Summary/audit mode | Diagram tools often supplement the drawing with issue hints | In model | Add `output=summary` with services and warnings for undefined targets, duplicate host ports, unused declarations, and cycles. |
| Render PNG/SVG export | Some tools export rendered images | Out of model | This repo's pure text page returns Mermaid; rendering/export belongs to Mermaid viewers or a future renderer. |
| Full Compose include/env resolution | Docker-aware tools may use the Docker Compose model after interpolation | Out of model | Do not execute Docker or read `.env`; leave `${VAR}` verbatim and document external `extends.file` as unresolved. |

## Defaults chosen

- `direction=TD`: matches Mermaid defaults and compact vertical diagrams.
- `networks=subgraph`: makes network grouping visible without adding many extra nodes.
- `ports=true`, `volumes=true`, `styled=true`: include architecture details by default.
- `labels=image`: service name plus image/build context is usually enough without overcrowding the diagram.
- `output=mermaid`: the most portable artifact for docs and Mermaid Live.

## Verification expectations

- Unit tests cover list/map `depends_on`, networks, ports, volumes, profiles, cycles, and YAML errors.
- CLI and page tests assert real Mermaid output, a non-default checkbox state, enum variants, and deep-link behavior.
- Hygiene checks ensure generated descriptor/manifest/page controls stay in sync.
