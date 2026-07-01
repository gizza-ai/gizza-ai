## SafeLink decoder

Corporate mail systems rewrite links so they point at a scanner first. Paste one
of those rewritten links and get the **real destination** back. It all happens
in your browser — the link is decoded from the text, never fetched.

### Wrappers it unwraps

- **Outlook SafeLinks** — `*.safelinks.protection.outlook.com/?url=…`
- **Proofpoint URLDefense** — both `v2` (`?u=…` with `-`/`_` encoding) and `v3`
  (`/v3/__…__;…`)
- **Google redirects** — `google.com/url?q=…`
- **Generic redirectors** — any link with a `url=`, `q=`, `u=`, `target=`,
  `redirect=`, … parameter pointing at an `http(s)` URL

Nested wrappers (a SafeLink wrapping a Google redirect, …) are followed
automatically.

### Why it's safe

It only decodes the text you paste — it never opens or fetches the link, so you
can inspect a suspicious destination without visiting it.

### FAQ

<details>
<summary>Does it visit the link?</summary>

No. It's pure local string decoding; nothing is
fetched or uploaded.

</details>

<details>
<summary>My link wasn't a known wrapper.</summary>

It's returned unchanged — only recognized
wrappers are unwrapped.

</details>

<details>
<summary>How many layers of nesting does it follow?</summary>

Up to 8. Each pass unwraps one layer (say a SafeLink whose target is a Google
redirect whose target is the real page) and stops as soon as the result is no
longer a recognized wrapper — so even multiply-forwarded corporate mail links
resolve in one click.

</details>

<details>
<summary>How are the two Proofpoint URLDefense versions handled?</summary>

**v2** links carry the destination in a `?u=` parameter with a substitution
cipher — `-` stands for `%` and `_` for `/` — which is reversed before
percent-decoding. **v3** links embed the URL literally between `/v3/__` and
`__;` (followed by an encoded checksum), so it's extracted from there. Both are
detected automatically from the link shape.

</details>

<details>
<summary>Can I decode a whole list of links at once?</summary>

Yes — paste one link per line and enable **Unwrap each line separately
(batch)**. Every non-empty line is decoded independently and blank lines are
preserved, so the output lines up 1:1 with your input for pasting back into a
spreadsheet or report.

</details>
