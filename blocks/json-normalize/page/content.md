## About this tool

JSON normalization turns a nested document into lookup tables keyed by id. Instead of keeping the same `user` object repeated inside every post and comment, the output stores one `entities.users["1"]` record and replaces each nested occurrence with the id `"1"`. This is the same shape many Redux, cache, and ETL pipelines expect: `{ "entities": { ... }, "result": ... }`.

Describe the relationships with a compact schema. JSON form uses entity names as keys, for example `{"articles":{"author":"users","comments":["comments"]},"comments":{"commenter":"users"},"users":{}}`. Shorthand form is easier to type: `articles: author -> users, comments -> [comments]`. Fields not named in the schema stay on their original entity.

Worked example: paste a post whose `author` is `{ "id": "1", "name": "Paul" }`, set root to `articles`, and use schema `articles: author -> users`. The post is stored in `entities.articles`, the author is stored in `entities.users`, and the article's `author` field becomes just `"1"`.

Limits and edge cases: the JSON document is capped at 5 MB, schemas are capped at 100 KB, nesting is capped at 100 levels, and at most 200,000 extracted entities are kept. The tool is schema-guided and deterministic; it does not run JavaScript callbacks, infer arbitrary polymorphic unions, or denormalize an entity store back into a tree.

## FAQ

<details>
<summary>Is this the same as flattening JSON into dotted keys?</summary>

No. Dotted-key flattening rewrites `{ "a": { "b": 1 } }` into something like `{ "a.b": 1 }`. Entity normalization extracts nested records into tables keyed by id and replaces nested objects with references, so repeated records are stored once.

</details>

<details>
<summary>How do I describe arrays of nested entities?</summary>

Use a one-element array in JSON schema form, such as `"comments": ["comments"]`, or brackets in shorthand form, such as `comments -> [comments]`. A single object in a list field becomes a one-element reference list so messy payloads still normalize.

</details>

<details>
<summary>What happens when an entity has no id?</summary>

The default is `error` so missing ids do not silently corrupt the store. You can choose `index` for run-local ids like `users-1`, `hash` for content-based ids, or `keep` to leave that nested object inline instead of extracting it.

</details>

<details>
<summary>Can I use custom id fields like `_id` or `id_str`?</summary>

Yes. Set `id_field` to a single field, a comma-separated fallback list such as `id,_id,uuid`, or a JSON map like `{ "*": "id", "tweets": "id_str" }` for per-entity rules.

</details>
