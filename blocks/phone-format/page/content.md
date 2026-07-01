## About this tool

The Phone Number Formatter & Validator parses any international phone number and
tells you whether it's valid, then renders it in every standard format.

Enter a number in international form (with a `+` and country code, e.g.
`+1 415 555 2671`) and it works on its own. For a number written the local way —
without a `+` — add the two-letter **region** code (an ISO-3166 alpha-2 code such
as `US`, `GB`, or `DE`) so it knows which country to interpret it for.

For each number you get:

- **Valid** — whether the number is a real, dialable number for its region.
- **E.164** — the canonical international form (`+14155552671`), ideal for
  storing in a database or passing to an SMS/voice API.
- **National** — how you'd write it locally (`(415) 555-2671`).
- **International** — the spaced international form (`+1 415-555-2671`).
- **Country/region** — the country the number belongs to.
- **Type** — the line type (mobile, fixed line, toll-free, VoIP, …) when the
  metadata can derive it.

Validation and formatting use Google's libphonenumber metadata, bundled into the
tool — everything runs in your browser via WebAssembly, so the number you type is
never sent to a server.

## FAQ

<details>
<summary>Why isn't my number recognized?</summary>

If the number has no `+` country prefix, the tool has no way to know which
country's dialing plan to apply — add the two-letter region code (`US`, `GB`,
`DE`, …) and it will be interpreted as a local number for that country. Numbers
written in full international form (`+44 20 7946 0958`) never need a region.

</details>

<details>
<summary>Does "valid" mean the number is actually in service?</summary>

No. Valid means the number matches a real, dialable pattern for its region
according to libphonenumber's metadata — right length, real prefix, plausible
line type. Whether it's currently assigned to a subscriber can only be known by
a carrier lookup, which this tool deliberately doesn't do (nothing is sent
anywhere).

</details>

<details>
<summary>Which format should I store in my database?</summary>

**E.164** — the compact canonical form like `+14155552671`. It's unambiguous,
sorts consistently, and is exactly what SMS and voice APIs (Twilio and friends)
expect. Use the national/international renderings only for display.

</details>

<details>
<summary>Why is the line type sometimes missing or vague?</summary>

The type (mobile, fixed line, toll-free, VoIP, …) is derived from number-range
metadata, and some countries don't partition ranges cleanly — in the US, for
example, mobile and fixed-line numbers share ranges, so a definite answer isn't
always possible.

</details>
