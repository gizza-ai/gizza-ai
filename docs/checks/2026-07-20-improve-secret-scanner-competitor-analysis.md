# secret-scanner — competitor analysis (2026-07-20)

Tool function: paste a code/config/text snippet and flag hardcoded secrets — provider API keys,
tokens, and private-key headers — using known provider prefix patterns plus a generic
keyword+entropy heuristic. Pure, offline, deterministic. Nothing uploaded.

## Competitors scanned (paraphrased; no copy/branding/trademark reproduced)

1. **Gitleaks** (open-source, rule-first). Ships 150+ regex rules keyed on provider prefixes
   (AWS `AKIA…`, GitHub `ghp_…`, Slack `xox…`, Stripe `sk_live_…`, Google `AIza…`, private-key
   PEM headers, JWT-shaped strings). Uses Shannon entropy as a secondary signal. Per-rule
   allowlists/stopwords cut false positives. Offers a `--redact` flag to mask matched values in
   output, and emits JSON/SARIF for tooling. Primary surface is a git-history/filesystem scanner.
2. **TruffleHog** (open-source, verification-first). Finds candidates by regex + entropy, then
   makes a live API call to the provider to confirm the credential is currently valid; classifies
   800+ secret types and maps each back to an identity. Scans git history, filesystems, buckets.
3. **detect-secrets** (Yelp, open-source). Plugin architecture: named regex detectors plus two
   entropy plugins — Base64HighEntropyString (default Shannon limit ~4.5) and HexHighEntropyString
   (default ~3.0) — and a keyword detector that triggers on context words like `password`,
   `secret`, `api_key`. Produces a baseline file so teams can audit/ignore known findings.

(GitGuardian ggshield and AWS git-secrets skimmed as secondary references — same shape: regex
provider rules + entropy + pre-commit/CI integration.)

## Table-stakes → in-model / out-of-model

| Capability | Decision | Where |
| --- | --- | --- |
| Named provider prefix patterns (AWS, GitHub, GitLab, Slack, Stripe, Google, OpenAI, Twilio, SendGrid, npm, Shopify, Square, Slack webhook) | in-model | `type` detectors, high severity |
| Private-key PEM header detection (`-----BEGIN … PRIVATE KEY-----`) | in-model | detector, high |
| JWT-shaped string detection | in-model | detector, medium |
| Generic keyword + high-entropy value (Shannon entropy) | in-model | `min_severity=all`, medium |
| Placeholder / stopword filtering to cut false positives | in-model | built-in allowlist (`example`, `your_…`, `changeme`, `xxxx`, …) |
| Redact matched secret in output (`--redact`) | in-model | `redact` boolean, default on |
| Severity / confidence levels | in-model | `min_severity` (all \| high) |
| Line + column of each finding | in-model | every finding |
| JSON output for tooling | in-model | `format` (text \| json) |
| Preset examples / one-click | in-model | `[[example]]` chips |
| **Live credential verification (API call to provider)** | out-of-model | needs outbound network per provider + non-deterministic; unsafe to auto-validate leaked creds. Listed, not built. |
| **Git-history / whole-repo / filesystem scan** | out-of-model | single paste-in input; no git or fs access in a browser wasm tool. |
| **Baseline file / audit workflow** | out-of-model | stateless single-shot tool; no persisted baseline. |
| **User-defined custom regex rules** | out-of-model (future) | would need a rules-config param + safe user-regex compilation; deferred. |
| **SARIF output** | out-of-model (future) | JSON covers the tooling-integration table-stake; SARIF is a niche format. |

## UX controls (competitors are mostly CLIs; presets → chips)

- Multiline textarea for the snippet (paste-friendly, newlines preserved).
- `min_severity` `<select>` with friendly labels.
- `redact` checkbox (default checked — safer for a security tool; unchecking reveals full values).
- `format` `<select>` (readable report vs JSON).
- `[[example]]` preset chips: AWS key, GitHub token, private-key header, generic API-key
  assignment, and a clean-config (no findings) case.

## Design decisions

- Default `redact = true`: the tool's whole point is surfacing secrets safely — masking the value
  (keeping only a short non-secret prefix) is the safe default; users can turn it off to see the
  full match.
- Generic keyword+entropy findings are **medium** and hidden by `min_severity=high`, because they
  carry the most false positives; named-provider and private-key findings are **high**.
- No live verification, ever: a clean result means "no known pattern matched", not "safe" — stated
  on the page and in the report footer. This is a heuristic aid, not a guarantee.
- Deterministic + offline: runs as WebAssembly in the browser tab; nothing is uploaded.
