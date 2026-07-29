## About this eGFR calculator

This tool estimates your **glomerular filtration rate (eGFR)** — a measure of how well your
kidneys filter waste from your blood — from a **serum creatinine** lab value, your **age** and
your **sex**. It uses the **CKD-EPI creatinine** equations, the standard used by clinical
laboratories, and reports the result in **mL/min/1.73 m²** along with the matching **KDIGO GFR
category** (G1–G5). Everything runs locally in your browser; nothing you enter is uploaded.

By default it uses the **CKD-EPI 2021 creatinine equation**, the current race-free standard
recommended by the National Kidney Foundation and the American Society of Nephrology. You can also
select the older **CKD-EPI 2009** equation for comparison — provided here in its **race-free
form**, with the 2009 race coefficient deliberately omitted, in line with the 2021 guidance.

Enter creatinine in **mg/dL** (US) or **µmol/L** (SI); µmol/L is converted to mg/dL by dividing by
88.42 before the equation is applied.

### Worked example

Enter creatinine **1.0** mg/dL, age **50**, sex **male**, equation **CKD-EPI 2021**:

```json
{
  "egfr": 92.0,
  "unit": "mL/min/1.73 m²",
  "equation": "ckd_epi_2021",
  "creatinine_mg_dl": 1.0,
  "age": 50.0,
  "sex": "male",
  "gfr_stage": "G1",
  "stage_description": "Normal or high",
  "summary": "eGFR 92 mL/min/1.73 m² (CKD-EPI 2021) — GFR category G1 (Normal or high)"
}
```

The 2021 equation gives `142 × (1.0/0.9)^−1.200 × 0.9938^50 ≈ 92` mL/min/1.73 m², which falls in
the **G1** band (≥90). A 60-year-old female with a creatinine of **1.2** mg/dL instead returns
about **52** mL/min/1.73 m² — the **G3a** band.

### KDIGO GFR categories

| Category | eGFR (mL/min/1.73 m²) | Meaning |
|---|---|---|
| **G1** | ≥ 90 | Normal or high |
| **G2** | 60–89 | Mildly decreased |
| **G3a** | 45–59 | Mildly to moderately decreased |
| **G3b** | 30–44 | Moderately to severely decreased |
| **G4** | 15–29 | Severely decreased |
| **G5** | < 15 | Kidney failure |

A **G1** or **G2** eGFR is in the normal range on its own — chronic kidney disease is only
diagnosed in those bands when there is *also* a marker of kidney damage (such as albuminuria)
present for at least three months.

### Limits & notes

- **Informational estimate, not a diagnosis or medical advice.** eGFR is an estimate; only a
  clinician can interpret it alongside your full history. Do not change any treatment based on this
  tool.
- **Adults only.** CKD-EPI is validated for people **18 and over**; ages under 18 are rejected —
  a pediatric equation (Schwartz / CKiD) is a different calculation.
- The creatinine value should be **IDMS-standardized** (as reported by modern labs). A single
  creatinine is a snapshot; eGFR can shift with hydration, muscle mass, diet and lab timing.
- **Cystatin-C equations** (2012 cystatin C, 2021 creatinine + cystatin C) need a cystatin C lab
  value and are **not** included here — this is a creatinine-only calculator.
- **Race is not used.** The default 2021 equation is race-free, and the 2009 option omits the race
  coefficient.

## FAQ

<!-- FAQ entries are <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>What is eGFR and what is a normal value?</summary>

**eGFR** (estimated glomerular filtration rate) estimates how many millilitres of blood your
kidneys filter per minute, normalized to a standard body size (1.73 m²). A value of **90 or above**
is considered normal or high (category G1). Values fall gradually with age even in healthy people.
On its own a normal eGFR doesn't rule out early kidney disease, and a single low value doesn't
confirm it — trends over months matter more than one reading.

</details>

<details>
<summary>Why does this calculator not use race?</summary>

In 2021 a National Kidney Foundation / American Society of Nephrology task force recommended
against including race in eGFR equations. This tool uses the **race-free CKD-EPI 2021** equation by
default, and when you pick the older 2009 equation it is applied in its **race-free form** (the
2009 race coefficient is left out). You get the same estimate regardless of race.

</details>

<details>
<summary>Should I enter creatinine in mg/dL or µmol/L?</summary>

Use whichever unit your lab report shows. US reports are usually in **mg/dL**; many other
countries report in **µmol/L** (SI). Pick the matching unit and the tool converts internally
(µmol/L ÷ 88.42 = mg/dL). For example, 1.0 mg/dL ≈ 88.4 µmol/L.

</details>

<details>
<summary>What's the difference between the 2021 and 2009 equations?</summary>

Both estimate eGFR from creatinine, age and sex. The **2021** equation is the current recommended
standard and is race-free by design. The **2009** equation is older and originally included a race
coefficient; here it is offered only for historical comparison and in its race-free form. The two
give slightly different numbers for the same inputs — for most people the 2021 result is what
clinical labs now report.

</details>

<details>
<summary>Can I use this for a child?</summary>

No. The CKD-EPI equations are validated for **adults (18 and older)**, so the tool rejects ages
under 18. Estimating GFR in children uses a different equation (such as the bedside Schwartz or
CKiD formula) based on height and creatinine.

</details>

<details>
<summary>Is my data sent anywhere?</summary>

No. The calculation runs entirely in your browser via WebAssembly. Nothing you enter is uploaded,
logged or stored — reload the page and it's gone.

</details>
