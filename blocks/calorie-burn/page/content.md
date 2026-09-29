## About this tool

The energy cost of an activity is conventionally expressed as a **MET** — a metabolic equivalent
of task. One MET is what your body spends sitting still: roughly 1 kcal per kilogram of body
weight per hour, or an oxygen uptake of 3.5 ml/kg/min. An activity rated at 8 METs costs about
eight times that. Multiply the MET value by your weight and by how long you moved and you have an
estimate of the calories the session cost:

```
kcal per minute = MET × 3.5 × body weight in kg / 200
```

This calculator carries a 59-entry table of standard MET values — everyday and household tasks,
walking and hiking, running, cycling, swimming and water sports, gym and studio work, and field
sports — plus a **custom** option and a MET override for any entry, so you can dial intensity
within an activity or type in a value from a published compendium table that is not in the list.

Weight is accepted in kilograms or pounds and duration in minutes or hours; both are converted
before the arithmetic and echoed back in both forms so you can check nothing was misread.

Two things it reports that most calculators do not:

- **Net as well as gross energy.** The gross figure includes the ~1 MET you would have burned
  existing anyway, which is what almost every tracker shows. Set the basis to **net** and it
  charges `MET − 1` instead — the honest number for calories the session *added*. For a moderate
  walk that is about 30% lower.
- **MET-minutes.** `MET × minutes` is the unit public-health guidance is actually written in;
  500–1000 MET-minutes a week is the usual target. The summary says how many sessions like the one
  you entered reach the bottom of that range.

Every output also prices the same weight and duration across eight reference activities, so you
can see what swapping the walk for a swim would be worth without running the tool again.

Everything runs locally in your browser as WebAssembly — your weight is never sent anywhere.

## Worked example

A 70 kg person walking at a moderate 3 mph for 30 minutes, on the defaults:

```
Calories burned — Walking, moderate (3 mph / 4.8 km/h)

Body weight 70 kg (154.3 lb) · duration 30 min · MET 3.5 (Compendium value for this activity)
Basis: gross — all energy used during the session, resting metabolism included

Energy burned        129 kcal
Per minute           4.3 kcal/min
Per hour             257 kcal/h
Activity volume      105 MET-minutes — 4.8 such sessions reach the 500 MET-min/week minimum
Oxygen uptake        12.3 ml/kg/min — 25.7 L of oxygen over the session
Body-fat equivalent  17 g — at 7700 kcal per kg of body fat

Same 30 min at 70 kg for comparison
  Sitting, desk work                      1.5 MET      55 kcal
  Walking, moderate (3 mph / 4.8 km/h)    3.5 MET     129 kcal
  Walking, brisk (4 mph / 6.4 km/h)         5 MET     184 kcal
  Cycling, moderate (12-14 mph)             8 MET     294 kcal
  Jogging, general                          7 MET     257 kcal
  Running, 6 mph / 9.7 km/h (10 min/mi)   9.8 MET     360 kcal
  Swimming laps, freestyle moderate       8.3 MET     305 kcal
  Weight training, vigorous                 6 MET     221 kcal
```

Step by step: `3.5 MET × 3.5 × 70 kg / 200 = 4.29 kcal/min`, and `4.29 × 30 = 129 kcal`.

Switch the basis to **net** and the same walk reports **92 kcal** — `2.5 MET` is charged instead of
`3.5`, because about 37 kcal of that half hour is metabolism you would have paid for sitting on
the sofa. The MET-minutes stay at 105 either way; that figure is defined on the gross value.

At 105 MET-minutes, five walks like this clear the 500 MET-min/week floor — which is the same
thing as the familiar "150 minutes of moderate activity a week".

## Limits and edge cases

- **Ranges.** Body weight 20–300 kg (44.1–661.4 lb), duration 0.5–1440 minutes (up to 24 hours),
  MET override 0.5–30. Anything outside these is rejected with a message saying what was expected
  and what it received, in the unit you used.
- **This is an estimate, not a measurement.** MET values are population averages measured on a
  reference adult. Two people of the same weight doing the same activity can differ by 20–30%
  through fitness, technique, terrain, gradient, wind, water temperature and how efficiently they
  move. Treat the number as a ballpark, and treat *changes* in it over time as more trustworthy
  than any single figure.
- **Age, sex, height and body composition are not inputs**, because the MET formula does not use
  them — energy cost scales with total mass. That also means a lean and a heavy person of the same
  weight get the same answer, though the leaner one is probably working less hard. For whole-day
  expenditure that does account for age, sex and height, use a TDEE calculator instead.
- **MET values assume the effort is sustained.** Enter moving time, not elapsed time: a 60-minute
  gym visit with 25 minutes of standing around is closer to 35 minutes of `circuit-training`.
