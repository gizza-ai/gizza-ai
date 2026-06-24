## Read Apache Avro files as JSON

Apache Avro is a compact, schema-driven binary format used widely in data
pipelines (Kafka, Hadoop, Spark). An **Object Container File** (`.avro`, often
called an **OCF**) bundles the records together with the **writer schema** that
describes them — so the file is fully self-describing. This tool reads that
container and turns the records back into plain JSON, right in your browser.

Paste the file's bytes as **base64** or **hex** and pick how you want the
output. No `.avsc` schema file is required, because the schema travels inside
the container.

## Output formats

- **Records** — a pretty-printed JSON array of every record in the file. Best
  for reading or piping into another JSON tool.
- **NDJSON** — newline-delimited JSON, one compact record per line. Handy for
  streaming into log tools or loading row-by-row.
- **Full** — an object with the embedded **writer schema**, the record
  **count**, and the **records**. Use it when you want to see exactly which
  schema the file was written with.

## How values are decoded

- Avro logical types are unwrapped: dates and timestamps come through as their
  underlying integers, a `uuid` becomes its string form, and `decimal`,
  `bytes`, and `fixed` values are base64-encoded so the raw bytes survive.
- Unions are flattened to the actual branch value, and records, arrays, and
  maps map onto JSON objects and arrays directly.

## Common uses

- Inspect an `.avro` file dumped from a Kafka topic or a data lake.
- Convert Avro records to JSON or NDJSON for quick `jq`-style exploration.
- Confirm which schema an Avro file was actually written with.

This tool reads **container files** that embed their schema — not bare,
single-object-encoded Avro values, which carry no schema on their own.
Everything runs locally in your browser via WebAssembly — your data never
leaves your machine.
