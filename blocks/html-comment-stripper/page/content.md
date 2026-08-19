## About this tool

Comments are for the people writing the page, not the people reading it. Ticket numbers, commented-out sections, staging notes and build stamps all ship to production inside the HTML, where they add bytes and occasionally leak something you would rather not publish. The usual fix — a `<!--.*?-->` search-and-replace, or reaching for a minifier — trades one problem for another: the regex deletes things that only look like comments, and the minifier reformats the whole document so the diff is unreviewable.

This tool does exactly one thing: **the output is the input minus the comment bytes**. No whitespace collapsing, no tag or attribute normalization, no re-indentation, no quote-style changes. Run it on a template and the diff shows the comments you removed and nothing else.

It also knows that not every comment is a note. Comments that carry meaning — Internet Explorer conditionals, server-side includes, licence banners — are recognized as kinds and **kept by default**, and everything else can be selected by regular expression.

### Worked example

Input:

```html
<!--! (c) 2026 Example Ltd -->
<h1>Hello</h1>
<!-- internal note: swap the hero copy -->
<p>Ship it.</p>
```

Output, with every control left at its default:

```html
<!--! (c) 2026 Example Ltd -->
<h1>Hello</h1>

<p>Ship it.</p>
```

The licence banner survived because its text starts with `!`, which is the long-standing convention for "do not strip this". The note is gone — and the line it sat on is now empty, because the default is to change no whitespace at all. Set **Blank lines** to *Drop lines a removed comment left blank* and the third line disappears too:

```html
<!--! (c) 2026 Example Ltd -->
<h1>Hello</h1>
<p>Ship it.</p>
```

Switch **Output** to the dry run and you get an inventory instead of markup, so you can check a rule before trusting it:

```text
line,kind,action,comment
1,bang,kept,<!--! (c) 2026 Example Ltd -->
3,plain,removed,<!-- internal note: swap the hero copy -->
```

The report output answers the other question — how much this actually saved:

```text
metric,value
comments_found,2
comments_removed,1
comments_kept,1
removed_plain,1
removed_conditional,0
removed_ssi,0
removed_bang,0
css_comments_removed,0
bytes_before,104
bytes_after,62
bytes_saved,42
percent_smaller,40.38
```

### Controls

- **HTML** — the markup to clean, pasted in. Up to 5,000,000 bytes.
- **Keep IE conditional comments** — on by default. `<!--[if lt IE 9]> … <![endif]-->` and the downlevel-revealed split forms are instructions to old browsers about which files to load; deleting them changes behaviour, so it is an explicit choice.
- **Keep server-side includes** — on by default. A comment whose text starts with `#`, such as `<!--#include virtual=… -->`, is a directive the web server acts on. Removing it silently removes part of the page.
- **Keep banner / licence comments** — on by default. A comment whose text starts with `!` is the industry marker for a copyright header or licence notice that must survive minification.
- **Pattern** and **Pattern means** — a regular expression matched against each comment's *inner* text (what is between the delimiters). Under *Keep matching comments* it is a protect-list: `RELEASE` keeps every comment mentioning a release, whatever kind it is. Under *Remove ONLY matching comments* the tool inverts — only matches are deleted and everything else stays, which is how `^\s*/?wp:` clears WordPress block markers out of exported content without touching real notes.
- **Also remove /\* … \*/ comments inside `<style>`** — off by default, so a run is purely an HTML-comment operation. The CSS pass is string-aware: a `/*` inside `content: "/* not a comment */"` is left alone.
- **Blank lines** — *keep* (default) changes no whitespace; *trim* drops lines a removal emptied while leaving lines that were already blank; *collapse* also folds runs of blank lines into one.
- **Output** — the cleaned HTML, the per-metric report, or the dry-run listing of every comment with its line, kind and fate.

### What it will not mistake for a comment

This is where a regex quietly corrupts a file and this scanner does not:

- **Raw-text elements.** Inside `<script>`, `<style>`, `<textarea>` and `<title>` the HTML tokenizer is in a raw-text state, so `var s = "<!-- hi -->"` is a string, not a comment. Those regions are copied through untouched (apart from the opt-in CSS pass). Note that `<pre>` is *not* in that list — a comment inside a `<pre>` really is a comment.
- **Quoted attribute values.** `<a title="<!-- hi -->">` contains no comment, and a `>` inside an attribute cannot end a tag early.
- **Nesting.** Comments do not nest in HTML. `<!-- a <!-- b --> c -->` ends at the **first** `-->`, so ` c -->` is text — the same conclusion a browser reaches.

