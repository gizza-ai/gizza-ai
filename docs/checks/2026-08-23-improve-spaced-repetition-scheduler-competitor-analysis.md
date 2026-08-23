# spaced-repetition-scheduler — competitor analysis (2026-08-23)

Scan run **before** implementing, per `/create-next-tool` step 4. Everything below is
**paraphrased** from public documentation and public tool pages — no competitor copy,
branding, trademarks, or assets are reproduced or reused. Out-of-model items are recorded
as "considered, not built", never forced into the descriptor.

Backlog row (`tools-to-build.csv:1632`): *"Computes the next review date for each card from
its review history using the SM-2 / FSRS algorithm."* — example prompt: *"Given my last
grades on these cards, when should I review each next?"*, type hint `pure`.

## Competitors surveyed

| # | What it is | Shape |
|---|---|---|
| 1 | A single-card web "flashcard spaced-repetition scheduler" calculator (miniwebtool) | One card, SM-2, form → next date |
| 2 | A course-site "spaced repetition scheduler" (IIENSTITU) | Deck-building app with Again/Hard/Good/Easy buttons |
| 3 | `supermemo2` (PyPI / alankan886) + `thyagoluciano/sm2` | Libraries: SM-2 state transition, returns next due date |
| 4 | The Anki-SM2 scheduler as documented by RemNote's help pages | Full app scheduler with tunable deck options |
| 5 | FSRS reference material: the `open-spaced-repetition` algorithm wiki, `py-fsrs`/`rs-fsrs`, and Expertium's technical write-up | Algorithm spec + libraries |

Unreachable/unsuitable candidates were replaced rather than run with fewer: generic
"interval calculator" SEO pages were dropped in favour of the two reference
implementations (3, 5), which document exact behaviour rather than marketing claims.

## Table stakes observed

### Capabilities

