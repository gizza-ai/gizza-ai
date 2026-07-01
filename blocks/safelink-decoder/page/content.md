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
