## About this tool

**Email Reply Cleaner** pulls the fresh message out of a replied or forwarded plain-text email
so you're left with only what the person actually wrote this time — no `>` quote markers, no
"On … wrote:" reply chain, and no signature or mobile footer. It's the quick way to grab a clean
quote for notes, a ticket, a changelog, or an LLM prompt without hand-deleting the cruft.

Everything runs locally in your browser — the email you paste is never uploaded.

### Worked example

**Email text**

```
Hi Bob,

Friday works — see you at 10.

Best,
Alice

-- 
Alice Smith
Acme Corp

On Mon, Jan 1, 2024 at 10:00 AM Bob <bob@example.com> wrote:
> Are you free on Friday?
> Let me know.
```

**Cleaned message**

```
Hi Bob,

Friday works — see you at 10.

Best,
Alice
```

The `-- ` delimiter and everything below it (the signature and the whole quoted reply chain) are
cut, and the surrounding blank lines are tidied.

### What each pass does

- **Remove quoted lines (>)** — drop every line that begins with the `>` quote marker, including
  nested `>>` and lines indented before the `>`.
- **Remove reply chain** — cut at the first reply boundary and everything below it: an
  `On … wrote:` attribution, an Outlook `-----Original Message-----` / `From:` header block, or a
  `---------- Forwarded message ----------` / `Begin forwarded message:` rule.
- **Remove signature** — cut at the RFC 3676 `-- ` delimiter or a common mobile/app footer
  (`Sent from my iPhone`, `Get Outlook for …`) and everything below it.
- **Collapse blank lines** — fold runs of blank lines into one and trim blank lines off the top
  and bottom.

Each pass is a checkbox and all four are on by default — turn one off to keep, say, the quoted
text but still strip the signature.

### Limits

- Plain text only — paste the text body, not raw HTML email markup.
- Attribution detection is **English-focused**: it keys on the `On … wrote:` line. The
  structural markers (`>` prefixes, the `-- ` delimiter, `From:`/`Sent:` headers, forwarded-
  message rules) are language-neutral, but a translated attribution line may not be caught.
- Signature detection is heuristic (the `-- ` delimiter plus a short list of known mobile/app
  footers), not a trained classifier, so an unusual signature with no delimiter may remain.
- A single-line attribution is detected; a header wrapped across two lines (as Gmail sometimes
  wraps `On … wrote:` at 80 columns) is not.

## FAQ

<details>
<summary>What exactly gets removed from my email?</summary>

By default, four things: lines starting with the `>` quote marker, the quoted reply chain (from
the `On … wrote:` attribution, an Outlook `-----Original Message-----` / `From:` header block, or
a forwarded-message rule downward), the signature (from the `-- ` delimiter or a mobile footer
downward), and extra blank lines. Each is a separate checkbox, so you can keep any of them.

</details>

<details>
<summary>Does it handle forwarded messages and Outlook-style headers?</summary>

Yes. **Remove reply chain** cuts at a `---------- Forwarded message ----------` /
`Begin forwarded message:` rule and at an Outlook `-----Original Message-----` marker or a
`From: Name <addr@host>` header line, removing that block and everything below it.

</details>

<details>
<summary>Why is my signature or attribution still showing?</summary>

Detection is heuristic. Signatures are found by the RFC 3676 `-- ` delimiter or a known mobile
footer, so a signature with no delimiter can slip through. Attribution detection is
**English-focused** (`On … wrote:`) and only catches single-line attributions — a translated or
line-wrapped header may not match. You can delete the leftover manually, or turn a pass off if it
is removing too much.

</details>

<details>
<summary>Is my email uploaded anywhere?</summary>

No. The cleaning runs entirely in your browser via WebAssembly — the text you paste never leaves
your device.

</details>
