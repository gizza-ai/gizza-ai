# critical-path-calculator — competitor analysis (2026-07-25)

Scan of the top online Critical Path Method (CPM) calculators to fix table-stakes
capabilities, defaults, and UX patterns before building. All findings paraphrased;
no competitor copy, branding, or trademarks reused.

## Competitors reviewed

1. **pmcalculators.com — Critical Path Method Calculator** — per-activity name,
   duration (or PERT optimistic/most-likely/pessimistic), and predecessors; up to
   50 activities; computes ES/EF, LS/LF, slack, project duration, critical path;
   draws an AON network diagram (red = critical); PERT variance/std-dev and
   completion-probability; project crashing cost analysis.
2. **atozmath.com — PERT/CPM calculator** — activity + predecessors + duration (or
   three-point PERT); outputs critical path, total float, free float, independent
   float, ES/EF/LS/LF; AOA and AON diagrams; crashing.
3. **artifactly.ai — CPM Calculator** — critical path, ES/EF, LS/LF, float per
   activity; "schedule optimization"; results table.
4. **constructioncalculators.net — CPM Calculator** — map tasks + dependencies,
   compute the critical path; table output.
5. **easycalculation.com — PERT/CPM calculator** — activity durations + predecessors
   → critical path and expected project time; PERT Te formula.

## Table-stakes → decision

| Capability | Competitors | Ours | Notes |
|---|---|---|---|
| Per-task name + duration + predecessors | all | **in** | `name, duration[, pred...]` per line |
| Earliest start / earliest finish | all | **in** | forward pass |
| Latest start / latest finish | all | **in** | backward pass |
| Total float (slack) | all | **in** | LS − ES |
| Free float | atozmath, artifactly | **in** | min(successor ES) − EF |
| Critical path | all | **in** | zero-float chain; one representative path shown |
| Total project duration | all | **in** | max EF |
| PERT three-point estimate (o/m/p → Te) | pmcalc, atozmath, easycalc | **in** | `2/4/9` duration token → (o+4m+p)/6 |
| Cycle / unknown-dependency detection | (implicit) | **in** | explicit error, better than most |
| JSON / machine-readable output | none | **in (bonus)** | `format = json` for LLM/CLI use |
| Independent float | atozmath | **out (deferred)** | rarely used in practice; total+free float cover the common need. Listed, not silently dropped. |
| AOA / AON network **diagram** | pmcalc, atozmath | **out-of-model** | this repo renders a text/number page; a rendered graph would need an SVG/canvas layout engine. Table output covers the numbers; a future `format` could emit DOT/Mermaid. |
| PERT variance / std-dev / completion probability | pmcalc, atozmath | **out (deferred)** | separate statistical concern; the core PERT expected-time table-stake is in. Could be its own `pert-analysis` tool. |
| Project **crashing** (time/cost optimization) | pmcalc, atozmath | **out-of-scope** | needs per-task cost/crash-cost inputs — a distinct optimization tool, not CPM scheduling. |

## UX patterns adopted

- **Preset example chips** (competitors ship worked examples): three `[[example]]`
  chips — a 5-task sample project, a PERT-estimate list, and a JSON-output run.
- **Multiline task textarea** with a realistic multi-line placeholder.
- **Output-format `<select>`** (report vs json), schema-derived.
- Worked example (input + expected 13-duration, `A -> B -> D -> E` critical path)
  and stated limits/edge cases (cycles, unknown predecessors) on the page.

## Out-of-model / deferred summary

Network **diagram rendering**, **independent float**, **PERT probability/variance**,
and **project crashing** are the only competitor features not built. Diagrams and
crashing are genuinely out of the current pure-text page model; independent float and
PERT statistics are in-model but deferred to keep this tool focused on the CPM
schedule (they are candidates for a separate PERT/analysis tool). Every other
table-stake ships in the descriptor.
