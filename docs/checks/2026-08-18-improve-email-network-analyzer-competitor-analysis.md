# email-network-analyzer — competitor analysis (2026-08-18)

Scan run **before** implementing, per the create-next-tool recipe. One web search
("email network analysis tool sender recipient communication graph mbox statistics")
plus direct reads of the top hits. All notes below are **paraphrased observations** —
no competitor copy, wording, branding, or asset is reproduced or reused anywhere in
this tool.

## Competitors reviewed

| # | Tool | Shape | Reachable |
|---|------|-------|-----------|
| 1 | DistrictDataLabs/tribe (GitHub) | Python CLI: mbox → GraphML for Gephi/NetworkX | yes |
| 2 | onlyphantom/emailnetwork (GitHub) | Python library: mbox → NetworkX graphs, header/domain counters, plots | yes |
| 3 | leodevbro/gmail-mbox-stats (GitHub/npx) | Node CLI: Gmail mbox → a folder of ranked CSV reports | yes |
| — | Promodag Reports (Exchange traffic analytics) | Commercial, server-side Exchange reporting suite; reachable but it reports on a live Exchange org's message-tracking logs, not on a pasted mailbox, so it is context only, not a profiled peer. |

## What they accept

- **tribe:** a single `.mbox` file path; `extract -w out.graphml in.mbox`. Documented
  on multi-GB mailboxes (minutes of runtime), so scale is a selling point.
- **emailnetwork:** an `.mbox` loaded through a reader object that can be filtered by
  date before any graph is built; individual messages can also be plotted alone.
- **gmail-mbox-stats:** two required arguments — the path to a Gmail mbox and *your own
  address*, which it uses to split every report into "mail I sent" vs "mail I received".

Nobody in the set accepts a **single pasted `.eml`** or a headers-only paste; all three
require a file on disk and a Python/Node toolchain. That is the gap this tool closes —
paste-in, browser-local, no install.

## Table-stakes capabilities (from the three profiles)

| Capability | Seen in | Our decision |
|---|---|---|
| Parse mbox into individual messages | 1, 2, 3 | **in-model — built** (`From ` postmark split; a lone `.eml` also parses) |
| Nodes = unique email addresses | 1, 2, 3 | **in-model — built** (`nodes=address`) |
| Edges = sender → recipient, weighted by message volume | 1, 2, 3 | **in-model — built** (edge weight = messages) |
| Directed *and* undirected views | 2 (explicit), 1 (implicit) | **in-model — built** (`direction`) |
| Include Cc (and Bcc) recipients as edges | 3 (counts Cc/Bcc separately) | **in-model — built** (`recipients=to \| to-cc \| to-cc-bcc`) |
| Top senders ranked by volume | 1, 2, 3 | **in-model — built** |
| Top recipients ranked by volume | 3 | **in-model — built** |
| Top correspondents / most-connected people | 2 ("key actors"), 3 | **in-model — built** (sent + received per person, plus degree) |
| "My" perspective: who I mail most / who mails me most | 3 (required `mymail` arg) | **in-model — built** (`me`, optional — adds a personal section and a reciprocity ratio) |
| Domain-level rollup / domain frequency | 2 (domain summary), 3 (sender-domain reports) | **in-model — built** (`nodes=domain` collapses every address to its domain) |
| Date-range filtering before graphing | 2 | **in-model — built** (`since` / `until`, inclusive, ISO dates) |
| GraphML export for Gephi/NetworkX | 1, 2 | **in-model — built** (`format=graphml`, weighted, directed flag honoured) |
| CSV export of ranked tables | 3 | **in-model — built** (`format=csv` → an edge list with weights and first/last dates) |
| Ranked top-N truncation | 3 (ranks everything) | **in-model — built** (`top`, 1–100, default 10) |
| Edge-weight threshold (drop one-off contacts) | — (none document one) | **in-model — built** (`min_messages`) — standard graph hygiene, cheap to add |
| Drop self-loops (you Cc'ing yourself) | — (none document one) | **in-model — built** (`self_loops`, default off) |
| Exclude noreply/automated senders | — (none document one) | **in-model — built** (`exclude`, comma-separated address/domain/substring list) |
| Graphviz DOT export | — | **in-model — built** (`format=dot`) — a second, install-free way to render the graph |
| Structured JSON of nodes + edges + summary | 2 (via NetworkX objects) | **in-model — built** (`format=json`) |

## Out of model (listed, not built)

- **Rendered network plots** (spring/shell layouts, matplotlib PNGs — #2). This block
  is a text/graph-data tool; the page renders text. The DOT and GraphML exports hand
  the drawing off to Graphviz/Gephi instead.
- **Centrality metrics** (betweenness, eigenvector, community detection — #1/#2 lean on
  NetworkX for these). Degree and volume are computed here; full centrality on an
  arbitrary graph is a different tool (`graph-algorithms` already exists in this repo).
- **Attachment-size rankings** (#3 ranks correspondents by total attachment MB). Would
  require decoding every MIME part of a whole mailbox in a 64 MiB sandbox; a paste-sized
  tool cannot carry multi-GB mailboxes, and this is a volume tool, not a storage tool.
- **Multi-GB mailbox files** (#1 advertises 7.5 GB exports). Input is a paste/CLI string,
  capped well below that; the caps are stated on the page.
- **Live Exchange / IMAP connections and saved dashboards** (Promodag). No accounts, no
  network access, no server-side state in this repo's model.
- **Thread reconstruction** (`In-Reply-To`/`References` chains). Not a network-graph
  feature, and reply-chain tooling is a separate backlog shape.

## UX control patterns worth matching

- #3's required "your address" argument is the single most useful affordance in the set —
  it turns a symmetric graph into a personal report. Kept, but **optional** so the tool
  still works on a mailing-list archive with no obvious owner.
- #2 exposes date filtering *before* graph construction; mirrored with `since`/`until`
  rendered as native date pickers on the page.
- #1's only output is GraphML; giving `report` (readable) as the default with GraphML,
  DOT, CSV, and JSON alongside covers both the "read it now" and "load it into Gephi"
  audiences.
- None of the three ship preset examples. The page adds `[[example]]` chips (a small
  mbox thread, a domain-level view, a GraphML export) so the tool is usable without
  first finding an mbox.

## Copy / branding

No competitor text, screenshots, naming, or marks are reused. The page copy, FAQ, and
parameter descriptions are written for this tool.
