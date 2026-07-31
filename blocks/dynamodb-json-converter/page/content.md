## About this tool

DynamoDB exposes item data as typed `AttributeValue` JSON: strings are wrapped as
`{"S":"..."}`, numbers as `{"N":"30"}`, booleans as `{"BOOL":true}`, lists as
`{"L":[...]}`, maps as `{"M":{...}}`, and sets as `SS`, `NS`, or `BS`. That
shape is exact and useful for AWS APIs, but it is noisy when you just want to
read or edit the data. This converter moves between the AWS typed shape and
ordinary JSON.

Use **Plain JSON → DynamoDB JSON** when you are preparing an item for an AWS CLI,
SDK, or test fixture. Use **DynamoDB JSON → Plain JSON** when you copied an item
from a stream event, export, CLI response, or debugging log and want a readable
object. **Auto-detect** treats input that already looks like DynamoDB typed JSON
as typed input and converts it back to plain JSON; otherwise it marshals the
plain JSON into typed AttributeValues.

The conversion is deterministic and local. There is no AWS SDK call, network
request, credential lookup, or table access — it only reshapes the JSON you paste.

## Worked examples

Plain JSON input:

```json
{"id":"user#1","age":30,"active":true}
```

DynamoDB JSON output:

```json
{
  "id": { "S": "user#1" },
  "age": { "N": "30" },
  "active": { "BOOL": true }
}
```

DynamoDB JSON input:

```json
{"roles":{"SS":["admin","user"]},"score":{"N":"9.5"}}
```

Plain JSON output:

```json
{
  "roles": ["admin", "user"],
  "score": 9.5
}
```

## Limits and edge cases

- DynamoDB `N` values are stored as strings. This converter parses them into JSON
  numbers; very large or high-precision decimals may be rounded by JSON number
  handling in downstream tools.
- Binary values (`B` and `BS`) stay as base64 strings; the tool does not decode
  or re-encode binary data.
- Set types (`SS`, `NS`, `BS`) become plain arrays when unmarshalling. Plain JSON
  arrays marshal as DynamoDB lists (`L`) because JSON has no set type.
- A top-level plain object marshals as a DynamoDB item map. A top-level scalar or
  array marshals as a single bare AttributeValue.

## FAQ

<details>
<summary>What is DynamoDB typed JSON?</summary>

It is the `AttributeValue` representation used by DynamoDB APIs and many AWS
CLI examples. Every value carries a type tag, such as `S` for string, `N` for a
number string, `BOOL` for boolean, `NULL` for null, `M` for map, and `L` for list.

</details>

<details>
<summary>Can it handle DynamoDB sets?</summary>

Yes. `SS`, `NS`, and `BS` unmarshal to plain JSON arrays. When converting plain
JSON back to DynamoDB JSON, arrays become `L` lists, because plain JSON does not
say whether an array was meant to be a DynamoDB set.

</details>

<details>
<summary>Does this connect to my AWS account?</summary>

No. It is a local JSON reshaper. It does not call DynamoDB, read credentials, or
fetch table metadata. Paste only the JSON you want to convert.

</details>

<details>
<summary>Why did my big number change when converting from DynamoDB JSON?</summary>

DynamoDB stores numbers as strings, while plain JSON numbers are numeric values.
This tool parses valid `N` strings into JSON numbers. Extremely large integers or
high-precision decimals may be limited by JSON number handling in the tools that
read the output; keep the typed form if exact arbitrary precision matters.

</details>
