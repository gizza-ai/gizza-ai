# reservoir-sampler — competitor analysis (2026-09-04)

Scan run before finishing the implementation. Search topic: online random line picker / random sample of rows / reservoir sampling. The reviewed tools and docs were paraphrased.

## Competitors reviewed

### 1. Browser random line pickers
- Inputs: a pasted list, sometimes with one item per line and optional duplicate handling.
- Outputs: one or more random picked lines.
- UX: textarea, count field, randomize button, sometimes a clear/copy control.
- Gap: most tools load the whole list and do not describe uniform sampling from streams.

### 2. CSV/random row sampling utilities
- Inputs: table rows, requested sample size or percentage, optional header preservation.
- Outputs: sampled rows as text/CSV.
- UX: count field, checkbox for header row, checkbox to skip blanks, and download/copy output.
- Gap: many are not seeded, so users cannot reproduce a draw.

### 3. Reservoir sampling algorithm references and demos
- Inputs: stream size, reservoir size, and algorithm choice.
- Outputs: sample plus explanation of Algorithm R or L.
- UX: educational controls emphasize `k`, stream pass count, and reproducible seeds.
- Gap: algorithm demos often do not work as practical paste-in tools for logs or CSV rows.

## Table-stakes checklist and decisions

| Capability | Seen at | In/out of model | Decision |
| --- | --- | --- | --- |
| Paste one-record-per-line data | random line tools | in-model | `data` textarea |
| Choose sample size | all categories | in-model | `k` integer, default 10, capped at 1,000,000 |
| Uniform sampling without replacement | algorithm demos | in-model | Algorithm R and Algorithm L implementations |
| Reproducible draw | algorithm demos, some data tools | in-model | `seed` parameter with deterministic splitmix64 PRNG |
| Skip blank records | list utilities | in-model | `skip_empty` checkbox defaults true |
| Preserve CSV header | row samplers | in-model | `header` checkbox excludes and echoes first line |
| Plain and numbered output | list utilities | in-model | `format=lines` or `numbered` |
| JSON output for pipelines | developer tools | in-model | `format=json` |
| Draw order vs source order | random pickers/data samplers | in-model | `order=input` or `reservoir` |
| Stats / inclusion probability | algorithm demos | in-model | `stats=true` adds scanned/sample/probability metadata |
| Streaming file upload | data tools | out-of-model here | pure text field only in this toolkit page |
| Cryptographic randomness | some randomizers claim it | out-of-model | deterministic seed is preferred for reproducible checks |
| Weighted sampling | separate algorithm family | out-of-model | not built; this is uniform fixed-size sampling |

## Design conclusions carried into the descriptor

- Make deterministic seeding explicit; it is a feature, not a weakness, for audits and repeatable examples.
- Include both Algorithm L and Algorithm R so users can choose the textbook or skip-based variant.
- Header preservation and numbered output are necessary for CSV/log workflows.
- Keep the output text-first, with JSON available for automation.
