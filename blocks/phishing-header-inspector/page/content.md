## What this tool does

The **Phishing Header Inspector** turns pasted raw email headers into a concise spoofing-risk report. It compares the identity headers attackers often manipulate — `From`, `Return-Path`, `Reply-To`, display name text, `Message-ID`, `Authentication-Results`, and `Received` — and highlights mismatches that deserve a closer look before anyone opens links or attachments.

Everything runs locally in your browser or in the gizza CLI. The tool does **not** contact DNS, SMTP, HTTP, reputation lists, or your mailbox provider, so pasted headers are not uploaded. That also means the report is a deterministic triage signal, not a final verdict from a mail gateway.

## Worked example

Paste a suspicious header block such as:

```text
From: "alerts@paypal.com" <notice@evil.example>
Return-Path: <bounce@mailer.bad>
Reply-To: help@bad.example
Authentication-Results: mx.example; spf=fail; dkim=none; dmarc=fail
Received: from [10.0.0.4] by mx.example
Message-ID: <abc@mailer.bad>
```

The report will call out the visible-display-domain mismatch, the `From` vs `Return-Path` difference, the `Reply-To` detour, failed SPF/DMARC, missing DKIM alignment, and the short/internal-looking `Received` path. For a clean internal sample with aligned domains and `spf=pass; dkim=pass; dmarc=pass`, the same fields produce a minimal-risk report.

## What it checks

| Signal | Why it matters |
| --- | --- |
| **From vs Return-Path** | A mismatch can indicate a forged visible sender or a third-party sender that needs extra scrutiny. |
| **Reply-To detours** | Attackers often route replies to a different domain while leaving the visible sender untouched. |
| **Display-name spoofing** | A display name that contains one email/domain while the real mailbox uses another is a common impersonation trick. |
| **SPF, DKIM, DMARC results** | Failed, missing, or neutral authentication results reduce trust in the sender identity. |
| **Received chain shape** | Missing, very short, or private/internal-looking hops make the route harder to validate from the pasted headers. |
| **Message-ID domain** | A Message-ID domain that does not resemble the sender can be benign but is useful context during triage. |

## Limits and edge cases

The inspector parses the header block only. It ignores the message body after the first blank line, does not decode attachments, does not visit links, and does not perform live DNS alignment checks. Forwarded messages, mailing lists, helpdesk systems, CRMs, and legitimate bulk senders can create domain differences that are not malicious; use the findings as prompts for investigation rather than automatic blocking rules.

## FAQ

<details>
<summary>Does a high score prove the email is phishing?</summary>

No. A high score means the pasted headers contain several structural warning signs, such as failed authentication or sender-domain mismatches. Legitimate forwarding and bulk-mail systems can also trigger some findings, so confirm with gateway logs, known-good sender records, and the message content before taking action.

</details>

<details>
<summary>Why does the tool not query SPF, DKIM, or DMARC DNS records live?</summary>

Gizza tools run locally and avoid network calls for privacy and repeatability. This inspector reads authentication results already stamped into the message by the receiving mail system. For incident response, compare the report with your mail gateway's live DNS and policy evaluation logs.

</details>

<details>
<summary>Can I paste a full email instead of just the headers?</summary>

Yes, as long as the header block comes first. The parser stops at the first blank line, which is where an RFC 5322 message switches from headers to body. Pasting only the body will return an error because it lacks `Header-Name: value` lines.

</details>

<details>
<summary>Why are some findings marked low or informational?</summary>

Low and informational findings are context, not accusations. For example, a single private IP in a `Received` hop may be normal inside an organization, and a Message-ID mismatch can happen with outsourced senders. The severity helps you focus on failed authentication and identity mismatches first.

</details>
