## About this tool

Search Index Builder turns a JSON array of documents into a serialized **inverted
index** — a compact JSON file that a static site can load to run full-text search in
the browser, with no server and no external service. It is the build step behind
"search-as-you-type" on documentation sites, blogs, and offline apps: index once at
build time, ship the JSON, and let a tiny client-side ranker answer queries.

Everything runs locally in your browser through WebAssembly, and the output is
fully deterministic — the same input always produces byte-for-byte the same index.

### What it produces

Give it an array like `[{"id":"1","title":"Intro","body":"Hello world"}]` and it
returns a single JSON object:

- **`index`** — the inverted index: `token → field → { df, postings }`, where `df`
  is the document frequency (how many documents contain the token in that field) and
  `postings` maps each document's ref to its term frequency (`tf`). Together `df` and
  `tf` are everything a client needs to rank results with TF-IDF.
- **`documents`** — an optional document store: for each ref, the display fields you
  chose to keep (title, url, …) so results can be rendered without a second lookup.
- **`documentCount`**, **`tokenCount`**, **`fields`** (the fields that were indexed,
  sorted), **`ref`** (the field used as each document's identifier), and **`boosts`**
  (per-field ranking weights, present only when you supply them).

### How tokenizing works

Each indexed field is split on non-alphanumeric boundaries into tokens. You control
the rest:

- **Lowercase** folds tokens to lower case for case-insensitive search (on by
  default). Turn it off to keep `API` and `api` distinct.
- **Remove stop words** drops a short list of common English words (the, and, of,
  to, …) so they don't bloat the index.
- **Minimum token length** (1–20) drops tokens shorter than the given length —
  set it to 2 or 3 to skip single letters and short noise.

The **id field** (default `id`) supplies each document's ref; when a document has no
such field, its 0-based position in the array is used instead. Refs must be unique.
Leave **fields** blank to index every string-valued field automatically, or list the
ones you want. **Boosts** (`title:3,body:1`) are recorded in the output for your
query-time ranker to weight matches per field.

### Worked example

Input (index everything, store `title` and `url`):

```
[{"id":"home","title":"Home","url":"/","body":"Welcome to the search demo"},
 {"id":"about","title":"About us","url":"/about","body":"We build fast offline search"}]
```

The token `search` appears in the body of both documents, so its entry records
`df: 2` under `body` with `postings: {"home": 1, "about": 1}`, while `about`'s
`title` gets its own postings. The `documents` store carries each page's `title` and
`url` so a result list can link straight to the page.

## FAQ

<details>
<summary>What is an inverted index and why do I need one for search?</summary>

An inverted index flips the usual "document → words" mapping around into "word →
documents", so that when someone searches for a term you can jump straight to the
list of documents containing it instead of scanning every document. It is the data
structure behind essentially every search engine. Building it ahead of time lets a
static site answer queries instantly in the browser — the client just looks up the
query terms in the index and ranks the matching documents.

</details>

<details>
<summary>How does a client use the df and tf numbers to rank results?</summary>

The postings carry a per-document **term frequency** (`tf`) — how often the term
appears in that document's field — and each token/field entry carries a **document
frequency** (`df`) — how many documents contain it. A common ranking formula,
TF-IDF, rewards documents where a term is frequent (high `tf`) but rare across the
corpus (low `df`), so distinctive words matter more than ubiquitous ones. The
optional per-field **boosts** let a client weight a match in `title` more heavily
than one in `body`. This tool ships the raw counts; the ranking formula lives in
your query-time code.

</details>

<details>
<summary>My documents are files in a folder, not a JSON array — how do I use this?</summary>

This tool takes the documents as a JSON array because it runs as a pure, in-browser
transform with no file-system access. In a real build pipeline, a small script reads
your folder of Markdown/HTML files, extracts the fields you care about (title, url,
body text), and assembles that array — then hands it to this builder to produce the
index JSON. Here you can paste the array directly to prototype the index shape,
tune the tokenizing options, and confirm the output before wiring it into a build.

</details>

<details>
<summary>Does it do stemming, fuzzy matching, or run the search queries?</summary>

No. It is deliberately a build-time **index generator**, not a query-time search
library. It tokenizes and counts; it does not stem words (running/ran/runs stay
distinct), do fuzzy/typo matching, expand prefixes, or execute queries. Those belong
to the client-side ranker that consumes the index. Keeping the builder to a pure,
deterministic documents-in / index-out transform makes the output stable and easy to
test, and lets you pair it with whichever ranking approach you like.

</details>

<details>
<summary>What are the limits and edge cases?</summary>

The `documents` input must be a JSON **array of objects** with at least one element;
an empty array, a bare object, or invalid JSON returns an error. Every document ref
must be unique — a duplicate `id` (or two documents both missing an id at the same
resolved position) is rejected. Only **string** fields are tokenized; numbers,
booleans, and nested objects are ignored for indexing (though any field can be copied
into the store). Minimum token length is clamped to 1–20. Everything is held in
memory in the browser, so very large corpora are best indexed in a real build step
rather than pasted here.

</details>
