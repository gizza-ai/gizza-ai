## About this tool

The **Critical Path Method (CPM)** turns a list of tasks — each with a duration and
a set of predecessors that must finish first — into a full project schedule. This
calculator runs the standard forward and backward passes to find:

- the **total project duration** (the length of the longest dependency chain),
- the **critical path** — the sequence of tasks with zero slack, where any delay
  pushes out the whole project,
- and, for every task, its **earliest start / finish**, **latest start / finish**,
  **total float** (slack) and **free float**.

Everything runs locally in your browser — no accounts, no uploads.

### Input format

Enter one task per line as `name, duration, predecessor, predecessor, ...`:

```
A, 3
B, 4, A
C, 2, A
D, 5, B, C
E, 1, D
```

`duration` is a plain number, or a **PERT three-point estimate**
`optimistic/most-likely/pessimistic` such as `2/4/9`, which is reduced to the
expected time `(o + 4·m + p) / 6`. Blank lines and lines starting with `#` are
ignored.

### Worked example

For the five tasks above, the tool reports a **project duration of 13** and a
**critical path of `A -> B -> D -> E`**. Task `C` is the only non-critical task:
it has a duration of 2 but sits on a branch with 2 units of slack (total float),
so it can slip up to 2 time units without delaying the project. The per-task table
shows, for example, that `C` has earliest start 3, earliest finish 5, latest start
5, latest finish 7 — confirming its float of 2.

## FAQ

<details>
<summary>What is the critical path?</summary>

The critical path is the longest chain of dependent tasks through your project. Its
length equals the shortest possible project duration, and every task on it has zero
float — delaying any of them delays the entire project. There can be more than one
critical path; this tool marks every zero-float task as critical and shows one
representative path.

</details>

<details>
<summary>What is the difference between total float and free float?</summary>

**Total float** (or slack) is how long a task can be delayed without pushing out the
project finish. **Free float** is how long it can be delayed without delaying its
own successors' earliest start. Free float is always less than or equal to total
float, and both are zero for critical tasks.

</details>

<details>
<summary>How do earliest and latest start/finish get calculated?</summary>

A forward pass sets each task's **earliest start** to the maximum earliest finish of
its predecessors (0 if it has none), and earliest finish = earliest start + duration.
A backward pass then sets each task's **latest finish** to the minimum latest start
of its successors (the project duration if it has none), and latest start = latest
finish − duration. Total float is latest start − earliest start.

</details>

<details>
<summary>Can I use PERT three-point estimates?</summary>

Yes. Write a duration as `optimistic/most-likely/pessimistic`, e.g. `2/4/9`. The
tool uses the PERT expected time `(o + 4·m + p) / 6` as the task duration. You can
mix plain numbers and three-point estimates in the same task list.

</details>

<details>
<summary>What happens if my dependencies contain a loop?</summary>

A project schedule must be acyclic, so if the tasks reference each other in a cycle
(for example `A` depends on `C`, `C` depends on `B`, `B` depends on `A`) the tool
reports an error listing the tasks involved instead of producing a schedule. It also
errors if a task lists a predecessor that isn't defined.

</details>
