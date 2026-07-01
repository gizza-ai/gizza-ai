## About this tool

Spam bots crawl the web and scrape email addresses out of page source — every
plain `you@example.com` or `mailto:` link is fair game for an address harvester.
The **Email Obfuscator** rewrites your address into HTML that a regex scraper
can't read, while a real browser still renders it (and, optionally, keeps it
clickable). Everything runs locally in your browser; your address is never sent
to a server.

## Obfuscation modes

- **HTML entities** (default) — every character becomes a numeric character
  reference, e.g. `you@example.com` → `&#121;&#111;&#117;…`. The browser shows
  the original text; a scraper sees only entities. Choose **decimal** (`&#106;`)
  or **hexadecimal** (`&#x6a;`) radix.
- **JavaScript** — emits a `<script>` that builds the address from character
  codes at run time with `document.write`, so the address never appears as a
  literal in the served HTML at all. A `<noscript>` entity-encoded fallback
  keeps it reachable without JavaScript.
- **CSS reversal** — prints the address backwards in the source and flips it
  back visually with `unicode-bidi: bidi-override; direction: rtl`. A scraper
  reads `moc.elpmaxe@uoy`; visitors see `you@example.com`.
- **ROT13 mailto** — a `mailto:` link whose `href` is ROT13-scrambled and
  decoded by a tiny inline `onclick` handler the moment a visitor clicks it
  (the classic WordPress-style trick).

## How to use

1. Type the email address you want to protect.
2. Pick an obfuscation mode (and, for entity output, decimal or hex).
3. Optionally turn off the clickable `mailto:` link, or set custom link text
   such as "Email us".
4. Copy the generated HTML and paste it into your page.

## Notes

- Obfuscation slows down *automated* scrapers; it is not encryption. For
  high-value inboxes, pair it with a contact form or a server-side relay.
- The address is validated as `local@domain` with a dotted domain before
  encoding, so typos are caught early.
- All four modes leave the address usable by humans — entities and CSS render
  normally, and the JS/ROT13 modes degrade to an entity-encoded fallback.

## FAQ

<details>
<summary>Which obfuscation mode should I pick?</summary>

**Entities** (the default) is the safest all-rounder — it needs no JavaScript and
keeps the mailto link clickable. **JavaScript** is the strongest against scrapers
because the address never appears as a literal in the served HTML, but it depends
on JS (a `<noscript>` entity fallback is included). **CSS reversal** is great for
display-only addresses, and **ROT13 mailto** is the classic trick when you mainly
care about protecting the `href`.

</details>

<details>
<summary>Why doesn't the CSS mode give me a clickable link?</summary>

Because the trick works by printing the address backwards and flipping it visually
with `unicode-bidi: bidi-override` — a reversed `mailto:` href wouldn't work when
clicked. CSS mode therefore always emits display text only; use entities, JS, or
ROT13 mode if you need a working link.

</details>

<details>
<summary>Does this actually stop spam?</summary>

It defeats the regex-based harvesters that account for most address scraping, but
it is obfuscation, not encryption — a bot that executes JavaScript or decodes
entities can still recover the address. For a high-value inbox, combine it with a
contact form or a server-side relay.

</details>

<details>
<summary>Why does it say my email address is invalid?</summary>

The address is checked as `local@domain` with a dotted domain (e.g.
`you@example.com`) before encoding, so a missing `@` or a bare domain like
`you@localhost` is rejected — better to catch the typo here than to publish
obfuscated HTML of a broken address.

</details>
