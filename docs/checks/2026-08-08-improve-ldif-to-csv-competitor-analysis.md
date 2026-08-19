# ldif-to-csv competitor analysis (2026-08-08)

Tool: `ldif-to-csv` — parse an LDAP LDIF export into CSV, mapping each entry to a row and
attributes to columns.

## Scan summary

I compared the table-stakes behavior users expect from the common LDIF→table workflows
(directory-server export utilities, admin scripting recipes, spreadsheet import guides, and
generic browser converters). Every capability below is paraphrased from the *expectations* those
workflows set — no competitor copy, naming, or branding is reproduced.

| Reference workflow | What users expect | In gizza model? | Decision for this tool |
| --- | --- | --- | --- |
| Directory-server export/import utilities (`ldapsearch -LLL` output, server-side export files) | Read strictly RFC 2849 shaped text: optional `version: 1` header, `#` comment lines, entries separated by blank lines, 76-column line folding with a single leading space, `attr: value`, `attr:: base64`, `attr:< url`. | Yes | Full RFC 2849 reader: version line, comments, blank-line record separation, continuation unfolding, all three value forms, and a DN that may itself be base64 (`dn::`). |
| Admin scripting recipes (shell/PowerShell/Python one-liners that flatten LDIF for a spreadsheet) | Choose which attributes become columns and in which order; keep `dn` as the first column; skip attributes you do not care about. | Yes | `columns` param takes an ordered comma-separated attribute list (blank = every attribute in first-seen order); `include_dn` puts `dn` first and can be turned off. A requested attribute that never appears still gets an (empty) column so the header stays stable. |
| Same recipes, multi-valued attributes | LDAP entries repeat attributes (`objectClass`, `memberOf`, `mail`). Users variously want them joined into one cell, spread across numbered columns, or reduced to a single value. | Yes | `multi_value` enum: `join` (default, `value_separator` between values), `indexed` (`memberOf`, `memberOf.2`, `memberOf.3`, … capped by `max_indexed`), `first`, `last`. |
| Spreadsheet import guides | The file must open cleanly in a spreadsheet: RFC 4180 quoting, a stable header row, an alternative delimiter when values contain commas, and no stray blank columns. | Yes | RFC 4180 writer (quotes fields containing the delimiter, `"`, CR or LF; doubles inner quotes), stable first-seen column order, and a `delimiter` param accepting a single char or `comma`/`tab`/`semicolon`/`pipe`. |
| Change-file (changetype) handling in migration guides | An LDIF may be a *change* record set (`changetype: add/modify/delete`) rather than a content export; the `add:`/`replace:`/`delete:` modification directives and their `-` separators are protocol, not data, and must not become columns. | Yes | When a record carries `changetype`, the change-protocol keywords (`control`, `changetype`, `add`, `replace`, `delete`, `newrdn`, `deleteoldrdn`, `newsuperior`) and `-` separator lines are excluded from the data columns; `include_changetype` optionally surfaces the operation itself as a column. |
| Binary attribute guidance (`jpegPhoto`, `userCertificate`, `objectGUID`) | Base64 values that are really text (accents, CJK, trailing spaces, leading `#`) should be readable in the CSV; genuinely binary values must not corrupt the file. | Yes, bounded | `decode_base64` (default on) decodes to text only when the bytes are valid UTF-8; binary payloads stay in their base64 form so the CSV remains valid text. Turning the option off keeps every `::` value as base64. |
| URL-referenced values (`attr:< file:///…`, `attr:< http://…`) | The value lives in another file; converters either fetch it or record the reference. | Yes, bounded | The reference URL is written into the cell as text. Fetching is deliberately not done — this block is pure, offline, and browser-local, so a fetch would be both impossible in the page sandbox and a privacy regression. Documented on the page and in the FAQ. |
| Attribute-name case handling | LDAP attribute names are case-insensitive, and one export can mix `givenName` / `givenname`, which naive splitters turn into two columns. | Yes | Names are folded case-insensitively for grouping; the header keeps the first-seen spelling. |
| Browser converters generally | Paste-in textarea, preset example chips, selects/checkboxes rather than free-text flags, copy + download of the result, everything client-side. | Yes | Multiline `ldif` textarea, enum `<select>` for `multi_value`, checkboxes for the booleans, three `[[example]]` preset chips, and the generator's built-in copy/download/reset affordances on a generic page. |
| Full directory clients / LDAP browsers | Connect to a live directory over LDAP(S), page through results, follow referrals, resolve schema/objectClass definitions, write changes back. | No | Out of model: requires network + an LDAP protocol client. This tool converts LDIF *text* you already have. |
| Schema-aware value formatting | Render `objectGUID`/`objectSid` in their canonical string form, or convert Active Directory timestamps (`whenCreated`, `pwdLastSet`) to dates. | No | Out of model for this tool: it needs per-attribute schema knowledge rather than LDIF parsing. Values are emitted exactly as the file states them; date/GUID formatting belongs in dedicated conversion tools. |
| Round-trip CSV → LDIF | Some workflows want the reverse direction for bulk import. | No | A separate tool: the reverse direction has a different input shape and its own DN-construction rules. Listed here so it is not silently dropped. |