- **Net is floored at zero.** `sleeping` is rated below 1 MET (0.95), so on the net basis it
  reports 0 kcal rather than a negative burn.
- **MET-minutes and oxygen uptake always use the gross MET**, even when the basis is net — that
  is how both quantities are defined.
- **The body-fat equivalent uses the conventional 7700 kcal per kg** of adipose tissue. It is an
  arithmetic conversion, not a prediction: real weight change depends on total intake, water,
  glycogen and adaptation, and burning 500 kcal does not reliably remove 65 g of fat.
- **Rounding.** Calories, calories per hour and fat grams are whole numbers; rates, MET-minutes
  and litres of oxygen are shown to one decimal. Totals are computed at full precision and rounded
  once at the end, so recomputing from the rounded per-minute rate can be a calorie or two out.
- **No distance or pace mode.** You enter duration, not "5 km at 5:30/km". Convert to minutes
  first, or pick the MET entry whose stated speed matches your pace and let the duration carry the
  rest.
- **Not medical or dietary advice.** If you are managing a condition, are pregnant, or are
  following a prescribed energy target, the arithmetic here is not a substitute for a clinician's
  numbers.

## FAQ

<details>
<summary>Why does my watch report a different number of calories?</summary>

Two reasons. First, most wearables estimate from heart rate (and sometimes movement and a
personal profile), not from a MET table, so they respond to how hard *you* found the session
rather than what the activity costs on average. Second, watches almost always report gross
calories and often add your resting burn for the whole period. Set the basis here to **gross**
for the closest comparison. Even then, agreement inside 15–20% is about as good as it gets, and
independent tests of wrist devices commonly find energy-expenditure errors larger than that.

</details>

<details>
<summary>Should I use the net or the gross figure?</summary>

Use **gross** when comparing against a tracker, a gym machine or another calculator, since that
is what they all report. Use **net** when you want to know what the session *added* — for example
when you are counting calories and have already accounted for your resting metabolism or your
whole-day TDEE elsewhere. Adding a gross exercise figure on top of a TDEE that already includes an
activity factor double-counts, which is the single most common way these numbers get inflated.

</details>

<details>
<summary>My activity is not in the list. What do I do?</summary>

Pick the closest entry and correct it with the MET override, or select **Custom** and enter the MET
value yourself. Rough anchors: 2–3 METs is light effort you could sustain all day, 3–6 is moderate
(you can talk but not sing), 6–9 is vigorous (speech comes in short phrases), and above 9 is hard
interval work you could not hold for an hour. The override also relabels the output so it is clear
the number is yours, not the table's.

</details>

<details>
<summary>What are MET-minutes and why should I care?</summary>

`MET × minutes` is a single number for the *volume* of activity, which is how public-health
recommendations are actually specified — commonly 500–1000 MET-minutes a week. It is useful
because it trades intensity against time automatically: 150 minutes of moderate walking at 3.5
METs and 75 minutes of running at 7 METs both land around 525, which is why the guidance can be
stated either way. Calories cannot do this, because they also scale with body weight, so two
people hitting the same guideline report very different calorie totals.

</details>

<details>
<summary>Does a heavier person really burn more for the same workout?</summary>

Yes, and roughly in proportion to weight, which is exactly what the formula says: moving more
mass costs more energy. An 80 kg person burns about 14% more than a 70 kg one on the same activity
for the same time. The flip side is that as you lose weight the same session burns slightly less,
which is one reason progress tends to flatten — re-run the calculation at your current weight
rather than the one you started at.

</details>

<details>
<summary>What is the oxygen uptake figure for?</summary>

It is the same intensity expressed the way exercise physiology measures it: 1 MET is an uptake of
3.5 ml of oxygen per kilogram per minute, so a MET value multiplied by 3.5 gives ml/kg/min, and
the total litres tell you how much oxygen the session consumed. It is handy for comparing an
activity against a measured VO₂ max — an 8 MET activity needs 28 ml/kg/min, which is a comfortable
share of a fit adult's capacity and close to all of a deconditioned one's. The conversion also
shows why the whole method is an approximation: it assumes your oxygen cost per MET is the
reference value.

</details>

<details>
<summary>Can I use this to work out how much I need to exercise to lose weight?</summary>

Only as a sanity check. The body-fat equivalent line converts calories at the conventional 7700
kcal per kilogram, so a 500 kcal session is about 65 g. That arithmetic is correct and the
conclusion it invites — that exercise alone is a slow route to weight change — is broadly right.
But real energy balance is not that tidy: appetite and spontaneous movement adjust, fat is not the
only tissue that changes, and a large share of day-to-day weight is water and glycogen. Diet moves
the intake side far faster; this tool is better used to compare activities than to plan a deficit.

</details>
