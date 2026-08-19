# phylip-to-fasta — competitor analysis (2026-08-08)

Scan run **before** implementation, per `create-next-tool` step 4. All competitor notes are
**paraphrased** — no competitor copy, branding, or trademarks were reused. Out-of-model items
are listed, never built.

## Scope

Backlog row: *"Converts PHYLIP sequential or interleaved multiple-sequence-alignment files into
standard FASTA."* Type hint: `pure`.

Dup check: `ls blocks/ | grep -iE 'fasta|phylip|seq|bio|dna|align|nexus'` → only
`blocks/fastq-to-fasta`. That block parses **FASTQ** (4-line reads: `@id` / sequence / `+` /
per-base Phred quality) and its whole option surface is quality-oriented (`min_quality`,
`quality_offset`, `strip_n`). PHYLIP is a completely different grammar — a `<ntaxa> <nchar>`
count header, fixed-width or whitespace-delimited taxon names, and a sequential **or**
interleaved block layout with no quality data at all. No overlap in parser, params, or user
intent; not a duplicate.

## Competitors surveyed

Search: "PHYLIP to FASTA converter online alignment format sequential interleaved".

### 1. sciencecodons.com — PHYLIP file/format converter
- Accepts both interleaved and sequential PHYLIP variants.
- Output targets beyond FASTA (Nexus, Clustal, JSON).
- Input validation that flags inconsistent sequence lengths and invalid characters.
- Batch mode over several alignments; a molecule-type dropdown; header customisation.
- Upload (drag-and-drop) **or** paste; convert / clear-all / copy / download buttons.
- Server-side: files uploaded over TLS, deleted after processing; REST API for pipelines.
- No worked example on the page; no stated size limits.

### 2. punnettsquare.org — alignment format converter
- Converts among FASTA, Clustal, PHYLIP, NEXUS and Stockholm.
- Explicitly promises gap characters survive the round trip.
- Paste-or-upload input, an output-format dropdown, and copy / clear / download buttons.
- An **Example** button that fills the box with sample data.
- Shows one worked input→output pair in the page copy.
- FAQ covers "which formats are accepted" and "are gaps preserved".

### 3. sequenceconversion.bugaco.com — Phylip → Fasta
- Fixed direction (phylip in, fasta out) with format dropdowns pinned to those defaults.
- An alphabet selector (none / DNA / RNA / protein / nucleotide) that only annotates records.
- File upload; conversion runs server-side on a BioPython backend.
- States the real PHYLIP gotcha plainly: **strict PHYLIP truncates taxon names at 10
  characters**.
- Also documents a local BioPython one-liner as the offline alternative.

## Table stakes → decision

| Table stake (seen at ≥1 competitor) | Fit | Where it lands |
| --- | --- | --- |
| Parse **sequential** PHYLIP | in-model | `layout = "sequential"` |
| Parse **interleaved** PHYLIP | in-model | `layout = "interleaved"` |
| Not making the user know which they have | in-model | `layout = "auto"` (default) — validated against the header's `nchar` |
| Strict 10-column names vs relaxed whitespace names | in-model | `name_style = "auto" \| "strict" \| "relaxed"`, default `auto` (competitor #3 documents the 10-char truncation but offers no relaxed mode) |
| Validate declared vs actual taxa/site counts | in-model | on by default; errors name the taxon and both numbers |
| Detect invalid sequence characters | in-model | validated against `A–Z a–z 0–9 - . ? * ~` |
| Tolerate slightly-off files instead of hard-failing | in-model | `tolerant` checkbox (downgrades the count/length/character checks) |
| Preserve gap characters | in-model | default behaviour — gaps pass through untouched |
| Optionally strip gaps (align → unaligned FASTA) | in-model | `remove_gaps` checkbox (`-` and `.`) |
| FASTA line wrapping | in-model | `wrap`, default `60` (the conventional FASTA width), `0` = one line |
| Case normalisation | in-model | `uppercase` checkbox |
| Paste input | in-model | multiline field |
| Copy / download / reset result | in-model | provided by the shared page runtime |
| One-click example data | in-model | four `[[example]]` preset chips |
| Worked input→output example + FAQ | in-model | `page/content.md` |
| Stated limits | in-model | limits section on the page |
| Runs without upload | **better than all three** | fully local wasm; competitors #1 and #3 are server-side |

## Considered, not built (out-of-model)

- **Other output formats** (NEXUS, Clustal, Stockholm, JSON). A different tool's job, not this
  slug's; folding a format matrix into a `phylip-to-fasta` descriptor would make the chat/CLI
  schema ambiguous.
- **File upload / drag-and-drop of `.phy` files.** Pure blocks take a text field; the page's file
  source is wired to the ffmpeg runtime. Paste is the supported path (and the page says so).
- **Batch mode over many alignment files.** Needs a multi-file input surface the page model
  doesn't have (same class as the multi-input ffmpeg limitation in
  `references/page-patterns.md`).
- **REST API / pipeline automation.** Out of scope here — the CLI (`gizza tool phylip-to-fasta …`)
  and the block's chat/MCP schema already cover scripted use, locally.
- **Molecule-type / alphabet selector.** FASTA carries no alphabet declaration, so the setting
  would change nothing in the output. Deliberately omitted rather than shipped as a no-op.
- **Server-side storage guarantees (TLS upload, auto-delete).** Moot: nothing is uploaded.

## Considered, rejected (in-model but declined)

- **Custom header templates** (competitor #1's "headers with taxon names, positions, or
  metadata"). PHYLIP carries no metadata beyond the taxon name, so a template language would
  have exactly one substitutable field. Rejected as schema bloat; the taxon name is emitted
  verbatim.
- **Renaming/numbering headers.** `fastq-to-fasta` needs it because FASTQ read ids repeat; PHYLIP
  taxon names are the alignment's identity and silently replacing them would break downstream
  tree building.

## Auto-detection rules shipped (documented on the page)

- `layout = auto`: blank-line-separated blocks whose first block holds exactly `ntaxa` lines →
  interleaved; exactly `ntaxa` data lines total → sequential; otherwise both parses are attempted
  and the one whose sequences match the header's `nchar` wins.
- `name_style = auto`: relaxed (first whitespace-delimited token) is tried first, then strict
  (first 10 columns); the parse that matches `nchar` wins. This is what lets a RAxML/PhyML-style
  file with >10-character names and a classic strict file both work with no user input.
- Repeated taxon names in interleaved continuation blocks (some writers emit them) are detected
  and stripped.
