## About this tool

This ideal weight calculator estimates an adult's **ideal body weight (IBW)** from **height** and
**sex** using the four equations clinicians actually use — **Hamwi (1964)**, **Devine (1974)**,
**Robinson (1983)** and **Miller (1983)** — and shows them **side by side** rather than picking
one for you, because they disagree by several kilograms at the same height. Enter your height in
**cm** (or switch **Units** to imperial and enter **total inches**, e.g. 5'10" = 70), choose your
**sex**, and optionally set a **body frame**, an **age**, and the **healthy BMI bounds**.
Everything runs locally in your browser; nothing you type is uploaded.

You get each formula's estimate in **kg and lb**, the **BMI that estimate represents** at your
height, the **average** of the four, their **min–max spread**, and a separate **healthy-weight
range** derived from a BMI window (18.5–24.9 by default). Because ideal-weight formulas ignore
body composition, the BMI range is usually the more useful number of the two — this tool gives
you both, plus notes explaining what applies to your input.

**Body frame** applies a ±10% adjustment to every formula: **small** (−10%), **medium** (no
adjustment), **large** (+10%). Set it to **auto** and supply a **wrist circumference** (measured
just below the wrist bone) and the frame is derived for you from the standard clinical
wrist-to-height table, so you don't have to read the table yourself.

### Example

Enter height **175**, sex **male**, units **metric**, frame **medium**, BMI bounds **18.5** and
**24.9**:

```json
{
  "height_cm": 175.0,
  "height_in": 68.9,
  "height_ft_in": "5'9\"",
  "sex": "male",
  "frame": "medium",
  "frame_source": "specified",
  "frame_adjustment_pct": 0.0,
  "formulas": [
    { "formula": "hamwi",    "label": "Hamwi (1964)",    "kg": 72.0, "lb": 158.8, "bmi_at_ideal": 23.5 },
    { "formula": "devine",   "label": "Devine (1974)",   "kg": 70.5, "lb": 155.3, "bmi_at_ideal": 23.0 },
    { "formula": "robinson", "label": "Robinson (1983)", "kg": 68.9, "lb": 151.9, "bmi_at_ideal": 22.5 },
    { "formula": "miller",   "label": "Miller (1983)",   "kg": 68.7, "lb": 151.6, "bmi_at_ideal": 22.4 }
  ],
  "average_kg": 70.0,
  "average_lb": 154.3,
  "formula_range": { "min_kg": 68.7, "max_kg": 72.0, "min_lb": 151.6, "max_lb": 158.8 },
  "healthy_bmi_range": {
    "bmi_min": 18.5, "bmi_max": 24.9,
    "min_kg": 56.7, "max_kg": 76.3, "min_lb": 124.9, "max_lb": 168.1
  },
  "notes": [ "…" ],
  "summary": "Ideal weight for a male at 5'9\" (175 cm), medium frame: 68.7–72 kg (151.6–158.8 lb) across four formulas, average 70 kg (154.3 lb); healthy BMI 18.5–24.9 range 56.7–76.3 kg (124.9–168.1 lb)"
}
```

175 cm is 68.9 inches, i.e. 8.9 inches over the 60-inch (5 ft) baseline every formula is anchored
at, so Devine gives `50.0 + 2.3 × 8.9 = 70.5` kg and Hamwi gives `48.0 + 2.7 × 8.9 = 72.0` kg. The
four answers span **68.7–72.0 kg**, average **70.0 kg** — while the healthy-BMI band at that
height is a much wider **56.7–76.3 kg**.

### Limits & notes

- Every field is optional and falls back to a documented default: height **175 cm** (69 in in
  imperial), sex **male**, units **metric**, frame **medium**, BMI bounds **18.5**–**24.9**.
- **Adults only.** Heights outside **122–250 cm** (48–98.4 in) are rejected, and heights below the
  5 ft baseline carry an extrapolation note — the equations were never fitted there and Hamwi's
  male line heads toward zero. For anyone under 18, use a CDC/WHO growth-chart percentile instead;
  entering an **age** under 18 adds that note to the result.
- **Wrist** is only read when frame is **auto**; if you set a frame explicitly, the result says the
  wrist was ignored. Accepted wrist range is 7.6–30.5 cm (3–12 in).
- The **BMI bounds** are adjustable — set **18.5** and **23** for the WHO Asian cutoffs. `bmi_max`
  must be greater than `bmi_min`.
- Results are always reported in **both kg and lb**, whichever unit system you entered.
- These equations were derived for **clinical drug dosing and dietetics**, not as personal targets.
  They ignore body composition, ethnicity and age, and a muscular person will read as "overweight"
  on every one of them. **Estimates for planning, not medical advice.**

## FAQ

<!-- FAQ entries are <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>Which ideal-weight formula should I use?</summary>

There is no single right answer, which is why all four are shown together. **Devine** is the one
most often used in clinical practice (it was written for drug dosing, and much of the medical
literature assumes it). **Robinson** and **Miller** were later attempts to fit real population
data and read lower for tall people. **Hamwi** is the oldest and the most generous, and is the
"rule of thumb" version dietitians learn. Look at the **spread** and the **average** rather than
fixating on one number — if the four disagree by 3 kg, that disagreement is the honest answer.

</details>

<details>
<summary>How do I measure my body frame, and what does it change?</summary>

Wrap a tape measure around your wrist just below the wrist bone, on the hand you write with, and
read the circumference. Set **Body frame** to **auto**, enter that measurement (cm if you're in
metric, inches if imperial), and the tool picks small, medium or large from the standard clinical
table, which for women also depends on height. The frame then applies a **−10%**, **0%** or
**+10%** adjustment to every formula. If you already know your frame, just pick it directly and
leave wrist blank.

</details>

<details>
<summary>Why is the healthy BMI range so much wider than the formula results?</summary>

They answer different questions. The four formulas each return a *single point* — one weight per
height — while the BMI range returns every weight between two BMI cutoffs, which at 175 cm spans
roughly 20 kg. The point estimates look precise but aren't: healthy weight is a band, not a
number. Use the BMI range as the realistic target zone and the formulas as a reference point
inside it.

</details>

<details>
<summary>Can I enter feet and inches?</summary>

Convert to **total inches** first and switch Units to imperial: 5'10" is `5 × 12 + 10 = 70`, 6'0"
is `72`, 5'4" is `64`. The result echoes your height back as `height_ft_in` (e.g. `5'10"`) so you
can check the conversion landed where you meant, plus `height_cm` and `height_in`.

</details>

<details>
<summary>Does this work for children or teenagers?</summary>

No. All four equations are anchored at a 5-foot adult baseline and are meaningless for growing
bodies — heights under 122 cm are rejected outright rather than returning a confident wrong
answer. For anyone under 18, a CDC or WHO growth-chart percentile for age and sex is the correct
tool; entering an **age** below 18 here adds a note saying exactly that.

</details>

<details>
<summary>Why does my ideal weight not change when I enter my age?</summary>

Because none of the four formulas take age as an input — they use height and sex only. The **age**
field is optional and exists purely so the tool can warn you when the equations don't apply
(under 18). Ignoring age is a genuine limitation of this family of formulas, not an omission
here; if you want an age-sensitive number, a TDEE or body-composition estimate is a better fit.

</details>

<details>
<summary>Is my data sent anywhere?</summary>

No. The calculation runs entirely in your browser via WebAssembly. Nothing you enter is uploaded,
logged or stored — reload the page and it's gone.

</details>
