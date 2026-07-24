# Competitor analysis — aws-sigv4-signer (2026-07-24)

Scan performed BEFORE implementation to fix table-stakes params, defaults, worked
examples, and UX. All observations are paraphrased from public tool pages and the
official AWS SigV4 documentation — no competitor copy, branding, or trademarks reproduced.

## Competitors reviewed

1. **hidekazu-konishi.com — AWS SigV4 Request Signer & Explainer** — client-side
   (CryptoJS), step-by-step "explainer" that shows every intermediate stage.
2. **datafetcher.com — AWS Signature Version 4 Calculator** — client-side (aws4-browser
   npm), debugging-oriented, emits signed headers + a copyable cURL.
3. **AWS official docs** — "Create a signed AWS API request" / "Elements of an AWS API
   request signature" (the normative algorithm + worked examples). Treated as the
   correctness ground truth.

(Several npm/GitHub libraries — psnszsn/aws-sign-v4, r7kamura/aws-signer-v4,
adam-fowler/aws-signer-v4 — confirm the same input/output surface but are code, not
interactive tools.)

## Table-stakes matrix

| Capability | Competitors | Our decision |
|---|---|---|
| HTTP method (fixed set) | both (GET/POST/PUT/DELETE/HEAD/PATCH/OPTIONS) | **in-model** — `method` enumv, default GET |
| Full request URL incl. query string | both | **in-model** — `url`, host/path/query derived + canonicalized |
| Region | both | **in-model** — `region` required |
| Service code | both | **in-model** — `service` required |
| Access key / secret key | both | **in-model** — `access_key` / `secret_key` required |
| Session token → `x-amz-security-token` | both | **in-model** — `session_token`, auto-signed when present |
| Timestamp (YYYYMMDDTHHMMSSZ) + "now" default | both ("Update timestamp" button) | **in-model** — `amz_date`; blank ⇒ current UTC per surface |
| Additional headers (multiline Name: Value) | both | **in-model** — `headers`, canonicalized (lowercased, trimmed, sorted) |
| Request body / payload | both | **in-model** — `payload`, SHA-256 hashed |
| Payload signing mode (SHA-256 vs UNSIGNED-PAYLOAD) | hidekazu | **in-model** — `unsigned_payload` boolean |
| Emit + sign `x-amz-content-sha256` (S3) | hidekazu ("include x-amz-content-sha256") | **in-model** — `sign_content_sha256` boolean |
| Canonical Request output | both | **in-model** — `output=canonical-request` / `all` |
| String to Sign output | both | **in-model** — `output=string-to-sign` / `all` |
| Signature (hex) output | both | **in-model** — `output=signature` / `all` |
| Authorization header output | both | **in-model** — `output=authorization` / `all` |
| Signed headers to send | both | **in-model** — `output=headers` / `all` |
| Copyable cURL command | both | **in-model** — `output=curl` |
| Signing-key HMAC-chain "explainer" view | hidekazu | **considered, not built** — intermediate key hex is a teaching nicety, not needed to produce a request; the derivation is documented in the page FAQ instead. Avoids exposing derived key material as a default output. |
| Preset example buttons (S3 GET, EC2 Describe, JSON API) | hidekazu | **in-model** — `[[example]]` preset chips on the page |
| Per-stage Copy buttons | both | **in-model** — generator gives Copy-result for free |
| SigV4a (ECDSA-P256) / `X-Amz-Region-Set` | AWS docs only | **out-of-model (listed, not built)** — SigV4a uses per-request ECDSA key derivation + a randomized-only signer path; out of scope for a deterministic SigV4-HMAC tool. Documented as a limit. |
| Presigned-URL (query-string auth) variant | AWS docs | **out-of-model (listed, not built)** — this tool ships header-based (`Authorization`) auth; query-string presigning is a separate flow. Noted as a limit on the page. |

## Defaults chosen

- `method=GET`, `output=all`, `unsigned_payload=false`, `sign_content_sha256=false`.
- `amz_date` blank ⇒ each surface supplies current UTC (`SystemTime` chat/CLI,
  `js_sys::Date` on the page) so the tool shows a live result; pass an explicit
  timestamp to reproduce a fixed test vector.

## Worked examples used as correctness ground truth

- **AWS S3 GET Object doc example** — access `AKIAIOSFODNN7EXAMPLE`, secret
  `wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY`, `us-east-1`/`s3`,
  `20130524T000000Z`, object `/test.txt`, `range:bytes=0-9`,
  `sign_content_sha256=true` ⇒ documented signature
  `f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41`.
- **AWS IAM ListUsers doc example** — access `AKIDEXAMPLE`, secret
  `wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY`, `us-east-1`/`iam`,
  `20150830T123600Z`, query `Action=ListUsers&Version=2010-05-08`,
  `content-type` + `x-amz-date` signed ⇒ documented signature
  `5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7`.

Both are asserted verbatim in `core/src/lib.rs` unit tests, giving an external
correctness check independent of our own implementation.

## UX notes carried into the page

- Multiline inputs for `headers` and `payload` (newlines preserved).
- Preset chips for the two documented examples so the page shows a real result
  immediately.
- FAQ covers the top signature-mismatch causes (key/secret vs derived signing key,
  exact payload bytes, header canonicalization, S3 `x-amz-content-sha256`), the
  SigV4a/presigned-URL out-of-scope note, and the local/no-upload privacy point.
