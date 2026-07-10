## About this tool

**JSON Redact** scans a JSON document and masks the secrets inside it — API keys,
tokens, passwords, private keys and emails — so you can paste a config, log line,
API response or webhook payload into a bug report, gist or chat without leaking
credentials. The JSON structure and key order are preserved; only the detected
values change.

It detects secrets two ways, together:

- **By key name** — any value whose key looks sensitive is redacted. Matching is
  case-insensitive and ignores separators, so `password`, `passwd`, `X-API-Key`,
  `clientSecret`, `access_token`, `DB_PASSWORD`, `authorization`, `private_key`,
  `session_id`, `email`, `ssn`, `cvv`, `pin` and similar all match.
- **By value pattern** (when *Also scan values for secret patterns* is on, the
  default) — any string value that *looks* like a secret is redacted even under an
  innocent key: JWTs (`eyJ…`), AWS keys (`AKIA…`), OpenAI keys (`sk-…`), GitHub
  tokens (`ghp_…`/`gho_…`), Stripe keys (`sk_live_…`), Google API keys (`AIza…`),
  Slack tokens (`xox…`), PEM `PRIVATE KEY` blocks, and email addresses.

### Worked example

Input:

```json
{ "user": "ada", "api_key": "AKIAIOSFODNN7EXAMPLE", "note": "ok" }
```

Output (default `redacted` style):

```json
{
  "user": "ada",
  "api_key": "[REDACTED]",
  "note": "ok"
}
```

`api_key` is masked by its key name. `user` and `note` are left alone (their
values don't look like secrets). Run it in a gizza chat or the [CLI](/) and you
also get a count and the JSON path of every redacted value — here
`["$.api_key"]`.

### Replacement styles

- **redacted** (default) — insert the placeholder text (default `[REDACTED]`; set
  your own in the placeholder field).
- **mask** — replace the value with `***`.
- **null** — replace the value with JSON `null`.
- **empty** — replace the value with `""`.
- **preserve-length** — replace a string with `*` repeated to its original length
  (handy when a downstream schema validates lengths).

### Privacy

Everything runs **in your browser** via WebAssembly — the document is never
uploaded to a server. The same logic is available from the gizza CLI and inside a
gizza chat.

### Limits & edge cases

- Detection is heuristic (key-name + known secret value shapes); it is **not a
  guarantee** that every secret is caught. Review the output before sharing
  anything high-stakes.
- Key-name matching is deliberately broad — a field named `token_type` or
  `csrf_token` is redacted because its key contains `token`. Turn a value into a
  non-secret shape or use narrower field names if that's too aggressive.
- Generic high-entropy strings under innocent keys are **not** flagged unless they
  match a known vendor pattern (that keeps false positives low).
- The input must be **valid JSON** — malformed input returns an error rather than
  a partial redaction.

## FAQ

<details>
<summary>Does my JSON get uploaded anywhere?</summary>

No. On this page everything runs locally in your browser through WebAssembly —
the document never leaves your device. The gizza CLI and chat run the same
pure-Rust logic on your machine.

</details>

<details>
<summary>How does it decide what's a secret?</summary>

Two ways at once. First by **key name**: the key is lowercased and stripped of
separators, then matched against markers like `password`, `secret`, `apikey`,
`token`, `privatekey`, `authorization`, `email`, `ssn` (plus any you add in
*Extra key names*). Second by **value pattern** — when value scanning is on, a
string that matches a known secret shape (JWT, `AKIA…`, `sk-…`, `ghp_…`,
`sk_live_…`, `AIza…`, `xox…`, a PEM private-key block, or an email) is redacted
even under a harmless key.

</details>

<details>
<summary>Can I keep the field but hide only its length or replace it with null?</summary>

Yes — that's the **Replacement style**. `mask` writes `***`, `null` writes JSON
`null`, `empty` writes `""`, and `preserve-length` writes `*` repeated to the
value's original character length. Use `redacted` with a custom placeholder when
you want a labelled marker like `[REDACTED]` or `<hidden>`.

</details>

<details>
<summary>Why was a non-secret field redacted?</summary>

Key-name matching is intentionally broad, so a key that merely *contains* a
marker (for example `token_type`, `csrf_token`, or `user_email`) is masked. That
errs on the side of safety. If you need it left alone, rename the field to
something without the marker, or turn off value scanning if the trigger was the
value rather than the key.

</details>

<details>
<summary>Does it change my JSON's structure or key order?</summary>

No. The document is parsed and re-serialized with key order preserved; only the
detected values are replaced. Numbers, booleans, arrays and nesting stay as they
were, so the redacted output is still valid JSON with the same shape.

</details>
