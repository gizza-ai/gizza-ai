# egfr-calc — competitor analysis (2026-07-29)

Scan of the top real eGFR / CKD-EPI calculators before building. All findings are
**paraphrased** — no competitor copy, branding, or trademarks reproduced. This is an
informational calculator, **not** a diagnostic or medical-advice tool; caveats below are
baked into the page.

## Competitors surveyed (paraphrased)

1. **National Kidney Foundation — CKD-EPI Creatinine (2021)** — the authoritative source for
   the 2021 race-free equation and its coefficients. States the equation is the recommended
   US standard; notes creatinine must be IDMS-standardized; output in mL/min/1.73 m².
2. **MDCalc — CKD-EPI GFR** — offers an equation family selector (2021 creatinine, 2021
   creatinine+cystatin C, 2009 creatinine, cystatin-C variants). Female/Male selector, age,
   creatinine. Shows a KDIGO GFR-category table alongside the result.
3. **Medscape / QxMD — eGFR CKD-EPI (2021)** — creatinine + age + sex; presents the numeric
   eGFR with unit.
4. **Merck Manual — eGFR by CKD-EPI without race (2021)** — creatinine (with a mg/dL ⇄ µmol/L
   unit choice), age, sex; race-free framing emphasised.
5. **Miscellaneous online eGFR tools (egfrcalculator.org, gfr-calculator.com,
   creatinineclearance.com)** — creatinine unit toggle (mg/dL / µmol/L), age, sex, 2021
   equation, plain-language stage read-out.

## Table-stakes parameters / defaults / UX

| Capability | Typical competitor behaviour | In gizza model? |
|---|---|---|
| Serum creatinine input | required numeric | ✅ built (`creatinine`) |
| Creatinine unit mg/dL **and** µmol/L | toggle (US vs SI) | ✅ built (`creatinine_unit`, 88.42 conversion) |
| Age (years) | required numeric, adults | ✅ built (`age`, 18–120) |
| Sex (female/male) | selector | ✅ built (`sex`) |
| CKD-EPI 2021 creatinine (race-free) | default / recommended standard | ✅ built (default `equation`) |
| CKD-EPI 2009 creatinine | offered for comparison | ✅ built (race-free form; race coefficient deliberately omitted per 2021 NKF/ASN guidance) |
| Output eGFR in mL/min/1.73 m² | whole-number read-out | ✅ built |
| KDIGO GFR category G1–G5 + plain-language label | stage table / badge | ✅ built (`gfr_stage`, `stage_description`) |
| Worked example on the page | some show one | ✅ built |
| Non-diagnostic disclaimer | present on clinical tools | ✅ built (page + FAQ + summary) |

## Worked examples used to validate the math

- **2021, 50 y male, Scr 0.9 mg/dL:** Scr/κ = 1.0 → eGFR = 142 × 0.9938⁵⁰ ≈ **104** (G1).
- **2021, 60 y female, Scr 1.2 mg/dL:** eGFR ≈ **52** (G3a).
- **2021, 50 y male, Scr 1.0 mg/dL (defaults):** eGFR ≈ **92** (G1/G2 boundary).

## Considered, NOT built (out of scope / out of model)

- **Cystatin-C equations (2012 cystatin C, 2021 creatinine + cystatin C).** A distinct
  equation family that needs a **cystatin C** lab value most users don't have on hand; it is a
  separate tool, not a variant of a creatinine calculator. Left out to keep this tool focused
  and its inputs universally available.
- **2009 race coefficient (×1.159 for Black patients).** Deliberately **omitted**: the 2021
  NKF/ASN task force recommends against using race in eGFR, and the 2021 race-free equation is
  the default here. The 2009 option is provided in its race-free form for historical
  comparison only.
- **Pediatric eGFR (Schwartz / CKiD).** A different population (<18) and equation; CKD-EPI is
  validated for adults, so the tool rejects ages under 18 with a clear message rather than
  returning a wrong number.
- **Saved history / patient tracking / EHR export.** Needs accounts and a backend — outside the
  browser-local, no-account gizza model.
- **MDRD equation.** Superseded by CKD-EPI (less accurate at higher GFR); not built.

## Positioning

Original, brand-free copy. Emphasise: current 2021 race-free standard by default, mg/dL and
µmol/L support, KDIGO stage read-out, everything computed locally in the browser, and a clear
**informational-only, not a diagnosis** framing throughout.
