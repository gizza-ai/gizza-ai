## About this tool

MongoDB stores documents as BSON, which has types plain JSON does not: ObjectId,
64-bit integers, Decimal128, binary, regular expressions, timestamps. **Extended
JSON** is the text encoding that keeps those types alive, by wrapping each value
in a `$`-prefixed marker — `{"$oid": "..."}`, `{"$date": "..."}`,
`{"$numberLong": "42"}`. It is exactly what `mongoexport`, driver `EJSON`
helpers, aggregation playgrounds, and change-stream logs emit, and it is noisy
to read or paste into an app that just wants ordinary JSON.

Use **Extended JSON → plain JSON** when you copied a document out of a
`mongoexport` dump, a log line, or a driver response and want a readable object.
Use **Plain JSON → Extended JSON** when you are preparing a fixture, a
`mongoimport` file, or a test document that must carry exact types.
**Auto-detect** unwraps input that already contains a recognised `$`-wrapper and
wraps anything else.

The **dialect** control also makes this a canonical↔relaxed normaliser: pick
*Plain JSON → Extended JSON* with Extended JSON already in the box, and the
document is re-emitted in whichever dialect you chose. Relaxed keeps bare numbers
and ISO-8601 dates for readability; canonical wraps every number and date so the
exact BSON type survives a round trip.

Conversion is deterministic and entirely local: no MongoDB driver, no connection
string, no network request. It only reshapes the JSON you paste.

## Worked examples

Extended JSON input:

```json
{"_id":{"$oid":"507f1f77bcf86cd799439011"},"created":{"$date":{"$numberLong":"1721485800000"}},"views":{"$numberLong":"42"},"name":"Ada"}
```

Plain JSON output:

```json
{
  "_id": "507f1f77bcf86cd799439011",
  "created": "2024-07-20T14:30:00Z",
  "views": 42,
  "name": "Ada"
}
```

Plain JSON input, converted to the canonical dialect with **Detect ObjectIds and
dates in strings** turned on:

```json
{"_id":"507f1f77bcf86cd799439011","created":"2024-07-20T14:30:00Z","views":42,"score":1.5}
```

Canonical Extended JSON output:

```json
{
  "_id": { "$oid": "507f1f77bcf86cd799439011" },
  "created": { "$date": { "$numberLong": "1721485800000" } },
  "views": { "$numberInt": "42" },
  "score": { "$numberDouble": "1.5" }
}
```

## Limits and edge cases

- **Document key order is preserved.** BSON is an ordered format, so re-sorting
  keys can change meaning (index specs, command documents, `$`-operator order).
- **Legacy spellings are accepted on input** and normalised on output:
  `{"$date": 1721485800000}`, `{"$binary": "...", "$type": "00"}`, and
  `{"$regex": "...", "$options": "i"}` all parse.
- **DBRef is not a wrapper.** `{"$ref": ..., "$id": ..., "$db": ...}` is an
  ordinary subdocument in BSON, so it round-trips as one; its `$id` is still
  converted.
- **`Infinity`, `-Infinity`, and `NaN` always unwrap to strings**, because JSON
  has no literal for them. There is no lossless numeric alternative to offer.
- **Very large numbers.** JSON parsers in JavaScript lose precision above
  2^53 − 1. Turn on *Keep 64-bit and decimal values as strings* to unwrap
  `$numberLong` and `$numberDecimal` as strings instead of numbers.
- **Dates outside years 1970–9999** keep the canonical `{"$date": {"$numberLong":
  ...}}` form in relaxed mode, matching the specification rather than emitting an
  out-of-range ISO string.
- **Binary values pass through as base64** plus their subtype; the tool does not
  decode binary payloads or reinterpret legacy UUID subtypes.
- Input must be a single JSON value. Newline-delimited JSON dumps need splitting
  into individual documents first.

## FAQ

<details>
<summary>What is the difference between canonical and relaxed Extended JSON?</summary>

Relaxed mode favours readability: integers and doubles are written as plain JSON
numbers, and dates as ISO-8601 strings. Canonical mode favours type fidelity:
every number is wrapped as `$numberInt`, `$numberLong`, or `$numberDouble`, and
every date as `{"$date": {"$numberLong": "..."}}`, so the original BSON type can
be reconstructed exactly. Choose canonical for fixtures and round-trip tests,
relaxed for anything a human reads.

</details>

<details>
<summary>Why did my ObjectId turn into a plain string?</summary>

That is the point of unwrapping. An ObjectId has no plain-JSON equivalent, so
`{"$oid": "507f1f77bcf86cd799439011"}` becomes the 24-character hex string.
Converting back with *Detect ObjectIds and dates in strings* enabled restores the
`$oid` wrapper.

</details>

<details>
<summary>Will it guess types when converting plain JSON to Extended JSON?</summary>

Only if you ask it to. By default strings stay strings, because promoting them is
a guess that can be wrong — a 24-character hex string might be a checksum, not an
ObjectId. Turning on *Detect ObjectIds and dates in strings* promotes
24-character hex strings to `$oid` and full ISO-8601 date-times to `$date`.
Partial dates such as `2024-07-20` are never promoted.

</details>

<details>
<summary>Does this connect to my database?</summary>

No. It is a local text transformer. It does not open a connection, read a
connection string, or contact a cluster. Paste only the document you want to
convert.

</details>

<details>
<summary>Can it read a binary .bson file or a mongodump directory?</summary>

No — this tool works on Extended JSON *text*, the format `mongoexport` produces.
Raw BSON bytes are a separate, binary format and need a decoder rather than a
JSON parser.

</details>
