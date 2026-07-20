## About this tool

This scanner reads a code snippet and points at the exact lines where a SQL query is
**assembled from a variable** instead of being sent as a parameterized statement — the root
cause of most SQL-injection bugs. It is a static, pattern-based check (in the spirit of a
security linter): it never runs your code, never opens a database, and nothing you paste
leaves the browser.

It flags four construction patterns:

- **String concatenation** — `"SELECT … WHERE name = '" + name + "'"` in Python/JS/Java/C#,
  or PHP's `"… WHERE id = " . $id`. Rule `SQLI-CONCAT`, high severity.
- **String interpolation** — Python f-strings (`f"… {id}"`), JS/TS template literals
  (`` `… ${id}` ``), C# interpolated strings (`$"… {id}"`), and Ruby `"… #{id}"`. Rule
  `SQLI-INTERP`, high.
- **Format / printf building** — `"…".format(v)`, `"… %s" % v`, and
  `sprintf`/`fmt.Sprintf`/`String.Format`. Rule `SQLI-FORMAT`, high.
- **Executing a bare variable** — `cursor.execute(sql)` where `sql` is a variable whose
  construction the scanner can't see. Rule `SQLI-EXEC-VAR`, medium (a prompt to check how
  that string was built).

A call that passes values as bound parameters — `execute("… WHERE id = %s", (uid,))` — is
recognized as safe and produces no finding.

### Worked example

Input (language `python`):

```
cursor.execute("SELECT * FROM users WHERE name = '" + name + "'")
```

Output:

```
1 finding(s) (1 high, 0 medium) in 1 line(s) scanned

line 1, col 16  HIGH  SQLI-CONCAT  SQL query built with string concatenation; a variable is joined directly into the SQL text.
  cursor.execute("SELECT * FROM users WHERE name = '" + name + "'")

Fix: use parameterized queries / prepared statements — pass user input as bound parameters (?, %s, :name, $1), never concatenate it into the SQL string.
Example: cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))
```

Switch **Output format** to JSON for a structured `{ summary, findings[] }` object you can
feed into other tooling.

### Limits and edge cases

- Detection is **line-based and heuristic**: a query split so that no single physical line
  contains both a SQL keyword and the concatenation may be missed, and a non-query string
  that happens to contain a SQL keyword plus a `+` can be a false positive. Treat findings as
  leads for review, not proof — and the absence of findings as "nothing matched", not a
  guarantee of safety.
- A line is only checked for the high-severity patterns if it contains **both** a quoted
  string and a SQL keyword (`SELECT`, `FROM`, `WHERE`, `INSERT INTO`, …).
- The **Language** setting scopes which syntaxes fire: pick PHP and JS `${…}` template
  literals are ignored; pick JavaScript and PHP's `.` concatenation is ignored. Leave it on
  **Auto** to apply every syntax at once.
- **Show → High severity only** hides the medium `SQLI-EXEC-VAR` warnings.
- This finds *vulnerable query construction in code*. It does **not** test whether a given
  input string is an attack payload (`' OR 1=1 --`), and it does not scan a live URL — those
  are different jobs.

## FAQ

<details>
<summary>What exactly does the scanner flag?</summary>

Lines where a SQL statement is built from a variable rather than a bound parameter: string
concatenation (`+`, or PHP `.`), string interpolation (f-strings, template literals, C#
`$"…"`, Ruby `#{…}`), and format/printf building (`.format()`, `%`, `sprintf`,
`String.Format`). It also raises a medium warning when a bare variable is passed to
`execute()`/`query()`, because it can't see how that string was assembled. A call that binds
values as parameters is treated as safe.

</details>

<details>
<summary>Is a clean result a guarantee my code is safe from SQL injection?</summary>

No. This is a static pattern match, not a prover. It can miss a query whose dynamic part is
split across lines, a concatenation hidden behind a helper function, or an ORM misuse it
doesn't recognize. Use it as a fast first pass in code review, then confirm with
language-specific static analysis (for example a security linter) and by testing the running
application.

</details>

<details>
<summary>How do I fix a flagged line?</summary>

Replace the concatenated/interpolated value with a bound parameter and let the database driver
substitute it. In Python: `cursor.execute("… WHERE id = %s", (user_id,))`. In PHP with PDO:
`$pdo->prepare("… WHERE id = :id")->execute(['id' => $id])`. In Node with pg:
`db.query("… WHERE id = $1", [id])`. The literal placeholder syntax (`?`, `%s`, `:name`,
`$1`) depends on your driver, but the principle is the same: the SQL string stays constant and
user values travel separately.

</details>

<details>
<summary>Why was my parameterized query not flagged, but a similar one was?</summary>

A call like `execute("… = %s", (val,))` keeps the SQL text constant and passes the value as a
second argument, so the scanner sees no variable inside the SQL string and stays quiet. If you
instead build the string first — `execute("… = " + val)` or `execute(f"… = {val}")` — the
value is now part of the SQL text, which is exactly the injection pattern it reports.

</details>

<details>
<summary>Does anything I paste get uploaded?</summary>

No. The scan runs as WebAssembly inside your browser tab; the code you paste is never sent to
a server. You can disconnect from the network and it still works.

</details>

<details>
<summary>Which languages does the Language setting understand?</summary>

Auto (all syntaxes), Python, PHP, JavaScript, TypeScript, Java, C#, Go, and Ruby. Choosing a
specific language narrows the patterns to that language's concatenation/interpolation style
(fewer false positives) and tailors the fix example at the bottom of the report. Auto is a
safe default when you're pasting mixed or unknown code.

</details>
