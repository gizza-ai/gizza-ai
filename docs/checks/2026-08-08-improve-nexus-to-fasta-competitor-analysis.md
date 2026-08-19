# nexus-to-fasta — competitor analysis (2026-08-08)

Scan run before implementation, per `create-next-tool` step 4. Notes are paraphrased; no competitor copy, branding, or trademarks were reused.

## Scope

Backlog row: *"Extracts the sequence matrix from a NEXUS file's DATA/CHARACTERS block and writes it out as FASTA."* Type hint: `pure`.

Dup check: there is no existing NEXUS parser block. `phylip-to-fasta` handles a different alignment grammar (`<ntaxa> <nchar>` header plus PHYLIP body). Existing FASTA/FASTQ tools do not read NEXUS blocks, comments, `dimensions`, `format`, `taxlabels`, or `matrix` commands.

## Competitors surveyed

Search: "NEXUS to FASTA converter online data characters matrix".

### 1. sciencecodons.com — NEXUS file/format converter
- Accepts pasted or uploaded NEXUS alignments and converts to FASTA and other alignment formats.
- Handles DATA/CHARACTERS matrix blocks and validates sequence-length consistency.
- Offers copy/download controls and file-cleanup assurances because conversion is server-side.
- Table-stakes surfaced: paste input, FASTA output, validation, download, local privacy messaging if possible.

### 2. alignment conversion web tools using BioPython / EMBOSS-style pipelines
- Convert among NEXUS, FASTA, PHYLIP, Clustal and Stockholm.
- Preserve taxon labels, gaps and missing-data characters.
- Usually expose input format, output format, alphabet/datatype and example buttons.
- Table-stakes surfaced: preserve alignment columns, support quoted taxon labels, document datatype as pass-through rather than adding no-op alphabet controls.

### 3. command-line conversion examples (`seqmagick`, BioPython AlignIO)
- Common offline route is a one-line command from NEXUS to FASTA.
- BioPython examples support comments, DATA/CHARACTERS, interleaved matrices, `matchchar`, and TAXA-block labels.
- Table-stakes surfaced: explicit worked example, clear failures for malformed/truncated NEXUS, and CLI-friendly exact output.

## Table stakes → decision

| Table stake | Fit | Where it lands |
| --- | --- | --- |
| Parse `#NEXUS` documents | in-model | required `nexus` multiline text param |
| DATA and CHARACTERS blocks | in-model | parser searches `begin data;` then `begin characters;` |
| Sequential and interleaved matrices | in-model | `layout = auto|sequential|interleaved`, default auto |
| `dimensions ntax=… nchar=…` validation | in-model | strict by default; `tolerant` downgrades mismatches |
| Preserve gaps and missing data | in-model | default output copies symbols |
| Strip gaps for unaligned FASTA | in-model | `remove_gaps` checkbox |
| FASTA line wrapping | in-model | `wrap` number field, default 60, `0` = one line |
| `matchchar=.` expansion | in-model | `expand_matchchar` checkbox default true |
| Non-default `gap=` symbols | in-model | parser reads `format gap=…` |
| Quoted labels and underscore convention | in-model | quoted labels supported; optional `underscores_to_spaces` |
| TAXA block labels with `labels=no` matrix | in-model | parser reads `taxlabels` |
| Example presets | in-model | page `[[example]]` chips |
| Copy/download output | in-model | shared text page runtime |
| File upload | out-of-model | pure pages use text fields; paste is supported |
| Server-side conversion/privacy text | out-of-model/better | local WebAssembly avoids upload entirely |
| Other output formats | out-of-scope | this slug is NEXUS → FASTA only |

## Considered, not built

- Multi-format conversion (PHYLIP, Clustal, Stockholm, JSON). This belongs in a broader alignment-format converter, not a single-direction slug.
- File upload / batch conversion. The pure page surface is a text form; multi-file input is not page-verifiable here.
- Datatype/alphabet dropdown. FASTA has no datatype declaration, so changing DNA/RNA/protein would not affect output; validation intentionally treats sequence symbols as pass-through alignment data.
- Tree/TREES block export. NEXUS can contain phylogenetic trees, but the backlog asks for sequence matrix extraction only.

## Shipped UX decisions

- Default auto-detection honours the NEXUS `format interleave` flag and otherwise tries both layouts, choosing the one that matches declared site counts.
- `expand_matchchar` defaults on because `.` usually means "same as first taxon" in NEXUS; a checkbox lets users preserve literal dots.
- `underscores_to_spaces` defaults off because many FASTA workflows prefer underscore-stable headers, even though NEXUS treats unquoted underscores as spaces.
- The page includes example chips for plain DATA, interleaved matchchar, quoted labels, and TAXA labels with `labels=no`.
