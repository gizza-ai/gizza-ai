## About this tool

The **Period Predictor** projects your upcoming menstrual cycles from two things
you already know: the **first day of your last period** and your **average cycle
length** (the number of days from the start of one period to the start of the
next — 28 days for many people, but anywhere from about 20 to 45 is common).
From there it lists your next several period start dates, and for each cycle it
also shows:

- the **weekday** the period is expected to start on,
- the **bleeding-end date** (period start + your period length − 1),
- the **estimated ovulation day** — roughly your luteal-phase length before that
  period, since the luteal phase stays fairly constant even when cycle length
  changes, and
- the **6-day fertile window** — the five days before ovulation through
  ovulation day, when conception is most likely.

Everything runs **locally in your browser** — your dates are never uploaded to a
server, there is no sign-up, and the tool is free. Adjust the cycle length,
period length, luteal phase and how many cycles to project with the sliders, or
deep-link a prediction by putting the values straight in the URL
(`?last_period=2026-07-01&cycle_length=28&cycles=6`).

These predictions are **estimates**. Real cycles drift by a few days with stress,
illness, travel, sleep and other factors, and irregular cycles are inherently
harder to predict. This tool is **not a contraceptive method** and is **not
medical advice** — talk to a healthcare professional about anything concerning.

## FAQ

<details>
<summary>How does the predictor calculate my next period?</summary>

It adds your **cycle length** to the first day of your last period, and repeats
that for as many cycles as you ask for. With a last period of `2026-07-01` and a
28-day cycle, the next start is `2026-07-29`, then `2026-08-26`, `2026-09-23`,
and so on. Each date is labelled with its weekday and its bleeding-end date
(start + period length − 1).

</details>

<details>
<summary>How are ovulation and the fertile window estimated?</summary>

Ovulation is estimated as the predicted period start **minus your luteal-phase
length** (default 14 days), because the luteal phase — the time between
ovulation and the next period — stays fairly constant even when cycle length
varies. The **fertile window** is the six days ending on ovulation day: the five
days before it (sperm can survive several days) plus ovulation day itself.

</details>

<details>
<summary>What if my cycles are irregular?</summary>

Predictions are only as steady as your cycles. If your cycle length varies a lot
month to month, use your **average** and treat every date as a rough guide — the
real day can land a few days either side. For very irregular cycles the fertile
window and ovulation estimates are especially approximate. This tool is an
estimate, **not a contraceptive method** and **not medical advice**.

</details>

<details>
<summary>Is my data private?</summary>

Yes. All the math runs **in your browser** using local WebAssembly — the dates
you enter are never sent to a server, there is no account, and nothing is stored
after you close the page. You can even use the tool offline once it has loaded.

</details>
