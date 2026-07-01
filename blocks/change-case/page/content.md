## What this tool does

Convert text from one letter case to another instantly, right in your browser.
Nothing is sent to a server — it runs locally, works offline, and needs no
sign-up. Paste your text, pick a **Case**, and copy the result.

## Letter cases

These keep your spacing and punctuation and only re-map the letters.

| Case | What it does | Example |
| --- | --- | --- |
| **UPPERCASE** | Makes every letter a capital. | `the quick fox` → `THE QUICK FOX` |
| **lowercase** | Makes every letter small. | `The QUICK Fox` → `the quick fox` |
| **Title Case** | Capitalizes the first letter of each word. | `the quick fox` → `The Quick Fox` |
| **Sentence case** | Capitalizes the first letter of each sentence. | `hello. how are you?` → `Hello. How are you?` |
| **Capitalize** | Capitalizes only the very first letter. | `hELLO wORLD` → `Hello world` |
| **swap case** | Inverts every letter's case. | `Hello World` → `hELLO wORLD` |
| **aLtErNaTiNg** | Alternates lower/upper across the letters. | `hi there` → `hI tHeRe` |

## Programmer (identifier) cases

These re-split the text into words — at spaces, symbols, and `camelCase`
boundaries — and join them in a coding style. So `Hello world-foo` and
`helloWorld` both tokenize cleanly.

| Case | Output for `Hello world-foo` |
| --- | --- |
| **camelCase** | `helloWorldFoo` |
| **PascalCase** | `HelloWorldFoo` |
| **snake_case** | `hello_world_foo` |
| **CONSTANT_CASE** | `HELLO_WORLD_FOO` |
| **kebab-case** | `hello-world-foo` |
| **Train-Case** | `Hello-World-Foo` |
| **dot.case** | `hello.world.foo` |
| **path/case** | `hello/world/foo` |

Existing acronyms are handled too: `HTTPServer` → `http_server`,
`getURLFromAPI` → `get-url-from-api`.

## Examples

| Input | Case | Output |
| --- | --- | --- |
| `the quick brown FOX` | Title | `The Quick Brown Fox` |
| `straße` | UPPER | `STRASSE` |
| `hello world. how ARE you?` | Sentence | `Hello world. How are you?` |
| `Hello World` | swap | `hELLO wORLD` |
| `helloWorld` | snake | `hello_world` |
| `My API Key` | constant | `MY_API_KEY` |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

</details>

<details>
<summary>Does it handle accents and non-English letters?</summary>

Yes. Case conversion uses
full Unicode mapping, so `café` → `CAFÉ` and `straße` → `STRASSE`.

</details>

<details>
<summary>What's the difference between Title Case and Sentence case?</summary>

Title Case
capitalizes the first letter of *every* word; Sentence case capitalizes only the
first letter of each sentence (after a `.`, `!`, or `?`).

</details>

<details>
<summary>Can it convert camelCase to snake_case?</summary>

Yes — pick **snake_case** and it will
split an existing `camelCase` or `PascalCase` identifier into words first
(`helloWorld` → `hello_world`).

</details>
