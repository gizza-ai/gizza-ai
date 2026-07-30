## About this tool

Use this molecular weight calculator to turn a chemical formula into molar mass,
monoisotopic mass, Hill notation, atom counts, and a per-element composition
table. It is designed for formula checks in the browser: no upload, account, or
network lookup is needed after the page loads.

Worked example: `C8H10N4O2` (caffeine) reports a molar mass of about
`194.194 g/mol`, a monoisotopic mass of about `194.0804 Da`, 24 total atoms, and
composition rows for carbon, hydrogen, nitrogen, and oxygen.

The parser supports normal element symbols and counts (`H2O`, `NaCl`,
`C6H12O6`), nested groups with multipliers (`Ca(OH)2`, `K4[Fe(CN)6]`), and
hydrate dot notation with a leading coefficient (`CuSO4·5H2O`, `CuSO4.5H2O`,
or `Na2CO3*10H2O`). Element symbols are case-sensitive, so `Co` means cobalt
while `CO` means carbon plus oxygen.

Limits and edge cases: formulas must use element symbols H through U from the
embedded atomic-mass table. Isotopic labels, charges, structural formulas,
fractional stoichiometry, and SMILES strings are not parsed. The exact mass is
the sum of each element's most abundant isotope; it is useful for quick checks,
not a substitute for a full isotope-pattern engine.

## FAQ

<details>
<summary>Does this accept SMILES strings?</summary>

No. SMILES parsing needs a chemistry graph engine and is outside this pure
formula parser. Enter the molecular formula instead, such as `C6H6` for benzene
or `C8H10N4O2` for caffeine.

</details>

<details>
<summary>What is the difference between molar mass and monoisotopic mass?</summary>

Molar mass uses standard atomic weights and is reported in `g/mol`. The
monoisotopic mass uses the exact mass of the most abundant isotope for each
element and is reported in daltons (`Da`).

</details>

<details>
<summary>Can I calculate hydrates and grouped formulas?</summary>

Yes. Use parentheses or brackets for groups, such as `Ca(OH)2`, and dot notation
for hydrates, such as `CuSO4·5H2O`. A plain period (`.`) or asterisk (`*`) also
works when the middle dot is inconvenient to type.

</details>

<details>
<summary>Why does capitalization matter?</summary>

Chemical element symbols are case-sensitive. For example, `Co` is cobalt, but
`CO` is carbon monoxide. The calculator follows that convention and reports an
unknown-element error when a symbol is not in the mass table.

</details>
