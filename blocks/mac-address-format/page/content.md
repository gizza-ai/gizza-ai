## About this tool

**MAC Address Format Converter** rewrites one or many **MAC addresses** from any
common notation into the single style and case you choose — then gives you back
a clean, uniform list.

- **Every notation, in and out**: colon (`00:1A:2B:3C:4D:5E`), hyphen
  (`00-1A-2B-3C-4D-5E`), Cisco dotted-quad (`001a.2b3c.4d5e`), and bare hex
  (`001A2B3C4D5E`). Paste a mix — each address is recognized regardless of how it
  was written — and 64-bit **EUI-64** addresses are handled too.
- **Pick the output**: choose colon, hyphen, Cisco, or bare, plus **lower** or
  **upper** case, and every address is rewritten that way.
- **One or many**: separate addresses with spaces, commas, or newlines. **Order
  is preserved and duplicates are kept**, so a list maps 1:1 to its reformatted
  output — handy for building a column to paste back into a spreadsheet or config.
- **Validated**: anything that isn't a 12- or 16-hex-digit MAC is flagged instead
  of silently passed through, so a typo doesn't slip into your config.

Everything runs **locally in your browser** via WebAssembly — your addresses are
never uploaded.

### Handy for

- Converting a list of MACs from Cisco dotted-quad to colon form (or vice versa).
- Normalizing vendor exports to the upper- or lower-case style your tooling wants.
- Preparing MAC allow-lists / DHCP reservations in a consistent format.

> Need to **pull MAC addresses out of a log or block of text** and deduplicate
> them instead? Use the **Extract MAC Addresses** tool.
