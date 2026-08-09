## About this tool

Email Spam Score Checker gives you a private, deterministic pre-flight score for suspicious or draft email copy. Paste a raw RFC 5322 message, an HTML body, or plain text and it reports a 0-100 spamminess score where higher is worse.

The report is intentionally transparent: every fired rule is listed with its point value, so you can see whether the risk came from trigger phrases, uppercase shouting, link density, shorteners, suspicious headers, authentication failures already present in `Authentication-Results`, hidden HTML, or other explainable signals. It does not contact DNS, blacklists, seed inboxes, SMTP servers, or any reputation service.

### Worked example

Paste this message and keep the default `format=auto`, `report=detailed`, and `check_headers=true`:

```text
From: "security@yourbank.com" <notice@grabber.top>
Return-Path: <bounce@mailer.example>
Reply-To: help@other.example
Subject: Verify your account
Authentication-Results: mx.example.com; spf=fail; dkim=none; dmarc=fail

We detected unusual activity. Please confirm your account within 24 hours or your account will be closed.
http://192.0.2.9/login
```

The output highlights the score band, message stats, and rules such as authentication failure, From/Return-Path mismatch, Reply-To detour, credential-pressure phrases, suspicious TLDs, insecure links, and IP-address URLs.

### Limits and edge cases

- The input cap is 1 MiB. Remove large quoted threads, inline base64 attachments, or long footers before scoring.
- Header checks only read headers that are already in the pasted message. The tool does not perform live SPF, DKIM, DMARC, DNS, blacklist, or reputation lookups.
- This is a transparent heuristic approximation, not a SpamAssassin score and not a deliverability guarantee.
- `report=json` is intended for scripts and CI checks that want the same deterministic score and rule list.

## FAQ

<details>
<summary>Is this the same as SpamAssassin or an inbox placement test?</summary>

No. It is a local heuristic checker with a published set of rules and weights. SpamAssassin, DNS reputation, RBL checks, and seed-inbox placement require server-side data or network lookups, which this browser-local tool intentionally does not use.

</details>

<details>
<summary>Why can a legitimate newsletter get a non-zero score?</summary>

Marketing email often contains some spam-like signals: many links, promotional phrases, unsubscribe headers, images, and tracking pixels. A non-zero score is not automatically bad; use the rule list to decide which signals are expected and which ones are worth changing.

</details>

<details>
<summary>What does the header checker actually verify?</summary>

It inspects pasted headers such as `Authentication-Results`, `From`, `Return-Path`, `Reply-To`, `Message-ID`, `Date`, `Received`, and `List-Unsubscribe`. It does not query DNS or validate cryptographic signatures itself; it only reads results already stamped by a mail gateway.

</details>

<details>
<summary>When should I use `format=html` or `format=text` instead of auto?</summary>

Use `format=html` when you paste only an HTML body and want HTML-specific rules such as image-heavy content, hidden text, tracking pixels, and anchor/href mismatches. Use `format=text` when the paste is plain copy with no headers and any angle brackets should be treated as text.

</details>