| Observed capability | Where seen | Decision |
|---|---|---|
| SM-2 core state: repetition count, ease factor, interval, next due date | 1, 3, 4 | **In-model — built.** Core `sm2` engine; all four are output columns. |
| 0–5 recall-quality grading (SuperMemo's original scale) | 1, 3 | **In-model — built.** `grade_scale = "sm2"`, plus `auto`. |
| Again / Hard / Good / Easy four-button grading | 2, 4, 5 | **In-model — built.** `grade_scale = "anki"`, word and letter aliases accepted. |
| Ease floor (an ease factor is never allowed below ~1.3 / 130 %) | 1, 3, 4 | **In-model — built.** `min_ease`, default 1.3. |
| Configurable starting ease (apps ship 2.5 / 230 % / 250 %) | 1, 4 | **In-model — built.** `ease_start`, default 2.5. |
| Fixed first/second intervals (1 day, then 6 days) before ease takes over | 1, 3 | **In-model — built.** `first_interval` (1), `second_interval` (6). |
| Easy bonus — an extra multiplier when the top grade is used | 4 | **In-model — built.** `easy_bonus`, default 1.3. |
| Hard interval — a sub-ease multiplier for the "passed with difficulty" grade | 4 | **In-model — built.** `hard_multiplier`, default 1.2. |
| Global interval modifier / multiplier to stretch or compress every interval | 4 | **In-model — built.** `interval_modifier`, default 1.0. |
| Lapse behaviour: reset to a short interval, optionally a % of the old one | 3, 4 | **In-model — built.** `lapse_multiplier`, default 0.0 = classic SM-2 restart. |
| Maximum interval cap | 4 | **In-model — built.** `max_interval`, default 36500 days. |
| Leech flagging after N lapses | 4 | **In-model — built.** `leech_threshold`, default 8, 0 = off. |
| FSRS three-component memory model (difficulty, stability, retrievability) | 5 | **In-model — built.** `algorithm = "fsrs"`, FSRS-6 with the published 21-weight default vector. |
| Desired-retention knob (FSRS schedules to the retention you ask for) | 5 | **In-model — built.** `desired_retention`, default 0.9. |
| User-optimised FSRS weight vector | 5 | **In-model — built.** `fsrs_weights` accepts a pasted 21-number vector; the default vector is used when blank. **Training** the vector is out-of-model (below). |
| Same-day / short-term review handling (two reviews on one date) | 5 | **In-model — built.** FSRS short-term stability formula fires when elapsed days is 0. |
| Projected timeline — "here are your next N reviews" | 1 | **In-model — built.** `output = "forecast"` with `forecast_reviews` and `forecast_grade`. |
| Due-queue view: only what is due now | 2, 4 | **In-model — built.** `only_due` flag + `sort = "due"` default + a `status` column (new / due today / overdue / scheduled / leech). |
| Interval fuzz (random ±jitter so cards do not clump) | 4 | **Considered, rejected.** Deliberately omitted: this tool is deterministic, so the same input always yields the same schedule and it is testable/diffable. Documented on the page as a stated limit rather than silently absent. |

### Input / output shape

| Observed | Decision |
|---|---|
| One-card-at-a-time forms (1, 2) | **Improved on, not copied.** Ours is batch-first: a pasted review log with many cards, which is what the backlog prompt asks for ("these cards", plural). A single card is just a one-line log. |
| Library input = current state (`repetitions`, `ease`, `interval`) rather than history (3) | **Both supported.** History replay is the default; a `state` input form lets a user paste the current card state instead of a full log. |
| CSV/TSV export of the schedule (4, as an app export) | **In-model — built.** `output = "csv"`, plus `json` for pipelines and an aligned `table` for reading. |
| Step-by-step formula breakdown of the calculation (1) | **In-model — built.** `output = "explain"` prints the per-review state transition for each card. |
| Animated forgetting-curve chart (1) | **Out-of-model.** The page renders text/media outputs, not bespoke charts; a chart is not expressible in the declarative page model. The numeric retrievability that a curve would plot **is** exposed (`retrievability` column under FSRS), so the data is not lost. |
| Calendar (`.ics`) export of due dates | **Considered, rejected** for a first version: it is expressible, but it would double the output surface for a format the CSV already feeds. Recorded here as a candidate for a later improve pass. |

### UX controls

| Observed | Decision |
|---|---|
| Preset "quick example" buttons (new card recalled well / mature card struggled / lapse) (1) | **In-model — built** as `[[example]]` preset chips — the declarative equivalent in this repo's generator. |
| Dropdown for recall quality with the meaning of each grade spelled out (1) | **In-model — built.** Enum params render as `<select>`; `[input.labels]` carries the plain-English meaning of each choice. |
| Grading buttons labelled by outcome, not by number (2, 4) | **In-model — built** via `[input.labels]` on `grade_scale` and `forecast_grade`, and by accepting `again`/`hard`/`good`/`easy` words in the log itself. |
| Numeric fields for deck options with the shipped default pre-filled (4) | **In-model — built.** Every numeric field has a real placeholder showing the default. |
| Slider for desired retention (FSRS UIs) | **In-model — built.** `kind = "slider"` on `desired_retention`, step 0.01. |

### Out-of-model (considered, not built)

- **FSRS weight optimisation / training** — fitting a personal 21-weight vector needs
  gradient descent over a full multi-year review history and, in practice, a training
  run measured in seconds-to-minutes over tens of thousands of rows. Out of scope for a
  synchronous browser-local tool; we consume a vector, we do not train one.
- **Accounts, deck sync, review history storage** — no server, no accounts by design.
- **Actually quizzing the user** (showing card fronts, collecting grades interactively) —
  that is a flashcard app, not a scheduler; this tool computes the schedule from grades
  a user already has.
- **Forgetting-curve chart rendering** — see above; the numbers are exposed instead.
- **Load balancing / workload smoothing across days** — depends on a whole-collection
  daily-limit model and would make output non-deterministic per card; not attempted.
- **Interval fuzz** — see above, deliberately deterministic.

## Positioning

Every competitor found is either a *single-card* calculator or a *whole application*.
The gap this fills is the batch middle: paste a review log for a whole deck, get every
card's next review date in one deterministic, offline, no-account run — with both the
classic SM-2 family and FSRS-6 available from the same input, so the two can be compared
directly on the same history.