## Table-stakes mapped to implementation

| Capability | Built | Notes |
| --- | --- | --- |
| Blank-line record separation | Yes | Consecutive blank lines collapse; trailing entry without a final blank line still parses. |
| `version:` header + `#` comments | Yes | Both ignored; a comment may itself be folded across lines. |
| Line unfolding (continuation) | Yes | A line starting with exactly one space appends to the previous logical line, space removed — applied before any `:` parsing so folded base64 and folded DNs work. |
| `dn` column | Yes | First column when `include_dn` is on; `dn::` base64 DNs are decoded like any other value. |
| Plain `attr: value` | Yes | One optional space after the colon is stripped (RFC 2849); further spaces are preserved. |
| Base64 `attr:: value` | Yes | Standard-alphabet decode with padding; invalid base64 is a clear named error. |
| URL `attr:< url` | Yes | Reference text kept as the cell value; never fetched. |
| Repeated attributes | Yes | `join` / `indexed` / `first` / `last`, `value_separator`, `max_indexed` 1–50. |
| Missing / empty values | Yes | Missing attribute → empty cell; `attr:` with no value → empty string, which still creates the column. |
| Stable column ordering | Yes | First-seen order across the whole file, or exactly the `columns` order when given. |
| CSV escaping / quoting | Yes | RFC 4180 via the `csv` crate; verified for embedded delimiter, quote, and newline. |
| Alternative delimiters | Yes | Single char or `comma`/`tab`/`semicolon`/`pipe`; a multi-char delimiter is a clear error. |
| Change records | Yes | Protocol keywords excluded; `include_changetype` opt-in column. |
| Case-insensitive attribute names | Yes | Folded for grouping, first-seen spelling in the header. |
| Size cap | Yes | 50,000 entries, with an explicit error above it rather than a browser hang. |
| Live LDAP connection | Out of model | Needs network + LDAP protocol client. |
| GUID/SID/AD-timestamp formatting | Out of model | Needs directory schema knowledge, not LDIF parsing. |
| CSV → LDIF (reverse) | Out of model | Distinct tool with different input shape and DN-construction rules. |
| Fetching `attr:<` references | Out of model | Pure offline block; fetching would break the no-upload guarantee. |

## Verification focus

The verification matrix covers:

- exact CSV for a two-entry export with a multi-valued `objectClass`;
- folded continuation lines reassembled byte-exactly;
- `version:`/comment lines ignored;
- base64 text decode, base64 binary passthrough, and `decode_base64=false`;
- `attr:<` URL reference kept as text;
- `multi_value` = `join` / `indexed` / `first` / `last`, plus a non-default `value_separator`;
- `max_indexed` at the cap (50) and one over (51 → error);
- `columns` selection/ordering, including an attribute absent from the file;
- `include_dn=false` and `include_changetype=true` on a change LDIF;
- quoting for values containing the delimiter, a quote, and a newline;
- empty input, DN-less input, and a bad delimiter as errors;
- browser deep link with URL-encoded LDIF plus non-default options;
- CLI exact-output run generated from the page's own example.
