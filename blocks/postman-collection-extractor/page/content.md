## About this tool

**Postman Collection Extractor** turns a **Collection v2.0/v2.1 export** —
the JSON file Postman writes for *Collection → Export → Collection v2.1* —
into a flat inventory of the requests inside it. Instead of clicking through
folders one request at a time, you get every endpoint at once:

- **Folder path** — nested folders joined with ` / ` (`Admin / Users`); empty
  for a request that sits at the collection root.
- **Name and method** — the request's name and its HTTP verb, upper-cased.
- **URL** — the raw URL as Postman displays it, or rebuilt from the v2.1
  `protocol` / `host` / `path` / `query` parts when there is no `raw` field.
  Disabled query params are dropped, because Postman would not send them.
- **Headers** — every enabled header as `Name: value`; headers you unticked in
  Postman are skipped.
- **Body** — the raw/JSON body, or a flattened form: `urlencoded` fields as
  `a=1&b=2`, `form-data` fields one per line with file parts as `field=@path`,
  and GraphQL as the query plus its `variables:` block.
- **Auth** — the auth *type* only (`bearer`, `basic`, `apikey`, `oauth2`, …).
  A request with no auth block of its own reports the folder's or
  collection's type, which is what Postman actually sends.

The **#** column is each request's position in the *original* collection
order, so a row stays cross-referenceable after you filter.

### Filters

- **Method filter:** case-insensitive exact match (`GET`, `POST`, `DELETE`, …).
- **URL contains:** case-insensitive substring — `/v1/`, `users`, a hostname.
- **Folder contains:** case-insensitive substring of the folder path, so
  `users` matches `Admin / Users`.

### Output formats

- **List** — one block per request with folder, auth, every header, and the
  full body. The most detailed view.
- **Table** — aligned columns (`#`, `METHOD`, `HDRS`, `BODY`, `NAME`, `URL`)
  plus a summary line, for a quick overview of a big collection.
- **JSON** — an array of request objects (`index`, `folder`, `name`,
  `method`, `url`, `auth`, `headers`, `body_mode`, `body`), ready for a script.
- **CSV** — the same fields as spreadsheet rows, RFC 4180-quoted so a
  multi-line JSON body stays in one cell.
- **Markdown** — a GFM table to paste into a README or wiki; pipes are
  escaped and newlines become `<br>`.
- **URLs** — one URL per line, duplicates removed, first occurrence wins.

### {{variables}}

The collection's own `variable` array is applied by default, so
`{{baseUrl}}/users` lists as a real URL. Add your own in the **Variables**
box to override them — `KEY=VALUE` lines, a JSON object, or a whole pasted
Postman **environment export** (its `values` array is read, and entries with
`"enabled": false` are ignored). Point a collection at staging without
touching the export:

```text
baseUrl=https://staging.example.com
```

Untick **Resolve {{variables}}** to see the placeholders exactly as exported.
Either way, a placeholder with no value is left verbatim — nothing is
silently blanked out.

### Worked example

A three-request collection with one nested folder, collection-level bearer
auth, a `{{baseUrl}}` variable, and one disabled header:

```json
{"info":{"name":"Demo API"},
 "variable":[{"key":"baseUrl","value":"https://api.example.com"}],
 "auth":{"type":"bearer","bearer":[{"key":"token","value":"REDACTED"}]},
 "item":[
   {"name":"List users","request":{"method":"GET","url":{"raw":"{{baseUrl}}/users?page=1"}}},
   {"name":"Users","item":[
     {"name":"Create user","request":{"method":"POST","url":"{{baseUrl}}/users",
      "header":[{"key":"Content-Type","value":"application/json"},
                {"key":"X-Debug","value":"1","disabled":true}],
      "body":{"mode":"raw","raw":"{\"name\":\"Ada\"}","options":{"raw":{"language":"json"}}}}},
     {"name":"Delete user","request":{"method":"DELETE","url":"{{baseUrl}}/users/1"}}]}]}
```

extracts to this table:

```text
3 of 3 requests · 1 folder

#  METHOD  HDRS  BODY  NAME         URL
1  GET     0     none  List users   https://api.example.com/users?page=1
2  POST    1     json  Create user  https://api.example.com/users
3  DELETE  0     none  Delete user  https://api.example.com/users/1
```

The `HDRS` count is `1`, not `2`, because the disabled `X-Debug` header is
skipped. The same collection as **CSV**:

