# dicom-anonymizer — competitor analysis (2026-07-23)

Function: strip patient-identifying data elements (name, ID, dates, institution,
physicians, private tags) from a DICOM Part-10 file and return a sanitized copy
safe to share. Surfaces here: **chat + CLI** (file-in → binary DICOM out). No page
(binary DICOM has no in-browser render mode — same shape as the sibling
`dicom-to-image`, `blur-image`, `svg-to-png`).

## Scan (one web search; top real tools skimmed, paraphrased — no copy/branding)

Surveyed: a review roundup of free DICOM de-identification tools plus several
browser-based "remove PHI locally, no upload" anonymizers (X-ray Interpreter,
CT Read, ReadYourLab), a clinical-grade desktop family (CTP-style profile tools),
and an AI-pixel-masking service. Common ground:

| Capability / param | Typical default | Fit to gizza model |
|---|---|---|
| Remove patient identity tags (name, ID, birth date/time, sex, age, address, phone, other IDs) | always on | **in-model** — blank/replace the PS3.15 patient-module tags |
| Remove physician / operator / institution tags (referring/performing physician, institution name+address, station, dept) | always on | **in-model** — blank |
| Remove study/series descriptions and selected date-like direct identifiers | on | **in-model** — covered by the common-PHI tag set; pixel data is untouched |
| Remove all private (odd-group) tags (vendor PHI often hidden there) | on | **in-model** — `profile=strict` |
| Replace text values with a placeholder (vs blank) so viewers still open the file | "ANON" / dummy | **in-model** — `placeholder` param, padded to original length |
| Mark the file de-identified: PatientIdentityRemoved (0012,0062)=YES + DeidentificationMethod (0012,0063) | set automatically | **out-of-model for v1** — adding new tags would require length/offset rewriting; v1 preserves byte offsets by in-place overwrite only |
| Client-side / no upload / privacy | core selling point | **matches** — runs locally in the CLI / wasm sandbox, no network |
| Custom per-tag edit (choose any tag to keep/replace) | GUI tag editor | **out-of-model** — a single tool call takes a fixed profile, not an arbitrary tag map (listed, not built) |
| Batch anonymization (whole study/folder) | drag many files | **out-of-model** — one file per call (list it) |
| UID remapping / consistent re-hashed StudyInstanceUID | optional | **out-of-model** for v1 — UIDs are kept unchanged so the file stays valid and internally consistent (MediaStorage vs SOP Instance UID match); regenerating a consistent UID map across a study needs cross-file state (listed) |
| AI pixel masking / burned-in annotation removal (ultrasound overlays) | premium | **out-of-model** — needs vision/ML; gizza is pure-Rust + ffmpeg (listed) |

## Decisions (all table-stakes accounted for — built or explicitly listed)

Built into the descriptor:
- `profile` enum (`basic` default, `strict`) — `basic` wipes the common direct
  identifier tags; `strict` additionally wipes private odd-group elements.
- `placeholder` string (default `ANON`) — copied into redacted text fields and
  padded/truncated to the original element length so offsets never change.

Automatic behaviour: overwrite the common patient-demographics + physician /
institution + StudyID / AccessionNumber / study-description tag set in place;
preserve PixelData and all lengths/offsets byte-for-byte; recurse into explicit
VR sequences and undefined-length sequence items so nested copies of identifying
tags are scrubbed where the stream is parseable without a full DICOM dictionary.

Out-of-model (listed above, not built): arbitrary per-tag editing, batch/folder
runs, cross-file UID remapping, adding new de-identification tags without changing
lengths/offsets, AI/pixel burned-in-annotation masking.

Known limits (stated on the tool / in errors):
- Only DICOM Part-10 files (128-byte preamble + `DICM`) in Implicit or Explicit
  VR Little Endian transfer syntax. Big-endian / deflated syntaxes error clearly.
- Pixel data (including encapsulated/compressed) is preserved byte-for-byte — this
  tool scrubs the header, it does NOT touch burned-in pixel annotations.
- Nested identifying tags inside an **Implicit-VR defined-length** sequence are not
  recursed (the stream is ambiguous without a data dictionary); explicit-VR and
  undefined-length sequences are recursed. Top-level tags are always scrubbed.

No competitor copy, branding, or trademarks reproduced.
