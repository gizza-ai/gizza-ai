## About this tool

This tool reads a GraphQL SDL schema and writes mock JSON that matches it. Paste the schema you already have — `type`, `interface`, `union`, `enum`, `input`, `scalar`, `schema` and `extend` definitions, with descriptions, comments, field arguments and directives — and pick what you want out: one mock object per type in the document, a single named type, or a full `{"data": ...}` response envelope for the `Query` root.

Everything runs in your browser. The schema is never uploaded, and output is **deterministic**: the same schema plus the same seed always produces byte-identical JSON, so you can paste the result straight into a test fixture or a snapshot without it churning on every run.

Nullable and non-null fields are treated differently on purpose. Non-null fields (`String!`) always get a value; nullable fields follow the **Nullable fields** control, so you can fill them, force every one of them to `null`, or leave them out of the object entirely — three shapes that exercise very different client code paths.

### Worked example

Paste this schema, set **What to generate** to *Query response envelope*, and leave the other controls at their defaults (2 items per list, depth 3, nullable fields filled, smart values on, seed 1):

```graphql
type Query {
  user: User
}

type User {
  id: ID!
  fullName: String!
  email: String!
  role: Role!
}

enum Role { ADMIN EDITOR VIEWER }
```

Output:

```json
{
  "data": {
    "user": {
      "email": "aria.davies@example.net",
      "fullName": "Mia Evans",
      "id": "e99ff867-dbf6-42c9-b82f-f84cb27281e9",
      "role": "ADMIN"
    }
  }
}
```

`ID!` became a v4 UUID, `Role!` was drawn from the enum's own values rather than invented, and `email`/`fullName` were shaped from the field names because **Infer realistic values from field names** is on. Change the seed to get a different-but-equally-stable fixture.

### Field-name inference and faker directives

With smart values on, field names steer the generator: `email`, `phone`, `avatarUrl`, `slug`, `username`, `firstName`, `city`, `country`, `postcode`, `currency`, `locale`, `timezone`, `color`, `ip`, `token`, `status`, `createdAt`, `birthday`, `description`, `title` and `id` all get appropriately shaped values, and numeric names such as `age`, `year`, `percent`, `count`, `price`, `rating`, `latitude` and `longitude` get plausible ranges. Turn the option off for neutral placeholder text when you want the shape without the flavour.

Per-field directives override both, using the vocabulary SDL mock servers already use:

- `@examples(values: ["Mx", "Dr"])` — pick from an explicit list (strings, numbers, booleans and enum names all work).
- `@fake(type: firstName)` — name a generator directly. Unknown generator names fall back to field-name inference instead of failing the document.
- `@listLength(min: 4, max: 4)` — override the global items-per-list for that one field.

Well-known custom scalars are shaped by name too, with no configuration: `DateTime`, `Date`, `Time`, `UUID`, `URL`, `Email`, `JSON`, `BigInt`, `Decimal`, `PhoneNumber`, `HexColor`, `Base64` and `Upload` among them. Any other `scalar Foo` becomes the clearly-labelled string `mock-foo`, so an unhandled scalar is visible in the output rather than silently plausible.

### Limits and edge cases

- Maximum schema size is 200,000 bytes; up to 10 items per list field; nesting depth 1–6.
- Recursive schemas (`type User { friends: [User!]! }`) terminate at the depth cap, where a nested object collapses to `{}` rather than recursing forever.
- An **interface** resolves to the first object type in the document that implements it, and a **union** resolves to its first listed member. Both always carry `__typename` so the concrete shape is unambiguous, even when the `__typename` option is off.
- `input` types, `enum` declarations and `scalar` declarations are skipped in *one mock per type* mode, because they are not response data. Use *single-type* mode to mock one of them directly.
- A field referencing an undefined type is an error, not a guess — declare it with `scalar Foo` if it comes from elsewhere.
- Executable documents are rejected. `query { me { id } }` is an operation, not a schema; paste the type definitions instead.
- Values are synthetic by construction — names come from a fixed word pool and every domain is an `example.com`-family reserved domain. Nothing here is real personal data, and `password` fields render as `not-a-real-password`.

## FAQ

<details>
<summary>Why does the same schema keep giving me the same JSON?</summary>

Because that is the point. Output is seeded, so a schema plus a seed is a stable fixture you can commit and diff. Change the **Seed** value to get a different dataset, or bump it in a loop to generate a set of distinct-but-reproducible records.

</details>

<details>
<summary>How do I mock the response to one specific query?</summary>

Choose *Query response envelope* to get `{"data": {...}}` covering every field on the `Query` root, then delete the fields your query does not select. This tool mocks the **schema**, not a query document — it has no selection-set executor, so it cannot trim the response for you.

</details>

<details>
<summary>My schema has `scalar DateTime` — do I need to configure it?</summary>

No. Common scalar names are recognised case-insensitively and shaped accordingly, so `DateTime` becomes an ISO-8601 timestamp and `URL` becomes a URL with no setup. An unrecognised scalar becomes `mock-<name>` so you can see at a glance that it needs attention.

</details>

<details>
<summary>What is the difference between the three nullable-field modes?</summary>

*Fill* generates a value for every nullable field, which is the friendliest default for a UI fixture. *Force to null* sets each one to JSON `null`, which is the fastest way to check that your client handles missing optional data. *Leave them out* omits the keys entirely, which is what a real server does for fields a query did not select. Non-null fields are always generated in all three modes.

</details>

<details>
<summary>Will it handle interfaces and unions?</summary>

Yes. An interface is mocked as the first object type that implements it, and a union as its first listed member, with `__typename` always included so the chosen shape is explicit. If you need a different member, put that type first in the document or mock it directly with *single-type* mode.

</details>

<details>
<summary>Is my schema uploaded anywhere?</summary>

No. The generator is compiled to WebAssembly and runs entirely in your browser tab, so the schema text never leaves your machine. The same code is available offline through the command line tool.

</details>
