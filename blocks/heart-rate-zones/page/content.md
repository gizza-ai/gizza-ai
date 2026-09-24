## About this tool

Heart-rate training zones turn a single "go easy" or "go hard" instruction into numbers your
watch can hold you to. This calculator takes your age and resting pulse, works out your maximum
heart rate and your heart-rate reserve, and prints every zone as a beats-per-minute range with
the kind of training each one is for.

Three methods are available, because the zone numbers you see elsewhere depend on which one was
used:

- **Karvonen (heart-rate reserve)** — the default. Each zone is a percentage of your *reserve*
  (maximum minus resting) added back on top of resting: `target = (max − resting) × % + resting`.
  Because your resting rate is part of the sum, two people with the same maximum get different
  zones, which is the point.
- **Percent of maximum heart rate** — plain percentages of your maximum, ignoring resting pulse.
  This is what most printed charts and watch defaults use, and its zones sit noticeably lower
  than Karvonen's for the same person.
- **Zoladz** — five bands placed at fixed distances below maximum (max−50, max−40, max−30,
  max−20, max−10, each ±5 bpm) instead of percentages.

You can also switch the zone model: the familiar five zones, the three-zone polarized model many
endurance coaches program against, or the two American Heart Association activity bands that
public-health guidance is written in terms of.

Everything runs locally in your browser as WebAssembly — your age and pulse are never sent
anywhere.

## Worked example

A 30-year-old with a resting heart rate of 60 bpm, using the defaults (Tanaka estimate, Karvonen
method, five zones):

```
Maximum HR      208 − 0.7 × 30 = 187 bpm
Reserve         187 − 60 = 127 bpm
Zone 1  Recovery      50–60%    124–136 bpm
Zone 2  Aerobic base  60–70%    136–149 bpm
Zone 3  Tempo         70–80%    149–162 bpm
Zone 4  Threshold     80–90%    162–174 bpm
Zone 5  VO2 max      90–100%    174–187 bpm
```

Zone 2 is the band most easy mileage should sit in: 136–149 bpm for this person. Set the single
target intensity to `70` and you also get one line, `Target at 70% intensity: 149 bpm`, which is
the form most training plans write a prescription in.

Switch the method to percent-of-max and the same person's zone 2 becomes 112–131 bpm — the same
label, ~18 bpm lower. Always note which method a plan's numbers came from before following them.

## Limits and edge cases

- **Age 5–120 years, resting 25–120 bpm, measured maximum 100–250 bpm.** A measured maximum must
  be above the resting rate; anything outside these ranges is rejected with a message saying what
  was expected.
- **Age formulas carry roughly ±7–12 bpm of individual spread.** They describe population
  averages, so a real maximum from a maximal test, a hard race finish, or the highest value your
  monitor has ever logged makes every zone meaningfully more accurate. Enter it in the measured
  field and the formula is ignored.
- **Zone edges are rounded to whole beats and therefore touch.** The value that ends one zone
  starts the next (136 bpm above); at a boundary, either label is fine.
- **Zoladz always returns five zones** whatever the zone-model selector says, because it is
  defined by fixed bpm offsets rather than percentages.
- Resting heart rate has no effect under percent-of-max — that method deliberately ignores it.
- **Heat, altitude, illness and caffeine shift your heart rate for the same effort**, typically by
  5–10 bpm. The zones here are computed from the numbers you enter; they are not adjusted for
  conditions, so on a hot day expect the same pace to read a zone higher.
- **Six age formulas are offered, not one.** Tanaka, Fox, Gulati, Nes and Inbar are linear fits;
  Oakland (`192 − 0.007 × age²`) is nonlinear and diverges from the others most at the ends of the
  age range. They disagree by up to ~10 bpm for the same person, which is itself a good reason to
  measure your maximum.
- **Zones anchored to a lactate threshold (LTHR) or to power (FTP) are out of scope.** Everything
  here is anchored to a maximum heart rate, measured or estimated. If your plan is written against
  a threshold test, use the numbers from that test rather than converting them here.
