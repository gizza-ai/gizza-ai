# address-parse — competitor analysis (2026-07-31)

Scope: a browser-local, wasm, no-account tool that splits a freeform postal address
string into structured fields (house number, street, unit, city, region/state,
postcode, country). Rule-based / heuristic (pure Rust), no ML model, no server.

## Competitors scanned

1. **libpostal (openvenues) / libpostal-rs** — the reference open-source address parser.
   Statistical CRF model trained on OpenStreetMap. Returns `house_number`, `road`,
   `unit`, `city`, `state`, `postcode`, `country`, plus `suburb`, `city_district`,
   `state_district`, `po_box`, `level`, `staircase`, `entrance`. Very high international
   accuracy on messy input; needs a ~2 GB trained model. (Out-of-model for gizza: ships an
   ML model + large data files; can't run browser-local.)
2. **Smarty (smarty.com) US Street Address / usaddress** — probabilistic parser (usaddress
   library). Components: Primary/house number, pre-direction, street name, street suffix,
   post-direction, secondary designator (Apt/Ste), secondary number, city, state, ZIP, plus
   ZIP+4. Also does USPS validation/standardization + geocoding (paid API — out-of-model).
3. **Parserator (Google Workspace add-on)** — wraps usaddress; splits into street, city,
   state, zip columns in a spreadsheet. US-focused; free tier 1000 parses. UX = spreadsheet
   columns, one row per address.
4. **AddressGenerator.org "Address Formatter"** — takes a raw address, parses to street /
   city / state-province / ZIP-postal / country, and re-emits a standardized single string.
   International, one address at a time, in-browser form.
5. **International Address Parser (address-parser.net)** — free-form → house number, street
   type, street name, unit, zipcode, state, country, city. International, form-based.

(Geoapify / Google Address Validation were also seen but are geocoding/validation APIs —
server + API key, out-of-model.)

## Table-stakes (features every serious parser ships) → decision

| Capability | In our model? | Where it lands |
|---|---|---|
| house number | yes | `house_number` field |
| street / road name | yes | `street` field |
| unit / secondary designator (Apt/Suite/Unit/#) | yes | `unit` field |
| city / locality | yes | `city` field |
| region / state / province | yes | `region` field (+ `region_code` for US/CA/AU) |
| postcode / ZIP (incl. ZIP+4, UK, CA) | yes | `postcode` field, country-aware regex |
| country | yes | `country` + `country_code` (ISO-3166-1 alpha-2) |
| country hint / default when absent | yes | `country` enum param (`auto` + common ISO codes) |
| comma-separated AND multi-line input | yes | both normalized before parsing |
| worked examples / presets | yes | 3 `[[example]]` preset chips |
| statistical/ML accuracy on ambiguous input | NO | out-of-model (needs libpostal-class model) |
| USPS/postal standardization & deliverability validation | NO | out-of-model (needs official datasets/API) |
| geocoding (lat/lng) | NO | out-of-model (needs server/API) |
| pre-/post-directional split (N/S/E/W as own field) | rejected | folded into `street`; a separate field adds schema noise for marginal value in a heuristic parser — stated as a limit |
| street-suffix classification (St→Street) | rejected | kept verbatim in `street`; normalization risks corrupting non-US street forms |
| full per-country region vocabularies (all provinces worldwide) | partial | region codes for US/CA/AU; elsewhere region is best-effort — stated as a limit |

## UX patterns adopted (ideas only, original copy)

- Multi-line textarea (addresses are pasted with line breaks).
- Country **`<select>`** hint (`auto` + common ISO codes) — biases postcode/region detection
  and fills `country` when the text omits it, mirroring how usaddress/libpostal take a
  country/locale hint.
- One-click preset chips (US, UK, multi-line) doubling as worked examples.
- Human-readable field table on the page; JSON on the chat/CLI surface.

## Out-of-model (considered, not built)

ML/CRF statistical parsing (libpostal-grade accuracy on unusual orderings), USPS/postal
standardization + deliverability, geocoding, and exhaustive worldwide region vocabularies —
all need a trained model, official datasets, or a server, none of which fit gizza's
browser-local pure-wasm model. The tool is an honest rule-based parser tuned for the common
comma/line-separated formats, with those limits stated on the page.
