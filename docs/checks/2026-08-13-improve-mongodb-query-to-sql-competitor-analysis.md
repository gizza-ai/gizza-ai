# mongodb-query-to-sql — competitor analysis (2026-08-13)

Scan run **before** implementation, per `/improve-tool` Phase 2–3. One web search
("MongoDB query to SQL converter online tool") plus direct reads of the reachable tools.
Everything below is **paraphrased**; no competitor copy, branding, or trademarks are reproduced.

## Landscape note

Most results for this query point the *other* way (SQL → MongoDB). Only a handful of real,
usable MongoDB-filter → SQL translators exist, which is the gap this tool fills. Of the five
candidates picked, one (querymongo.com) no longer resolves (DNS failure) and one (site24x7)
returned a rate-limit page and converts SQL → MongoDB only, so it was read for UX signal only.

## Competitor profiles

### 1. gordonBusyman/mongo-to-sql-converter (npm/GitHub library)

```json
{
  "name": "mongo-to-sql-converter",
  "url": "https://github.com/gordonBusyman/mongo-to-sql-converter",
  "features": ["string in / string out", "db.<coll>.find(...) call syntax accepted",
               "flag to rewrite _id as id"],
  "params_options": [{"name": "removeUnderscoreBeforeID", "type": "boolean", "default": "false",
                      "range": "rewrites _id to id in the output"}],
  "operators": ["$or", "$and", "$lt", "$lte", "$gt", "$gte", "$ne", "$in"],
  "input_formats": ["db.user.find({age: {$gte: 21}, name: 'julio'})"],
  "output_formats": ["SQL WHERE-clause text"],
  "output_quality": "Narrow but predictable; throws on any method other than find.",
  "ux_patterns": ["library only — no UI"],
  "limits": ["no $regex, $exists, $nin, $not, $nor, $mod, $size",
             "no projection / sort / limit / skip / count handling",
             "no full SELECT emission", "no dialect awareness"],
  "free_vs_paid": "free, MIT-style library"
}
```

### 2. CodeConvert AI — MongoDB to SQL

```json
{
  "name": "CodeConvert AI (MongoDB → SQL)",
  "url": "https://www.codeconvert.ai/mongodb-to-sql-converter",
  "features": ["LLM-backed translation of arbitrary snippets", "type / paste / upload a file",
               "free-text extra-instructions box", "clear button", "copy + download result",
               "cross-links to other target languages (PostgreSQL, MySQL, CQL, Redis, Elasticsearch)"],
  "params_options": [{"name": "additional instructions", "type": "free text", "default": "empty"},
                     {"name": "target language", "type": "select", "default": "SQL"}],
  "input_formats": ["pasted snippet", "uploaded file"],
  "output_formats": ["SQL text in a read-only editor pane"],
  "output_quality": "Flexible (handles anything an LLM can guess at) but non-deterministic and unverifiable.",
  "ux_patterns": ["two-pane editor", "language icons", "copy/download", "clear"],
  "limits": ["2 free conversions per day when signed out",
             "signed-in tier caps input at ~25k characters and spends credits",
             "history/notes/chat are paid"],
  "free_vs_paid": "hard daily cap free; most capability behind an account + paid tiers"
}
```

### 3. AI2SQL — MongoDB to SQL learn/tool page

```json
{
  "name": "AI2SQL MongoDB → SQL",
  "url": "https://ai2sql.io/learn/mongodb-to-sql-converter",
  "features": ["natural-language or Mongo-syntax input", "documents find → SELECT/FROM/WHERE",
               "documents projection → SELECT list", "documents $group/$sum → GROUP BY/SUM"],
  "params_options": [{"name": "target dialect", "type": "select", "default": "generic SQL"}],
  "input_formats": ["db.customers.find({ country: 'USA' })", "prose"],
  "output_formats": ["SELECT ... FROM ... WHERE ..."],
  "output_quality": "Documents only $gt and $sum concretely; the rest is asserted, not specified.",
  "ux_patterns": ["worked input→output pairs in the copy", "dialect picker", "signup CTA"],
  "limits": ["no documented handling of $eq/$in/$or/$not/$regex/$exists/$type",
             "no documented sort / limit / skip / dotted-path behavior"],
  "free_vs_paid": "account required for real use"
}
```

