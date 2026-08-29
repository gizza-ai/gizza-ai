## About this tool

Normalize messy country columns into ISO 3166-1 values without sending the data anywhere. Paste names, alpha-2 codes, alpha-3 codes, numeric codes, former names, common names or a mixed list; the tool returns the canonical country name, alpha-2, alpha-3, numeric code and flag emoji.

Use it when a CRM export, survey file or vendor spreadsheet contains values such as `USA`, `U.S.`, `Deutschland`, `Korea, Republic of`, `826` and `Swizerland` in the same column. The matcher ignores case, punctuation and accents, understands a curated set of common aliases, and can correct small typos while marking ambiguous or unknown rows instead of guessing.

### Worked example

Input:

```text
usa
Deutschland
Korea, Republic of
826
Swizerland
Atlantis
```

Default output:

```text
Input               Name                  Alpha-2  Alpha-3  Numeric  Match      Flag
usa                 United States of America  US       USA      840      exact      🇺🇸
Deutschland         Germany                                               DE       DEU      276      alias      🇩🇪
Korea, Republic of  Korea (Republic of)                                   KR       KOR      410      exact      🇰🇷
826                 United Kingdom of Great Britain and Northern Ireland  GB       GBR      826      exact      🇬🇧
Swizerland          Switzerland                                           CH       CHE      756      fuzzy      🇨🇭
Atlantis            —                                                     —        —        —        unmatched  —
```

Choose `Output = Alpha-2 only` to produce an import-ready code column, `CSV` or `JSON` for every field, or `Rows that don't resolve = Show only these` to audit values before loading them into another system.

### Limits and edge cases

- The tool covers the 249 officially assigned ISO 3166-1 entries compiled into the block.
- Each run accepts up to 1000 non-empty items.
- `auto` splitting keeps commas inside names when the input is already one item per line; for a single line it splits on commas, semicolons, pipes and tabs.
- Unofficial, historical or supra-national entities such as `Kosovo`, `Soviet Union` and `EU` are not forced into ISO country codes.
- Fuzzy matching is conservative: equally close candidates are reported as ambiguous rather than guessed.

## FAQ

<details>
<summary>Can it convert both names to codes and codes back to names?</summary>

Yes. Paste an ISO alpha-2 code like `US`, alpha-3 like `USA`, numeric like `840`, or a country name/alias. Pick `Output = Name only`, `Alpha-2 only`, `Alpha-3 only`, `Numeric only`, `Flag emoji only`, `CSV`, `JSON`, or keep the default table.

</details>

<details>
<summary>What happens to rows the tool cannot match?</summary>

By default they stay in the output and are marked `unmatched`, so a converted column remains aligned with your source rows. Change `Rows that don't resolve` to `Leave blank`, `Drop the row`, or `Show only these` when you want an audit list.

</details>

<details>
<summary>Does fuzzy matching ever guess between two similar countries?</summary>

No. A typo with one clear nearest country, such as `Swizerland`, can resolve as a `fuzzy` match. If several countries are equally close, the row is marked ambiguous and no code is emitted.

</details>

<details>
<summary>Does this include phone dial codes, currencies or capitals?</summary>

No. This tool is intentionally limited to ISO 3166-1 country identifiers: English country name, alpha-2, alpha-3, numeric code and flag emoji. Extra geography reference fields use different datasets and update rules.

</details>
