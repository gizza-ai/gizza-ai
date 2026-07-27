## About this tool

DynamoDB's `BatchWriteItem` API does not accept plain JSON. Every value has to be
wrapped in a typed AttributeValue — strings become `{ "S": ... }`, numbers become
`{ "N": "..." }`, booleans become `{ "BOOL": ... }`, and so on. Hand-writing that
wrapper for more than a couple of rows is tedious and error-prone.

This tool takes a normal JSON array of objects and produces the full
`BatchWriteItem` request payload for you. Paste your rows, set the target table,
choose **put** or **delete**, and copy the result into the AWS CLI
(`aws dynamodb batch-write-item --request-items file://payload.json`), an SDK
call, or a seeding script. It runs entirely in your browser — nothing is uploaded
and no AWS credentials are involved.

### What it converts

- `string` → `{ "S": "..." }`
- `number` → `{ "N": "..." }` (the exact JSON spelling is preserved, including
  large integers and decimals)
- `true` / `false` → `{ "BOOL": true | false }`
- `null` → `{ "NULL": true }`
- array → `{ "L": [ ... ] }` (each element is converted recursively)
- object → `{ "M": { ... } }` (each field is converted recursively)

For **put**, each object is wrapped as `{ "PutRequest": { "Item": ... } }`. For
**delete**, each object is wrapped as `{ "DeleteRequest": { "Key": ... } }`, so
you should include only the key attributes.

## Worked example

Input array:

```json
[
  { "id": "user#1", "name": "Ada", "age": 36, "active": true },
  { "id": "user#2", "name": "Grace", "age": 45, "active": false }
]
```

With table name `Users` and operation `put`, the tool returns:

```json
{
  "RequestItems": {
    "Users": [
      {
        "PutRequest": {
          "Item": {
            "id": { "S": "user#1" },
            "name": { "S": "Ada" },
            "age": { "N": "36" },
            "active": { "BOOL": true }
          }
        }
      },
      {
        "PutRequest": {
          "Item": {
            "id": { "S": "user#2" },
            "name": { "S": "Grace" },
            "age": { "N": "45" },
            "active": { "BOOL": false }
          }
        }
      }
    ]
  }
}
```

## Limits & edge cases

- **25 items maximum.** DynamoDB accepts at most 25 write requests per
  `BatchWriteItem` call, so the tool errors above 25 items instead of emitting a
  payload AWS would reject. Split larger data sets into batches of 25.
- **Input must be a JSON array of objects.** A bare object, a scalar, or an array
  containing a non-object element is rejected with a clear error.
- **The table name must not be empty.**
- **Numbers keep their exact text.** Because DynamoDB stores `N` as a string, the
  original digits are preserved — a value like `12345678901234567890` is not
  rounded to a float.
- **No key inference.** For deletes, the tool trusts the object you provide as the
  key; it does not know your table's key schema, so include exactly the partition
  (and, if present, sort) key attributes.
- **Empty values.** DynamoDB rejects empty strings in some key positions and empty
  binary sets; this tool passes values through as written and does not validate
  against your table's constraints.

## FAQ

<details>
<summary>How do I use the output with the AWS CLI?</summary>

Save the result to a file, for example `payload.json`, then run
`aws dynamodb batch-write-item --request-items file://payload.json`. The tool's
output is already shaped as the `--request-items` value, with your table name as
the top-level key.

</details>

<details>
<summary>Why is my number wrapped as a string like `{ "N": "36" }`?</summary>

That is exactly how DynamoDB represents numbers. The `N` AttributeValue type is
always a string in the wire format to avoid precision loss, and the SDKs convert
it back to a numeric type on read. This tool preserves the original JSON spelling
so large integers and decimals survive the round trip.

</details>

<details>
<summary>What should I put in each object for a delete operation?</summary>

Only the key attributes. A `DeleteRequest` is matched by the item's primary key,
so include the partition key (and the sort key if your table has one) and nothing
else. Extra attributes are still converted, but DynamoDB ignores everything beyond
the key when deleting.

</details>

<details>
<summary>Can I mix puts and deletes in one payload?</summary>

Not in a single run — the `operation` setting applies to the whole array. A real
`BatchWriteItem` call can mix `PutRequest` and `DeleteRequest` entries, so if you
need both, generate a put payload and a delete payload separately and merge the
two request arrays for the same table by hand.

</details>

<details>
<summary>How do I handle more than 25 rows?</summary>

Split your data into chunks of 25 or fewer and run the tool once per chunk, then
send each payload as its own `BatchWriteItem` call. DynamoDB enforces the 25-item
limit per request, so batching is required regardless of which tool builds the
payload.

</details>