### Limits and edge cases

The document is capped at **5,000,000 bytes**; one byte over is refused rather than truncated. An **unterminated comment** — a `<!--` with no closing `-->` — is an error that names the line, because the alternative is deleting the entire rest of the document. An empty document is an error too. The pattern uses the Rust `regex` engine, which has no backreferences and no lookaround; that restriction is exactly what guarantees linear-time matching, so nothing you can type here will hang the page. Selecting *Remove ONLY matching comments* with an empty pattern is rejected instead of silently doing nothing. Line numbers in the dry run are 1-based and refer to the **input**. Blank-line trimming tracks lines emptied by an HTML comment removal only, so a `<style>` line left blank by the optional CSS pass stays where it is. Everything runs locally in your browser; the markup is never uploaded.

## FAQ

<details>
<summary>Why keep conditional comments — aren't they dead?</summary>

Internet Explorer stopped honouring them in IE 10, so on a modern-only site they are dead weight and you can untick the box. But a conditional comment is not a note: `<!--[if lt IE 9]><script src="shiv.js"></script><![endif]-->` is the only thing loading that script for the browsers that still need it, and the downlevel-revealed form `<!--[if !IE]>-->…<!--<![endif]-->` actually wraps content that *every* browser renders. Deleting one half of that pair changes what is on the page.

Because the cost of wrongly removing them is a broken page and the cost of wrongly keeping them is a few dozen bytes, the default is to keep. The same reasoning applies to the `#` (server-side include) and `!` (licence banner) kinds: all three are meaningful markup dressed as comments, and all three have their own switch so you can strip them deliberately.

</details>

<details>
<summary>How is this different from running a minifier?</summary>

A minifier's job is to make the file small, so removing comments is one of a dozen transformations it applies — it will also collapse whitespace, drop optional closing tags, unquote attributes and rewrite boolean attributes. That is fine for a build artifact and terrible for a source file you intend to keep editing, because the resulting diff touches every line.

Here the contract is narrower and easier to review: every byte that was not part of a removed comment comes out in the same order, at the same indentation, with the same quoting. You can run this on a template in your repository, commit the result, and the diff will show only the comments that went away. If you actually want a small file, minify *after* this step.

</details>

<details>
<summary>Can it remove comments from my JavaScript too?</summary>

Deliberately not. Comments inside `<script>` are left exactly as they are, because doing it correctly needs a real JavaScript lexer: `//` appears inside string literals and URLs, `/*` appears inside regular expression literals, and telling a regex literal from a division operator requires tracking the parser state. A naive pass silently corrupts working code, and the corruption often survives testing.

CSS is a different story — its string rules are simple enough to handle safely — so `/* … */` inside `<style>` is offered as an opt-in checkbox with string-aware matching. For JavaScript, run the output through a dedicated JS minifier, which removes comments as part of a proper parse.

</details>

<details>
<summary>How do I strip CMS block markers like &lt;!-- wp:paragraph --&gt; and keep everything else?</summary>

Set **Pattern** to `^\s*/?wp:` and **Pattern means** to *Remove ONLY matching comments*. That inverts the whole tool: instead of removing everything except the protected kinds, it removes nothing except what the pattern matches, so the opening `<!-- wp:paragraph -->` and its closing `<!-- /wp:paragraph -->` both go while your genuine comments stay.

The pattern is matched against the comment's inner text only, with the `<!--` and `-->` delimiters excluded, which is why the expression starts at `^\s*` rather than trying to match the delimiters. The same trick works for analytics placeholders, template-engine markers, or any other machine-generated comment with a recognisable prefix — and because the match wins over the kind rules, it can even target a conditional or banner comment you specifically want gone.

</details>

<details>
<summary>Why did I get an "unterminated comment" error instead of output?</summary>

Because somewhere in the document there is a `<!--` that is never closed by a `-->`, and there is no safe way to guess what you meant. A browser treats the rest of the file as comment text and renders nothing after it; a regex-based stripper usually deletes everything from that point to the end of the file. Both outcomes destroy data quietly, so this reports the line number of the offending `<!--` and refuses to write anything.

The usual causes are a typo'd closer (`--!>`, `-- >`), a comment that got cut in half by a template include, or markup pasted from a diff. Fix the opener it names and re-run. If the document is genuinely truncated, closing the comment by hand at the end of the file is enough to get a clean pass.

</details>
