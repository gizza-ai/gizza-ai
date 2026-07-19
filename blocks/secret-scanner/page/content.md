## About this tool

This scanner reads a snippet of code, a config file, or an `.env` and points at the exact
lines where an **API key, token, or private key is hardcoded** instead of loaded from the
environment or a secrets manager. It is a static, pattern-based check (in the spirit of a
secrets linter): it never runs your code, never contacts a provider, and nothing you paste
leaves the browser.

It looks for three kinds of secret:

- **Known provider keys** matched by their distinctive prefix shape — AWS access key IDs
  (`AKIA…`), GitHub tokens (`ghp_…`, `github_pat_…`), GitLab PATs (`glpat-…`), Slack tokens
  (`xox…`) and webhooks, Stripe keys (`sk_live_…`/`sk_test_…`), Google API keys (`AIza…`),
  OpenAI keys (`sk-…`), and Twilio, SendGrid, npm, Shopify, and Square tokens. Reported as
  **high** confidence.
- **Private-key headers** — any `-----BEGIN … PRIVATE KEY-----` block (RSA, EC, OpenSSH, PGP,
  …). Rule `private-key`, high.
- **Generic secret assignments** — a secret-ish name (`api_key`, `password`, `secret`,
  `token`, `client_secret`, …) assigned a value with **high Shannon entropy** that doesn't
  look like an obvious placeholder. Rule `generic-secret-assignment`, **medium**. JWT-shaped
  strings (`eyJ….eyJ….…`) are also reported at medium.

Matched values are **redacted by default** — only a short, non-secret prefix is shown — so you
can share the report safely. Turn redaction off to see the full matched value.

### Worked example

Input (default settings — redaction on):

```
aws_secret_key = "AKIAIOSFODNN7EXAMPLE"
```

Output:

```
1 finding(s) (1 high, 0 medium) in 1 line(s) scanned

line 1, col 19  HIGH  aws-access-key-id  AWS Access Key ID
  AKIA…[redacted]

Recommendation: remove hardcoded secrets from source, rotate anything real that was exposed, and load credentials from environment variables or a secrets manager. A clean result means nothing matched, not that the code is secret-free.
```

Switch **Output format** to JSON for a structured `{ summary, findings[] }` object (each finding
carries `line`, `column`, `severity`, `rule`, `provider`, and the redacted `match`) that you can
feed into other tooling.

### Limits and edge cases

- Detection is **line-based and heuristic**. A secret whose value is split across lines, built
  at runtime, base64-wrapped, or read from elsewhere can be missed; and a non-secret string
  that happens to match a prefix shape can be a false positive. Treat findings as leads for
  review, and a clean result as "nothing matched" — **not** a guarantee the code is secret-free.
- It **never verifies** whether a key is live. Unlike tools that call the provider to confirm a
  credential is active, this scanner only matches patterns — safer and fully offline, but it
  can't tell a real key from a look-alike.
- The generic keyword+entropy pass only fires on a value with high entropy (roughly base64/hex
  randomness) that isn't an obvious placeholder (`your_api_key`, `changeme`, `xxxx`, a single
  repeated character, …). Short or low-entropy values are skipped to cut false positives.
- **Show → High confidence only** hides the medium generic and JWT findings.
- Input is capped at 200,000 characters — scan a smaller snippet if you hit the limit.
- This scans **source text you paste**. It does not clone a git repository, walk your
  filesystem, or read commit history — those are different jobs handled by CI/pre-commit tools.

## FAQ

<details>
<summary>Which secrets does it detect?</summary>

Known provider keys by prefix shape (AWS `AKIA…`, GitHub `ghp_…`/`github_pat_…`, GitLab
`glpat-…`, Slack `xox…` and webhooks, Stripe `sk_live_…`, Google `AIza…`, OpenAI `sk-…`,
Twilio, SendGrid, npm, Shopify, Square), any PEM `-----BEGIN … PRIVATE KEY-----` header, JWT-
shaped strings, and generic `name = "value"` assignments where the name is secret-ish
(`api_key`, `password`, `secret`, `token`, …) and the value has high entropy. Provider
patterns and private keys are high confidence; JWTs and generic assignments are medium.

</details>

<details>
<summary>Is a clean result a guarantee my code has no secrets?</summary>

No. This is a static pattern match, not a prover. It can miss a secret that is assembled at
runtime, encoded, stored under an unusual name, or split across lines, and it doesn't know
every provider's key format. Use it as a fast first pass, then back it up with a git-history
scanner in CI and by rotating anything you're unsure about.

</details>

<details>
<summary>Does it check whether a detected key still works?</summary>

No — and that's deliberate. Some scanners make a live API call to the provider to confirm a
credential is valid. This tool never sends anything to a provider (or anywhere): it runs
entirely offline in your browser and only matches patterns. If it flags a real key, assume it
is compromised, rotate it, and remove it from source.

</details>

<details>
<summary>Why was one high-entropy string flagged but a similar one wasn't?</summary>

The generic pass only fires when a **secret-ish keyword** (`api_key`, `password`, `secret`,
`token`, …) is assigned the value, the value is at least 8 characters with high Shannon
entropy, and it doesn't look like a placeholder. A random-looking string with no secret-ish
name next to it, or one that reads like `your_api_key_here`/`changeme`/`xxxx`, is skipped on
purpose to keep false positives down. Named provider prefixes are matched regardless of the
surrounding name.

</details>

<details>
<summary>What does "redact" do, and why is it on by default?</summary>

With redaction on (the default), each finding shows only a short non-secret prefix followed by
`…[redacted]`, so you can paste the report into a ticket or chat without leaking the secret.
Turn it off to reveal the full matched value when you need to confirm exactly what was found.

</details>

<details>
<summary>Does anything I paste get uploaded?</summary>

No. The scan runs as WebAssembly inside your browser tab; the text you paste is never sent to
a server. You can disconnect from the network and it still works.

</details>

<details>
<summary>How do I fix a flagged secret?</summary>

Remove the hardcoded value from source and load it from an environment variable or a secrets
manager at runtime (for example `os.environ["API_KEY"]` / `process.env.API_KEY`). If the
secret was ever committed, rotate it — assume anything that reached a repository is
compromised — and add a pre-commit or CI secret scan so it can't happen again.

</details>
