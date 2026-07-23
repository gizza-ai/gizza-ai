# hl7-v2-parse — competitor analysis (2026-07-23)

New pure tool. Parses a pipe-delimited HL7 v2.x message into named segments, fields,
components and subcomponents (MSH, PID, OBX, …) and renders structured JSON or a flat
CSV/table. Scan done BEFORE implementing; all copy below is paraphrased — no competitor
copy, branding, or trademarks reproduced.

## Competitors scanned (top 3 + refs)

1. **hl7viewer.com — HL7 Viewer** — instant browser-local decode; per-segment/field
   breakdown with human-readable segment & field descriptions; "no data leaves your
   browser" privacy angle. Strong SEO on "HL7 viewer / message parser".
2. **hl7tools.io — HL7 Message Analyzer** — highlights each segment, field, component
   AND subcomponent; browser-based; also bundles FHIR tooling. Emphasises the full
   delimiter hierarchy visualization.
3. **majedynamics.com/tools/hl7-parser — MajeDynamics HL7 Parser** — deep segment
   analysis for MSH, PID, OBR, OBX and 30+ segments; **JSON and CSV export**; in-browser.
   Positions on breadth of named segments + export formats.

Refs: developers.do/tools/hl7-parser (message → structured JSON w/ segment+field
breakdown), parsehog.com/hl7/parser (view message contents), python-hl7 (library),
RedoxEngine/redox-hl7-v2 (schema-fied JSON library).

## Table-stakes → decision

| Capability | Competitors | Decision (this build) |
|---|---|---|
| Split segments → fields → components → subcomponents | all | **in-model** — full hierarchy in JSON |
| Repetition (`~`) handling | all | **in-model** — `repetitions` array |
| MSH offset (MSH-1 = field sep, MSH-2 = encoding chars) | all | **in-model** — special-cased |
| Custom delimiters read from MSH-1/MSH-2 | all | **in-model** — encoding chars parsed from MSH-2 |
| Named segment descriptions (MSH, PID, OBX, …) | hl7viewer, MajeDynamics | **in-model** — curated dictionary (~45 segments) |
| Named field descriptions | hl7viewer | **in-model, partial** — curated for common segments (MSH/EVN/PID/PV1/OBR/OBX/NK1/AL1/DG1/IN1); full HL7 field dictionary is thousands of entries → listed as out-of-model |
| JSON export | all | **in-model** — default output |
| CSV / flat table export | MajeDynamics | **in-model** — one row per non-empty leaf, `SEG.field[.comp[.sub]]` location |
| HL7 escape decoding (`\F\ \S\ \T\ \R\ \E\ \Xhh\ \.br\`) | viewers | **in-model** — `unescape` flag (default on) |
| 100% browser-local / no upload | all | **in-model** — inherent (wasm, no I/O) |
| FHIR conversion / validation | hl7tools | **out-of-model** — needs FHIR schema engine + mapping tables; separate tool |
| Full per-field HL7 data dictionary (every segment, every version) | hl7viewer | **out-of-model** — thousands of entries per HL7 version; curated common set shipped instead |
| Message editing / re-serialization / generation | Redox lib | **out-of-model** — this is a read/parse tool |

## Params shipped

- `data` (required) — raw HL7 v2.x message.
- `output` enum `json|csv` (default `json`).
- `include_descriptions` bool (default true) — attach segment/field names.
- `unescape` bool (default true) — decode HL7 escape sequences to literal characters.

UX: multiline paste field, enum `<select>` for output, checkboxes for the two flags,
three `[[example]]` preset chips (ADT→JSON, ORU/OBX→CSV, raw/no-descriptions), Reset +
Copy + Download (text format) provided by the platform.
