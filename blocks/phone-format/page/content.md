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
