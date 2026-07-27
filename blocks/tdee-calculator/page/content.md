## About this tool

This TDEE calculator estimates two numbers that anchor almost every diet and training plan:
your **Basal Metabolic Rate (BMR)** — the energy your body burns at complete rest — and your
**Total Daily Energy Expenditure (TDEE)** — that BMR scaled up by how active you are. Enter your
**age**, **sex**, **weight** and **height** (in metric or imperial units), pick an **activity
level**, and optionally choose a **BMR formula** and whether to see results in **Calories** or
**kilojoules**. Everything runs locally in your browser; nothing you type is uploaded.

It returns your **BMR**, your **TDEE** at the chosen activity level, the **activity multiplier**
used, your **BMI** and its category, calorie **goals** for cutting, maintaining and bulking, and
your **TDEE at all five activity levels** so you can see how much movement changes the number.

Three standard equations are supported. **Mifflin-St Jeor** (the default and most accurate for
most people today) and the revised **Harris-Benedict** both use age, sex, weight and height.
**Katch-McArdle** instead uses your **lean body mass** derived from a body-fat percentage, so it
ignores age, sex and height. TDEE is always `BMR × activity multiplier`, where the multipliers are
1.2 (sedentary), 1.375 (light), 1.55 (moderate), 1.725 (very active) and 1.9 (extra active).

### Example

Enter age **30**, sex **male**, weight **70**, height **175**, units **metric**, activity
**moderate**, formula **mifflin_st_jeor**:

```json
{
  "bmr": 1649.0,
  "tdee": 2556.0,
  "activity": "moderate",
  "activity_multiplier": 1.55,
  "formula": "mifflin_st_jeor",
  "energy_unit": "calories",
  "bmi": 22.9,
  "bmi_category": "normal",
  "goals": {
    "mild_loss": 2306.0,
    "loss": 2056.0,
    "extreme_loss": 1556.0,
    "maintain": 2556.0,
    "mild_gain": 2806.0,
    "gain": 3056.0
  },
  "tdee_by_activity": [ ... ],
  "summary": "BMR 1649 kcal (mifflin-st-jeor); TDEE 2556 kcal/day at moderate activity (×1.55)"
}
```

Mifflin-St Jeor gives BMR = `10×70 + 6.25×175 − 5×30 + 5 = 1649` kcal, and moderate activity
lifts that to `1649 × 1.55 = 2556` kcal/day. To lose about half a kilo (one pound) a week, eat
around the **loss** target of **2056** kcal; to gain, aim near the **gain** target of **3056**.

### Limits & notes

- Every field is optional and falls back to a sensible default (age **30**, **male**, **70**
  kg, **175** cm, **moderate** activity, **Mifflin-St Jeor**, **metric**, **Calories**).
- With imperial units, enter **weight in pounds** and **height in total inches** (e.g. 5'10" =
  **70**). BMI is reported for every formula.
- The activity multipliers (1.2–1.9) and goal offsets (±250, ±500, −1000 kcal/day) are the widely
  used industry conventions; goal targets are floored at 0 and are **estimates for planning, not
  medical advice**. Very low calorie targets are not automatically safe.
- **Macronutrient (protein/carb/fat) splits are not included** — there is no single standard
  ratio; use the calorie targets with your own preferred split.
- These are population averages with a typical error of a few percent; individual metabolism
  varies. Track your weight for a couple of weeks and adjust.

## FAQ

<!-- FAQ entries are <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>What's the difference between BMR and TDEE?</summary>

**BMR** (Basal Metabolic Rate) is the energy your body burns just to stay alive at complete
rest — breathing, circulation, cell repair — with no activity at all. **TDEE** (Total Daily
Energy Expenditure) is your *whole-day* burn: BMR plus everything you do — walking, exercise,
digesting food, fidgeting. This tool estimates TDEE by multiplying BMR by an activity factor
between 1.2 and 1.9. Your maintenance calories are your TDEE.

</details>

<details>
<summary>Which BMR formula should I choose?</summary>

**Mifflin-St Jeor** is the default and is generally the most accurate for the modern general
population, so use it unless you have a reason not to. **Harris-Benedict** (the revised 1984
version) is an older equation that tends to read a little higher. **Katch-McArdle** can be more
accurate *if you know your body-fat percentage*, because it works from lean body mass and ignores
age, sex and height — useful for lean or very muscular people. If you don't know your body fat,
stick with Mifflin-St Jeor.

</details>

<details>
<summary>How do I pick the right activity level?</summary>

Match it to a typical week, and be honest — most people overestimate. **Sedentary** (×1.2) is a
desk job with little or no exercise; **light** (×1.375) adds 1–3 easy workouts; **moderate**
(×1.55) is 3–5 solid sessions; **very active** (×1.725) is 6–7 hard sessions; **extra active**
(×1.9) is twice-a-day training or heavy physical labour. The tool shows your TDEE at *all five*
levels so you can compare. If your weight isn't moving as expected after two weeks, step the level
down.

</details>

<details>
<summary>How are the cutting and bulking targets calculated?</summary>

They are fixed daily offsets from your TDEE: **mild loss** is −250 kcal, **loss** is −500 kcal
(about 0.5 kg / 1 lb per week), **extreme loss** is −1000 kcal (about 1 kg / 2 lb per week), and
the two **gain** targets add +250 and +500 kcal. Roughly 7,700 kcal ≈ 1 kg of body fat, which is
where those weekly estimates come from. They're planning starting points, not guarantees — real
results depend on adherence, water weight and individual metabolism, and very aggressive deficits
aren't safe for everyone.

</details>

<details>
<summary>Can I get results in kilojoules instead of Calories?</summary>

Yes — set **Energy unit** to **Kilojoules** and every energy figure (BMR, TDEE, all goals and the
per-level TDEE) is converted using 1 kcal = 4.184 kJ. Note that "Calories" on nutrition labels
means kilocalories (kcal), which is what this tool uses by default.

</details>

<details>
<summary>Is my data sent anywhere?</summary>

No. The calculation runs entirely in your browser via WebAssembly. Nothing you enter is uploaded,
logged or stored — reload the page and it's gone.

</details>
