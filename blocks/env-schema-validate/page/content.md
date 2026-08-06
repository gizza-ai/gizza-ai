## About this tool

A missing or mistyped environment variable rarely fails where you can see it. The app boots,
connects to nothing, and dies later with `undefined is not a function` or a timeout against
`localhost`. This tool moves that failure to the front: you declare what the environment must
contain — which keys are **required**, what **type** each value has, and which **allowed values**
are legal — and it checks a `.env` file against that declaration, listing every problem with its
line number, a severity and the rule that caught it.

Everything runs locally in your browser via WebAssembly. The `.env` you paste is never uploaded,
and values of secret-looking keys (names containing `SECRET`, `TOKEN`, `PASSWORD`, `KEY`, `AUTH`)
are masked in the report so a result is safe to paste into an issue or a CI log.

### Worked example

Schema:

```
NODE_ENV=required|enum:development,staging,production
PORT=required|port
DATABASE_URL=required|url
API_KEY=required|min:32
TIMEOUT=number|min:1|max:60|default:30
```

`.env`:

```
# staging
export NODE_ENV=staging
PORT=99999
DATABASE_URL=localhost
API_KEY=short
LEGACY_FLAG=1
```

Result:

```
.env schema check: FAILED — 3 errors, 1 warning (5 declared keys, 5 keys in the file)
  line 3    error    type                    'PORT' must be a TCP port between 1 and 65535, got '99999'
  line 4    error    type                    'DATABASE_URL' must be a URL with a scheme and host (e.g. https://api.example.com), got 'localhost'
  line 5    error    min                     'API_KEY' must be at least 32 characters long, got 5
  line 6    warning  unknown-key             'LEGACY_FLAG' is set in the .env file but is not declared in the schema
```

`NODE_ENV=staging` passes because `staging` is in the allowed list, and `TIMEOUT` is absent without
complaint because it is optional and documents a default.

### The rules dialect

One `KEY=rule|rule:arg` line per variable. A bare `KEY=` line simply means *required*, so an
existing `.env.example` works as a schema as-is.

| Rule | Meaning |
| --- | --- |
| `required` / `optional` | The key must be set / may be absent (optional is the default) |
| `string` `number` `integer` `boolean` | Basic value types (`boolean` accepts true/false/1/0/yes/no/on/off) |
| `port` `url` `email` `host` `json` | A TCP port 1–65535, a URL with scheme and host, an email address, a hostname or IP, parseable JSON |
| `enum:a,b,c` | The value must be exactly one of the listed allowed values |
| `min:N` / `max:N` | A value bound for numeric types, a character-length bound for everything else |
| `pattern:REGEX` | The value must match the regular expression |
| `secure` | 8+ characters with lower case, upper case, a digit and a symbol |
| `default:VALUE` | Documentation only — a missing key that documents a default is a warning, not an error |

Prefer JSON Schema? Paste one instead: `type`, `required`, `properties` with `enum`,
`minimum`/`maximum`, `minLength`/`maxLength`, `pattern`, `format` (`uri`, `email`, `hostname`) and
`default` are all honoured, and the **auto** format setting detects it by the leading `{`.

### Limits and edge cases

- Only the `.env` **text** you paste is examined — the tool never reads `process.env`, the shell,
  or any file on disk, and it never rewrites your file.
- Duplicate keys follow dotenv semantics: the **last** definition wins and is the one validated;
  the earlier one is reported as a warning.
- `${VAR}` interpolation is not expanded — a value like `http://${HOST}:${PORT}` is checked as the
  literal text it is, so a `url` rule on an interpolated value will flag it.
- Comments (`#`), blank lines, `export ` prefixes, single/double quotes and trailing ` # comments`
  on unquoted values are all understood.
- `min`/`max` on a string counts **characters**, not bytes.
- There is no practical size cap; a schema with thousands of keys still validates instantly, since
  everything runs as local WebAssembly.

<details>
<summary>How is this different from a plain .env linter?</summary>

A linter only knows what is *in* the file: duplicate keys, stray spaces, broken quotes. It cannot
know that `PORT` must exist and be a port number, because nothing declares that. This tool takes a
schema as a second input, so it catches the errors that matter at boot — a key that was never set,
a URL that is really just `localhost`, a `NODE_ENV` typo like `prodution`.

</details>

<details>
<summary>Can I use my existing .env.example as the schema?</summary>

Yes. Paste it into the schema field and set the schema format to **Template**: every key the
template lists becomes required and its placeholder values are ignored. If you leave the format on
**Auto**, a template whose keys have blank values (`DATABASE_URL=`) is read the same way, because a
key with no rules means required.

</details>

<details>
<summary>How do I fail a CI build on a bad environment?</summary>

Choose the **JSON** output and read the `ok` field — it is `false` whenever there is at least one
error. The object also carries `declared_keys`, `file_keys`, `error_count`, `warning_count` and an
`issues` array with a `line`, `key`, `severity`, `rule` and `message` per problem, which is enough
to annotate a pull request. Set *Keys not declared in the schema* to **error** if a stray variable
should also fail the build.

</details>

<details>
<summary>What counts as a missing variable?</summary>

A key that is absent from the file, and — unless you turn the option off — a key written as `KEY=`
with an empty value, since most loaders treat an empty string as unset. A missing key is an error
only when the schema marks it `required`; if the schema also documents a `default:`, it is reported
as a warning instead, because the loader is expected to supply the fallback.

</details>

<details>
<summary>Are my secrets safe to paste here?</summary>

The check runs entirely inside your browser tab as WebAssembly — no request carries the file
anywhere, and there is no server to store it. On top of that, values belonging to secret-looking
keys are never echoed back: the report says "a masked value of 12 characters" instead of printing
the token, so even the result is safe to share.

</details>

<details>
<summary>Which regular-expression syntax does pattern use?</summary>

Rust's `regex` syntax, which covers the usual Perl-style constructs — character classes, anchors,
repetition, groups, alternation — but not backreferences or lookaround. Patterns are unanchored, so
add `^` and `$` when you mean the whole value, as in `pattern:^sk_[a-z0-9]{16,}$`.

</details>