- This is arithmetic on the numbers you type, not medical advice. If you take heart-rate-altering
  medication (beta blockers especially), have a cardiac condition, or are returning from illness,
  zones derived from an age formula can be badly wrong for you.

## FAQ

<details>
<summary>Karvonen or percent of max — which should I use?</summary>

Karvonen, unless you are following a plan that was written in percent-of-max. Karvonen includes
your resting heart rate, so it adapts as your fitness changes and it does not put a fit person
with a 45 bpm resting pulse in the same zone 2 as an untrained person with a 75 bpm pulse.
Percent-of-max is simpler and matches most published charts, but its zones read 15–20 bpm lower
for the same person, which is why the two methods appear to disagree.

</details>

<details>
<summary>How do I measure my resting heart rate properly?</summary>

Take it lying down immediately after waking, before you get up or look at your phone. Count your
pulse for a full 60 seconds, or read it off a chest strap or watch. Do it on three to five
consecutive mornings and average the numbers — a single reading can be 5–10 bpm off because of
caffeine, alcohol, a poor night's sleep, or a cold coming on. Rough guide: 40–50 bpm for elite
endurance athletes, 50–60 for well-trained, 60–80 for most adults.

</details>

<details>
<summary>Why does the tool default to 208 − 0.7 × age instead of 220 − age?</summary>

`220 − age` is a convenient rule of thumb from the 1970s that was never derived as a research
finding. Tanaka's `208 − 0.7 × age`, fitted across many studies, tracks measured maxima better —
it reads lower for the young and higher for the old. The classic formula is still available as
the `fox` option because so many charts and gym posters use it, along with `gulati`
(`206 − 0.88 × age`, from a large treadmill study of women), `nes` (`211 − 0.64 × age`), `inbar`
(`205.8 − 0.685 × age`) and the nonlinear `oakland` fit (`192 − 0.007 × age²`). Switching between
them is the cheapest way to see how much of a "zone" is really just formula choice.

</details>

<details>
<summary>Where is the fat-burning zone?</summary>

It is zone 2 in the five-zone model — roughly 60–70% here. That band burns the highest *share* of
its calories from fat, which is where the name comes from, but harder work burns more fat in
absolute terms per minute as well as more carbohydrate. Some calculators publish separate
fat-burning bands for men and women; this one does not, because the difference between those
published bands is smaller than the ±7–12 bpm error in an age-estimated maximum. Train zone 2 for
the aerobic adaptations, not for a fat-burning window.

</details>

<details>
<summary>What is the three-zone polarized model for?</summary>

It is the way a lot of endurance coaching is actually programmed: easy below the first lactate
turn point (roughly 50–81% here), moderate in the band between the turn points (81–87%), and hard
above the second turn point (87–100%). The idea is to spend most of your week in zone 1 and the
rest genuinely hard, rather than drifting into the moderate middle. It maps onto the five-zone
model — three is roughly five-zone 1–2, then 3–4, then 5 — but it makes the "don't live in the
grey zone" split explicit.

</details>

<details>
<summary>My watch shows different zones than this calculator. Which is right?</summary>

Both, probably — they are answering slightly different questions. Watches usually default to
percent-of-max with a `220 − age` maximum, and some use a maximum they learned from your own
hard efforts, or a lactate-threshold estimate rather than a maximum at all. Check which method
and which maximum your watch is using, then set the same ones here and the numbers will line up.
If your watch has recorded a higher heart rate than any formula predicts, that recorded value is
better evidence than the formula — use it as the measured maximum.

</details>

<details>
<summary>Can I use this if I take beta blockers or have a heart condition?</summary>

Not as-is. Beta blockers lower both resting and maximum heart rate, often substantially, so an
age-estimated maximum will be too high and every zone will be set too hard. The same applies
after a cardiac event or with an arrhythmia or a pacemaker. If a clinician has given you a
measured maximum or a prescribed target range, enter the measured maximum here and use their
range; otherwise ask them rather than an age formula.

</details>
