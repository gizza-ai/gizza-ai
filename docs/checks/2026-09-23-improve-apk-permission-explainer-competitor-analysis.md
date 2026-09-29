# Competitor analysis: apk-permission-explainer (2026-09-23)

## Scope

Tool goal: extract permissions from an uploaded Android APK or AndroidManifest.xml and explain each Android permission in plain English, highlighting dangerous and privacy-sensitive entries. The gizza implementation must stay fully local, pure Rust/WASM, and compatible with the existing text/page/CLI model.

## Competitor scan

1. Android Studio APK Analyzer / `aapt dump permissions`
   - Table stakes: reads the binary manifest from an APK, lists requested permissions, and exposes package / SDK metadata.
   - UX patterns: developer-oriented raw lists, no privacy explanation, no browser-local single-purpose page.
   - In-model: APK ZIP read, binary manifest decode, package and SDK metadata, deterministic permission list.
   - Out-of-model: full APK resource inspection, DEX/class analysis, IDE tree views.

2. Exodus Privacy / tracker-and-permission reports
   - Table stakes: groups Android permissions into privacy-impacting categories and pairs them with explanations users can understand.
   - UX patterns: risk labels, summary counts, human-readable explanations, distinction between permissions and trackers.
   - In-model: risk categories, short explanations, counts by category, preservation of unknown/vendor permissions.
   - Out-of-model: remote database of tracker signatures, historical app-store reports, behavioral scoring.

3. Online APK permission checkers and Play-console permission declarations
   - Table stakes: accept an APK/manifest, display dangerous permissions prominently, and provide exportable lists for audits.
   - UX patterns: filters for dangerous/risky permissions, compact list and CSV/JSON export options, examples using decoded manifests.
   - In-model: output format selector, risk filter, sort control, worked manifest examples, CSV/JSON output.
   - Out-of-model: drag-and-drop binary upload on this generic text-only page, store-policy compliance questionnaires, signing certificate trust checks.

## Design decisions

- Input model: text field accepting Base64 APK bytes, Base64 manifest bytes, or decoded manifest XML. This fits the current page and chat model without adding a custom binary upload surface.
- Core parser: pure Rust ZIP extraction plus local AXML decoding, with plain XML support for examples and tests.
- Permission knowledge: bundled table of common Android permissions with categories: dangerous, privacy-sensitive, signature/system, normal, and unknown/app-defined.
- Controls: `mode` enum for report/list/CSV/JSON, `risk` enum for filters, and `sort` enum for risk/name ordering.
- Output: Markdown report by default, with summary counts, package/SDK metadata, permission descriptions, SDK-23/max-SDK notes, and declared custom permissions.

## Out-of-model / deferred

- Tracker SDK detection from DEX strings belongs to a separate APK tracker scanner.
- Full APK resource/icon inspection belongs to a separate APK info/icon tool.
- Store policy compliance and malware verdicts require external policy data or behavior analysis and are intentionally not claimed.
- File-picker upload UX would require page-generator support beyond the current generic text field.
