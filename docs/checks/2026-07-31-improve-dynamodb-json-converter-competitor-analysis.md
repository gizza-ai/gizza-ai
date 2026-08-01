# dynamodb-json-converter — competitor analysis (2026-07-31)

Tool function: convert between DynamoDB typed AttributeValue JSON and ordinary JSON in both directions.

Distinct from existing tools:
- `blocks/json-to-dynamodb-batch` prepares DynamoDB batch-write request shapes; it is not a bidirectional AttributeValue JSON marshaller/unmarshaller.
- Generic JSON converters (`json-yaml-convert`, `json-to-json-schema`, etc.) do not understand DynamoDB type tags.
- This tool is viable as a pure deterministic JSON reshaper with no AWS SDK or network access.

## Scan (top competitors, paraphrased — no copy/branding reproduced)

1. **Dynobase DynamoDB JSON converter**. Converts plain JSON/JS-object-like input to DynamoDB-compatible marshalled JSON and back. Table-stakes: bidirectional marshal/unmarshal, simple paste area, readable examples, and clear explanation that DynamoDB's format is not ordinary JSON.
2. **AWS SDK JavaScript DynamoDB.Converter docs**. Exposes `marshall`, `unmarshall`, `input`, and `output`, plus options around numbers. Table-stakes: support the official AttributeValue tags, preserve number strings on the DynamoDB side, and document precision caveats when returning plain-language numeric values.
3. **dangerfarms unmarshall DynamoDB JSON**. Small web utility focused on the DynamoDB JSON to regular JSON direction with examples for strings, lists, maps, numbers, nulls, and booleans. Table-stakes: handle nested `M`/`L`, `BOOL`, `NULL`, and visible before/after examples.
4. **AWS SDK Go v2 `attributevalue` package**. Official marshal/unmarshal helpers for application objects and AttributeValue maps. Table-stakes: same tag model, nested values, set types, and clear distinction between DynamoDB item maps and single AttributeValues.

## Table-stakes → decision

| Table-stake | In/out model | Decision |
|---|---:|---|
| Plain JSON → DynamoDB typed JSON | in | `direction = to-dynamodb` |
| DynamoDB typed JSON → plain JSON | in | `direction = from-dynamodb` |
| Auto-detect direction | in | `direction = auto` |
| String, number, bool, null tags | in | `S`, `N`, `BOOL`, `NULL` |
| Nested maps and lists | in | `M`, `L` recursion |
| Binary values | in | `B` and `BS` pass base64 strings through |
| DynamoDB string/number/binary sets | in | `SS`, `NS`, `BS` unmarshal to arrays |
| Pretty vs compact output | in | `pretty` boolean |
| Top-level item map vs bare AttributeValue handling | in | item maps for object roots, bare AttributeValue for scalar/list roots |
| Number precision warnings | in | page limits note |
| SDK object/class marshalling | out | no language runtime objects in a browser-local JSON tool |
| AWS account/table integration | out | server/account feature; this repo tool has no network or credentials |
| Expression builder / batch-write generator | out | covered by adjacent DynamoDB tools or separate backlog ideas |

## Descriptor / UX decisions

- The page exposes one multiline JSON field, a direction select, and a pretty-print checkbox.
- Preset chips cover plain-to-DynamoDB, DynamoDB-to-plain, and auto-detect typed input.
- Page copy explicitly calls out that set types become arrays and arrays marshal as lists because plain JSON has no set type.