### 4. Site24x7 SQL ↔ MongoDB converter (read for UX only — opposite direction)

```json
{
  "name": "Site24x7 SQL → MongoDB",
  "url": "https://www.site24x7.com/tools/sql-to-mongodb.html",
  "features": ["single input box", "Convert + Clear buttons", "load query from a local file",
               "download the converted result"],
  "params_options": [],
  "input_formats": ["MySQL / Oracle / PostgreSQL / SQL Server query text"],
  "output_formats": ["MongoDB query text"],
  "output_quality": "not assessable — the page served a rate-limit notice during the scan",
  "ux_patterns": ["convert/clear pair", "file load", "result download"],
  "limits": ["server-side, rate limited (\"too many requests\")"],
  "free_vs_paid": "free, but throttled"
}
```

### 5. querymongo.com — UNREACHABLE

DNS no longer resolves (`getaddrinfo ENOTFOUND`). Recorded as dead rather than replaced with a
fifth listicle; four real data points were enough to fix the table stakes.

## Table stakes extracted (all landed in this build)

| Table stake | Source | Where it landed |
| --- | --- | --- |
| Accept `db.<coll>.find({...})` call syntax, not just a bare filter | 1, 3 | call-chain parser; bare filter objects also accepted |
| Accept shell syntax, not strict JSON (unquoted keys, single quotes, trailing commas, comments) | 1, 3 | relaxed parser |
| `$and $or $lt $lte $gt $gte $ne $in` | 1 | supported |
| `$eq $nin $not $nor $exists $regex $mod $size` | gap in all four | supported (beyond every competitor) |
| Projection → SELECT list | 2, 3 | `output = select` |
| `_id` → `id` rewrite switch | 1 | `rename_id` checkbox (default off) |
| Dialect choice | 3 | `dialect` = ansi/postgres/mysql/sqlserver |
| Copy / download the result | 2, 4 | shared page chrome (copy button, text download) |
| Worked input→output pairs in the copy | 3 | content.md worked example + 5 preset chips |
| Clear / reset | 2, 4 | shared page Reset button |

## Decisions

**In-model, built**
- Full operator coverage above, including the ones no competitor documents.
- `.sort()/.limit()/.skip()/.count()/.countDocuments()/findOne()` chain → `ORDER BY`/`LIMIT`/
  `OFFSET`/`COUNT(*)`, with SQL Server's `TOP (n)` and `OFFSET … ROWS FETCH NEXT … ROWS ONLY`.
- MongoDB Extended JSON (`$oid`, `$date`, `$numberInt/Long/Double/Decimal`, `$regularExpression`)
  and shell helpers (`ObjectId()`, `ISODate()`, `new Date()`, `NumberLong()`, `/re/i`).
- Dotted paths as either one column name or a real JSON extraction per dialect
  (`->>` / `JSON_UNQUOTE(JSON_EXTRACT(…))` / `JSON_VALUE(…)`), with Postgres casts so numeric and
  boolean comparisons stay type-correct.
- Identifier quoting per dialect, switchable off.
- Deterministic, offline, no cap on conversions and no account — the direct answer to
  competitor 2's 2-per-day gate and competitor 4's throttling.

**Considered, rejected (in-model but declined)**
- Parameterised output (`?` placeholders + a bound-value list). Real value, but it doubles every
  output mode and the WHERE clause stops being paste-able, which is the tool's main job.
- `$all` / `$elemMatch` / `$type` rewrites. Each needs a schema assumption (is the column a JSON
  array? a relational child table?) that the tool cannot know; a wrong-but-plausible translation is
  worse than an error that names the operator.

**Out-of-model (needs a server, model, or account — not built)**
- LLM translation of arbitrary snippets and prose (competitors 2 and 3): needs a hosted model.
- Aggregation pipelines (`$group`, `$lookup`, `$unwind`): expressible in SQL only with schema
  knowledge; deliberately rejected with a message that says so rather than guessed at.
- File upload of multi-megabyte scripts, saved conversion history, accounts, credits.
