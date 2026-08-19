## About this tool

This converter turns an LDAP LDIF export into a CSV table. Each `dn:` entry becomes one row, attribute names become columns, and the output is quoted as normal CSV so it opens cleanly in spreadsheet tools or feeds into command-line data pipelines.

It understands the LDIF details that break simple `split(':')` scripts: optional `version: 1` headers, `#` comments, blank-line record separation, folded continuation lines, base64 values (`attr:: ...`), URL references (`attr:< ...`), repeated attributes such as `objectClass` and `memberOf`, and change records with `changetype: modify` protocol lines.

Everything runs locally in your browser. `attr:<` references are kept as reference text; the tool does not fetch files or contact an LDAP server.

### Worked example

Input LDIF:

```ldif
dn: uid=ada,ou=people,dc=example,dc=com
objectClass: top
objectClass: person
cn: Ada Lovelace
mail: ada@example.com

dn: uid=bo,ou=people,dc=example,dc=com
objectClass: top
cn: Bo Diaz
mail: bo@example.com
```

With the default options, repeated `objectClass` values are joined with `|`:

```csv
dn,objectClass,cn,mail
"uid=ada,ou=people,dc=example,dc=com",top|person,Ada Lovelace,ada@example.com
"uid=bo,ou=people,dc=example,dc=com",top,Bo Diaz,bo@example.com
```

Set **Repeated attributes** to *Numbered columns* when you need one column per value (`objectClass`, `objectClass.2`, ...), or fill **Columns** to force a stable subset and order such as `cn,mail,telephoneNumber`.

### Base64 and URL values

LDIF uses `::` when a value needs base64 encoding, often because it starts with a space, includes non-ASCII text, or is binary. With **Decode base64 text values** on, valid UTF-8 becomes readable text; binary bytes stay as the original base64 so the CSV remains valid. Turn the option off to keep every `::` value encoded.

For URL values such as `jpegPhoto:< file:///tmp/photo.jpg`, the reference URL is written to the cell. The converter never fetches the URL.

### Limits and edge cases

- Up to 50,000 entries per paste; split very large exports and convert them in parts.
- `max_indexed` accepts 1–50. In indexed mode, values beyond the cap are joined into the final indexed column rather than silently dropped.
- Attribute names are matched case-insensitively; the header keeps the first spelling seen in the file.
- Missing attributes become empty cells. An explicit empty value (`mail:`) also becomes an empty cell.
- Change-file protocol keywords (`add`, `replace`, `delete`, separator `-`, and related lines) are not data columns. Enable **Include changetype column** to keep the operation name itself.
- This is a text converter, not an LDAP client. It does not connect to a directory, follow referrals, or apply schema-specific formatting for GUID/SID/timestamp attributes.

## FAQ

<details>
<summary>Why did my repeated LDAP attribute become one cell?</summary>

The default is **Join values in one cell**, because it keeps one CSV column per attribute. Change **Repeated attributes** to *Numbered columns* to get headers such as `memberOf`, `memberOf.2`, and `memberOf.3`, or use *Keep first value* / *Keep last value* when your downstream system accepts only one value.

</details>

<details>
<summary>Does it decode `cn:: Q2Fmw6k=` values?</summary>

Yes, when **Decode base64 text values** is enabled. Text payloads decode to readable UTF-8, so that example becomes `Café`. Binary payloads such as photos or certificates stay base64-encoded so the CSV stays valid text.

</details>

<details>
<summary>Can it fetch `jpegPhoto:< file:///...` or HTTP references?</summary>

No. URL-referenced values are kept as their reference text and are never fetched. Fetching would make the tool depend on local files or network access; this converter is pure and browser-local.

</details>

<details>
<summary>Why is the DN column quoted?</summary>

DNs often contain commas, and a comma is also the default CSV delimiter. The CSV writer quotes only when needed and doubles embedded quotes according to standard CSV rules, so spreadsheet importers read the DN as one cell.

</details>

<details>
<summary>Can I convert a change LDIF?</summary>

Yes. The converter ignores modification protocol lines such as `replace: mail`, `add: description`, `delete: attr`, and `-` separators so they do not become data columns. Enable **Include changetype column** if you want to keep whether a row was `add`, `modify`, or `delete`.

</details>
