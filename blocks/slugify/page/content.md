## What this tool does

Turn a title, heading, or any phrase into a clean, **URL-safe slug** — the
lowercase, hyphenated string you put in a permalink, like
`/blog/10-tips-for-creme-brulee`. Everything runs locally in your browser:
nothing is sent to a server, it works offline, and there's no sign-up.

Type a **Title or phrase** and the slug updates instantly. Optionally change the
**Separator**, keep the original case, or set a **Max length**.

## How a slug is built

1. **Transliterate to ASCII** — accents and non-Latin scripts are folded to their
   closest ASCII letters, so `Crème Brûlée` becomes `creme-brulee` and `北京`
   becomes `bei-jing`.
2. **Drop apostrophes** — contractions and possessives join up, so `Bob's` becomes
   `bobs`, not `bob-s`.
3. **Lowercase** (optional, on by default).
4. **Collapse separators** — every run of spaces and punctuation becomes a single
   separator, with no leading or trailing separator.

## Options

| Option | What it does |
| --- | --- |
| **Separator** | The character between words — `-` (default) for a classic kebab-case slug, or `_` for snake_case. Any non-alphanumeric string works. |
| **Lowercase** | On by default. Turn it off to keep the original capitalisation (e.g. `Hello-World`). |
| **Max length** | `0` means no limit. A positive value truncates the slug on a word boundary so it never cuts a word in half. |
| **Slugify each line separately** | Off by default. Turn it on and put one title per line to slugify a whole batch at once — each line becomes its own slug. |

## Examples

| Input | Settings | Slug |
| --- | --- | --- |
| `Hello, World!` | defaults | `hello-world` |
| `10 Tips for Crème Brûlée!` | defaults | `10-tips-for-creme-brulee` |
| `Münchner Straße` | defaults | `munchner-strasse` |
| `Bob's Burgers` | defaults | `bobs-burgers` |
| `北京` | defaults | `bei-jing` |
| `Hello World Test` | separator `_` | `hello_world_test` |
| `Keep The Case` | lowercase off | `Keep-The-Case` |
| `The Quick Brown Fox` | max length 13 | `the-quick` |
| `Hello World`<br>`Foo & Bar` | slugify each line | `hello-world`<br>`foo-bar` |

## FAQ

**Is it free and private?** Yes — your text never leaves your device, and the tool
keeps working offline once the page has loaded.

**What's a slug?** It's the readable, URL-safe part of a web address that
identifies a page, such as `creme-brulee-recipe` in
`example.com/recipes/creme-brulee-recipe`. Slugs use only lowercase letters,
digits, and hyphens, which is friendly for both readers and search engines.

**How are accented and non-Latin characters handled?** They're transliterated to
the closest ASCII equivalent — `é`→`e`, `ñ`→`n`, `ß`→`ss`, and scripts like
Chinese, Cyrillic, or Greek are romanised — so the slug stays plain ASCII.

**Can I use underscores instead of hyphens?** Yes — set the **Separator** to `_`
(or any non-alphanumeric character) to change the word separator.

**Does it limit the length?** Only if you want it to. Set **Max length** to a
positive number and the slug is trimmed on a word boundary so words aren't cut in
half; leave it at `0` for no limit.
