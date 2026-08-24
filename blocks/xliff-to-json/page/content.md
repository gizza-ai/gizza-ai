## About this tool

XLIFF to JSON reads an XLIFF localization file — the `.xlf` / `.xliff` format that
CAT tools, Angular, and most translation vendors exchange — and turns its
translation units into JSON you can actually use: a source/target pair for every
unit id, a flat bundle of just the translations, or a nested tree for an i18n
library. Paste the file and the conversion runs locally in your browser; nothing
is uploaded.

Both dialects are read by the same pass, so you never have to tell it which one
you have:

- **XLIFF 1.2** — `<trans-unit id resname>` with `<source>`, `<target>`, and `<note>`.
- **XLIFF 2.x** — `<unit id name>` › `<segment>` › `<source>`/`<target>`, with notes
  under `<notes>`.

## Output shapes

- **Source + target pairs** (the default) gives `{ "home.title": { "source": "Welcome",
  "target": "Willkommen" } }` — lossless, and the shape to use when you are reviewing
  or diffing a translation.
- **Translations only** gives `{ "home.title": "Willkommen" }`, which drops straight
  into an i18n bundle.
- **Source text only** gives the same shape keyed to the original strings — useful for
  extracting a fresh string list.
- **Array of records** gives `[ { "id": …, "source": …, "target": … } ]`. Object keys
  have to be unique, so this is the shape that survives a file which legally repeats
  an id across several `<file>` elements.

Keys come from the unit `id` by default. You can key by the `resname` (1.2) / `name`
(2.x) attribute or by the source text instead; both fall back to the id when a unit
doesn't have one, so no unit is ever silently dropped. Turn on nesting to split each
key on a separator — `home.hero.title` becomes `{ "home": { "hero": { "title": … } } }`.

## Placeholders are preserved

A naive text-only extraction quietly deletes inline markup, which is how an
`{{name}}` interpolation disappears from a bundle and ships broken. By default each
inline code element (`<x/>`, `<ph>`, `<g>`, `<bpt>`/`<ept>`, `<pc>`, `<sc>`/`<ec>`)
is rendered as its `equiv-text` when the file supplies one, and as `{id}` otherwise —
so an Angular `messages.xlf` round-trips its interpolations intact. Switch to
**Strip** for plain prose, or **Keep** to hold on to the inline XML verbatim.

## Untranslated units

Missing `<target>` and empty `<target></target>` mean the same thing here — no
translation — because CAT tools disagree about which one to emit. Untranslated units
are included by default with an empty target, so you can see what still needs work.
Turn off "Include untranslated units" to get only finished strings, or turn on the
source fallback to fill the gaps with the original text. The fallback is off by
default on purpose: filling silently means untranslated English ships as if it were
the translation.

## What else it handles

- `<group>` nesting is walked, not rejected — with metadata on, each unit reports its
  group path.
- Several `<segment>` children of one XLIFF 2.x `<unit>` are joined in document order;
  `<ignorable>` content is skipped.
- `<alt-trans>`, `<seg-source>` and translation-memory matches are ignored, so
  fuzzy suggestions never overwrite the real translation.
- XML entities and CDATA are decoded, `xml:space="preserve"` is honoured, and
  namespace prefixes are reduced to local names.

## Private and offline

Everything runs in your browser via WebAssembly. Your translation file never leaves
your device — no upload, no account, no tracking. Output is deterministic
pretty-printed JSON in document order, so re-running the same file gives a byte-identical
result you can commit and diff.

## FAQ

<details>
<summary>Does this translate my text?</summary>

No. It extracts what the file already contains. XLIFF stores the source string and,
where a translator has filled one in, the target string; this tool reads both and
reshapes them into JSON. Units with no target come out empty (or filled from the
source, if you enable that option) — nothing is machine-translated.

</details>

<details>
<summary>Do I need to know whether my file is XLIFF 1.2 or 2.0?</summary>

No. The parser captures `<source>`/`<target>` when their parent is a `<trans-unit>`
(1.2) or a `<segment>` (2.x), so both dialects are read by the same pass and a mixed
or unlabelled file still works. Vendor files such as `.sdlxliff` and `.mqxliff` are
XLIFF underneath, so their translation units parse fine; vendor-private namespaced
extensions are not interpreted.

</details>

<details>
<summary>My file uses &lt;group&gt; tags — will that break the conversion?</summary>

No. Groups are legal in both versions and CAT tools emit them constantly, so they are
walked transparently and the units inside come out like any others. Enable "Include
notes, state, file and group" and each unit also reports the slash-joined path of the
groups that enclose it.

</details>

<details>
<summary>Why did my &#123;&#123;name&#125;&#125; placeholder survive but the bold tags turn into &#123;1&#125;?</summary>

Inline elements are rendered from the best marker the file provides. Angular writes
`equiv-text="{{name}}"` on its interpolation elements, so that exact text is restored.
Paired formatting codes like `<bpt id="1">`/`<ept id="1">` usually carry no
`equiv-text`, so they fall back to `{id}` — `{1}` in that case. Choose **Strip** if you
want the markup gone entirely, or **Keep** to get the raw inline XML back.

</details>

<details>
<summary>Two units have the same id and one of them vanished. Why?</summary>

A JSON object cannot hold two members with the same key, so in the object shapes the
last unit with a given id wins. This is legal in XLIFF — one document can contain
several `<file>` elements that each define an id. Switch the output shape to **Array
of records** to keep every unit, and turn on metadata to see which `<file>` each one
came from.

</details>

<details>
<summary>Nesting failed with a "cannot nest key" error — what happened?</summary>

Nesting turns `a.b` into `{ "a": { "b": … } }`, which is impossible if some other unit
already claimed `a` as a plain string. A file containing both `home` and `home.title`
hits exactly that collision. Either turn nesting off, or pick a separator that your
ids don't otherwise use.

</details>