```text
index,folder,name,method,url,auth,headers,body_mode,body
1,,List users,GET,https://api.example.com/users?page=1,bearer,,none,
2,Users,Create user,POST,https://api.example.com/users,bearer,Content-Type: application/json,json,"{""name"":""Ada""}"
3,Users,Delete user,DELETE,https://api.example.com/users/1,bearer,,none,
```

Note that requests 2 and 3 inherit `bearer` from the collection even though
neither declares an auth block, and that only the *type* is reported — the
token never appears in the output.

### Limits and edge cases

- **Collection v2.0/v2.1 only.** A v1 export (or a raw Postman dump without a
  top-level `item` array) is rejected with a message saying so; convert it to
  v2 in Postman first.
- Up to **500 requests** per run; each body is truncated at **2 000
  characters** with a visible `… (truncated at 2000 characters)` marker.
- Extraction is deliberately **forgiving per request**: a request with no
  `method` lists as `GET`, one with no URL lists with an empty URL, and an
  unnamed one shows as `(unnamed)`. Only empty input, non-JSON input, a
  non-collection object, an empty collection, an unknown format, or filters
  that match nothing are hard errors.
- **Not extracted:** pre-request scripts, tests, saved example responses,
  cookies, `protocolProfileBehavior`, and secrets of any kind. Auth is a type
  label only — tokens, passwords, and API keys are never printed.
- Everything runs in your browser's WebAssembly. The collection is not
  uploaded, and no request is ever sent to the API it describes.

### Handy for

- Auditing what a team's collection actually calls before an API change.
- Producing a Markdown endpoint table for a README, or a CSV for a
  spreadsheet review.
- Pulling every URL out of a collection to feed a script, a smoke test, or a
  firewall allow-list.

## FAQ

<details>
<summary>How do I get the collection JSON out of Postman?</summary>

In Postman, click the **…** menu next to the collection name and choose
**Export**, then **Collection v2.1 (recommended)**. Postman saves a `.json`
file — open it in a text editor and paste the whole thing into the box above,
or run `gizza tool postman-collection-extractor --collection "$(cat
My-API.postman_collection.json)"` from a terminal. Collection **v2.0** exports
work too; a legacy **v1** export has to be converted to v2 first.

</details>

<details>
<summary>Is it safe to paste a collection with tokens in it?</summary>

The collection never leaves your machine — extraction runs entirely in your
browser via WebAssembly, with nothing uploaded and no request sent to the API
being described. The output is also narrower than the input: auth blocks are
reduced to a **type** label (`bearer`, `basic`, `apikey`, …), so tokens,
passwords, and API keys are never printed. Headers and URLs *are* printed
verbatim, so if you keep an API key in a header or a query string, review the
result before sharing it.

</details>

<details>
<summary>Why is my URL still full of {{curly braces}}?</summary>

A placeholder is only replaced when a value for it exists. Values come from
the collection's own `variable` array plus whatever you put in the
**Variables** box; Postman keeps *environment* and *global* variables in
separate files, so a collection that relies on an environment exports with
its placeholders unresolved. Paste that environment export (or a
`baseUrl=https://api.example.com` line) into the Variables box and the URLs
fill in. Unknown placeholders are always left verbatim rather than blanked,
so you can see exactly what is missing.

</details>

<details>
<summary>Does it handle nested folders and inherited auth?</summary>

Yes. Folders are walked recursively to any depth and the path is reported as
a column (`Admin / Users / Bulk`), so nothing is hidden inside a subfolder.
Auth resolves the way Postman does it: a request's own auth block wins,
otherwise the nearest enclosing folder's, otherwise the collection's. Use the
**Folder contains** filter to narrow the listing to one subtree.

</details>

<details>
<summary>Can it give me runnable curl commands instead?</summary>

That is a different tool — this one produces an *inventory* (what endpoints
exist, with their headers and bodies), not code. For copy-paste-ready
requests, use the **Postman Collection Converter**, which turns the same
export into `curl`, JavaScript `fetch()`, or `axios` calls. For the same kind
of listing from a browser DevTools capture instead of a collection, use the
**HAR Request Extractor**.

</details>

<details>
<summary>What is <em>not</em> included in the output?</summary>

Pre-request scripts, test scripts, saved example responses, cookies, and
`protocolProfileBehavior` settings are all skipped — they are not part of a
request inventory. Disabled headers, disabled query parameters, and disabled
form fields are also skipped, because Postman would not send them. Bodies
longer than 2 000 characters are cut off with a visible truncation marker,
and a collection with more than 500 requests is rejected rather than silently
shortened.

</details>
