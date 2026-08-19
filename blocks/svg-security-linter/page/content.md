## What this tool does

SVG is not a picture format — it is an XML document with a scripting model. An `.svg` file can carry a `<script>` element, an `onload=` attribute, a `javascript:` link, a `<foreignObject>` full of HTML, a DOCTYPE that pulls in a local file, or a reference to a server you do not control. That is why user-uploaded avatars, logos and icon packs are a recurring XSS vector: the file passes an image MIME check, then gets served back and inlined into a page.

Paste the markup and this linter walks it as **text** and reports every construct that makes the file dangerous, ranked high, medium or low, each with a line and column, a rule code, the element and attribute involved, and the source snippet that triggered it. Line and column both matter here, because real-world SVGs arrive minified onto a single line where "line 1" tells you nothing.

The report opens with a verdict:

- **unsafe** — at least one high-severity finding. Do not inline this file.
- **review** — findings exist, but none are high. Usually external references, `data:` URLs, or an unknown namespace.
- **clean** — nothing matched.

Pick **Ranked report** while you are reading, **JSON** to gate a CI job on `verdict`, or **CSV** to paste findings into a spreadsheet or a ticket.

## Worked example

Input — a plausible-looking icon that is four separate attacks:

```xml
<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">
  <script>alert(document.domain)</script>
  <a href="javascript:alert(2)" target="_blank"><text>go</text></a>
  <image href="https://evil.example/pixel.png"/>
</svg>
```

Output with **Minimum severity = All** and **Output format = Ranked report**:

```text
SVG security lint · verdict: unsafe · 5 findings · 3 high · 1 medium · 1 low

L1:41 [high] EVENT-HANDLER: <svg> onload — 'onload' is an inline event handler: its JavaScript runs when the event fires, with no <script> element anywhere in the file
  <svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">

L2:3 [high] SCRIPT: <script> — <script> element runs JavaScript when the SVG is opened directly or inlined into a page (it stays inert inside an <img> tag, but not in an <object>, <iframe> or inline <svg>)
  <script>alert(document.domain)</script>

L3:3 [low] ANCHOR-TARGET: <a> target — <a target="_blank"> without rel="noopener" hands the opened page a window.opener reference it can use to redirect the original tab
  <a href="javascript:alert(2)" target="_blank"><text>go</text></a>

L3:6 [high] JS-URL: <a> href — 'href' points at a javascript: URL; activating the reference executes the code
  <a href="javascript:alert(2)" target="_blank"><text>go</text></a>

L4:10 [medium] EXTERNAL-REF: <image> href — 'href' references an external host (https://evil.example/pixel.png); rendering the file makes that request, which leaks the viewer's IP and user agent and lets the remote host swap the content later
  <image href="https://evil.example/pixel.png"/>
```

Raise **Minimum severity** to **High only** and the same input still reads `verdict: unsafe`, lists the three high findings, and closes with:

```text
2 finding(s) below the selected severity are hidden; the verdict above still counts them.
```

A hand-authored icon with no active content returns:

```text
SVG security lint · verdict: clean · 0 findings · 0 high · 0 medium · 0 low

No XSS-risk constructs found.
```

## Rule codes

| Code | Severity | What it catches |
| --- | --- | --- |
| `SCRIPT` | high | A `<script>` element, inline or with a `href`/`xlink:href` source. |
| `EVENT-HANDLER` | high | Any `on*` attribute — `onload`, `onclick`, `onmouseover`, `onbegin`, and the rest. |
| `JS-URL` | high | Any attribute value or CSS declaration that resolves to a `javascript:`, `vbscript:`, `livescript:` or `mocha:` URL, plus CSS `expression(...)`. Values are entity-decoded first, so `&#106;avascript&#58;alert(1)` is caught. |
| `FOREIGN-OBJECT` | high | `<foreignObject>`, which embeds arbitrary HTML inside the graphic. |
| `EMBEDDED-HTML` | high | `iframe`, `object`, `embed`, `base`, `link`, `meta`, `audio`, `video`, `form` and friends appearing inside the SVG. |
| `ANIMATE-HREF` | high | SMIL `<animate>` / `<set>` whose `attributeName` retargets `href`, `style` or an `on*` handler — a classic sanitizer bypass. |
| `HANDLER` | high | The SVG 1.2 `<handler>` element and `ev:*` event attributes. |
| `DOCTYPE-ENTITY` | high / medium | Any DOCTYPE or `<!ENTITY>`. High when it declares a `SYSTEM`/`PUBLIC` external entity (XXE); medium for an internal subset or a bare DOCTYPE. |
| `DATA-URI` | high / medium / low | A `data:` URL. High for markup media types (`text/html`, `image/svg+xml`, XHTML, XML, anything containing `javascript`); medium for other non-image types; low for `image/*`. |
| `EXTERNAL-REF` | medium | An `http(s)` or protocol-relative reference to a host you do not control, in a URL attribute (`href`, `src`, `data`, `poster`, `action`, `formaction`, `xml:base`) or a CSS `url(...)`. |
| `CSS-IMPORT` | medium | `@import` inside a `<style>` block or a `style=` attribute, which pulls in a stylesheet that markup filtering never sees. |
| `XML-STYLESHEET` | medium | An `<?xml-stylesheet?>` processing instruction, which loads before element-level filtering. |
| `UNKNOWN-NS` | low | A namespace declaration that is not SVG, XLink, XML, or a known editor namespace. |
| `ANCHOR-TARGET` | low | `<a target="_blank">` without `rel="noopener"`. |

## Limits and edge cases

- **This is a reporter, not a sanitizer.** It never emits a "cleaned" file, on purpose. A blocklist scrub of untrusted SVG gives false assurance — the bypass list is long and keeps growing. If you accept SVG uploads, run an allowlist-based parser server-side, or serve the files from a separate origin with `Content-Disposition: attachment` and a restrictive CSP.
- **Nothing is rendered, fetched or uploaded.** The markup is scanned as text in WebAssembly in your browser. No DOM parses it, no entity is resolved, no URL in the file is requested, and no byte leaves the page.
- **Input is capped at 1,000,000 bytes.** Larger input is rejected with an error rather than silently truncated, so a partial scan can never be mistaken for a full one.
- **At most 500 findings are listed.** If a file exceeds that, the overflow is stated explicitly at the end of the report — never dropped in silence.
- **Minimum severity is a display filter only.** The verdict is computed before it is applied, so raising the threshold can never turn an unsafe file into a clean-looking report; hidden rows are counted in the closing note.
- **Allow external references is a policy switch, and it does change the verdict.** Turn it on only when remote `http(s)` references are genuinely acceptable in your pipeline — for example an icon set that legitimately points at your own CDN. It drops `EXTERNAL-REF` findings entirely; every other rule is untouched.
- **Ignore rule codes also changes the verdict**, because suppressed findings are removed before it is computed. An unrecognised code is an error, not a silent no-op, so a typo can never quietly disable a rule.
- **The scanner is deliberately forgiving.** Malformed markup is scanned as far as it parses instead of being rejected, because hostile files are frequently not well-formed. That means a badly broken document may under-report; it will not be waved through as clean by a parse error.
- **Comments and CDATA sections are skipped**, so a `<script>` written inside `<!-- ... -->` is not reported. This matches how a browser treats them, and keeps documented examples from producing noise.
- **Clean is not the same as safe.** A verdict of `clean` means none of the 14 rules matched. It says nothing about whether the graphic renders as expected, whether it is enormous, or whether it exploits a renderer bug.

## FAQ

<details>
<summary>Is an SVG with a &lt;script&gt; in it actually dangerous if I only use it in an &lt;img&gt; tag?</summary>

Inside `<img src="...">` browsers do not run scripts in the referenced SVG, so that specific case is inert. The risk is everywhere else the same file ends up: inlined into HTML, loaded in an `<object>` or `<iframe>`, opened directly by a user clicking the file URL, or fetched and injected by a front-end framework. Files also outlive the code path they were uploaded for. Treat `verdict: unsafe` as "do not serve this from your origin", not "do not put it in an `<img>`".

</details>

<details>
<summary>Why doesn't the tool just strip the dangerous parts and hand me a clean file?</summary>

Because a text-level blocklist scrub of untrusted SVG is unsound, and shipping one would be worse than shipping nothing — it converts "I know this file is hostile" into "a tool told me it was fine". Namespace tricks, character references, SMIL attribute retargeting and CSS can all reintroduce active content past a naive filter. Sanitizing properly means parsing to a real XML tree and rebuilding it from an allowlist of elements and attributes, on the server. This tool tells you what is in the file and where; it leaves the rewrite to a parser that can do it correctly.

</details>

<details>
<summary>What is the difference between "Minimum severity" and "Ignore rule codes"?</summary>

**Minimum severity** hides rows from the listing but does not affect the verdict — a file with one high finding still reads `unsafe` even at `High only`, and the report tells you how many rows were hidden. **Ignore rule codes** removes those findings entirely, before the verdict is computed, so ignoring the only high-severity rule that matched can move a file from `unsafe` to `review` or `clean`. Use the severity filter for reading; use ignore only for rules you have consciously reviewed and accepted.

</details>

<details>
<summary>Why is a plain https:// reference to an image flagged at all?</summary>

`EXTERNAL-REF` is medium, not high, because it is a privacy and supply-chain issue rather than direct code execution. Rendering the SVG makes the browser fetch that URL, which discloses the viewer's IP address and user agent to the remote host, and lets whoever controls that host swap the content later — the file you reviewed is not necessarily the file your users see. If remote references are an accepted part of your pipeline, tick **Allow external references** and the finding disappears.

</details>

<details>
<summary>My SVG came out of Inkscape or Illustrator and reports low-severity findings. Is that normal?</summary>

Editor exports carry extra namespaces and metadata, and the usual suspects are recognised and not flagged: SVG itself, XLink, XML, Inkscape, Sodipodi, Dublin Core, Creative Commons, RDF, Adobe Illustrator, Sketch, Serif and svg.js. An unfamiliar namespace is reported as `UNKNOWN-NS` at low severity — informational, since a namespace declaration on its own executes nothing, but worth a glance because it is how content from an unexpected vocabulary gets carried along. If you have reviewed it, add `UNKNOWN-NS` to **Ignore rule codes**.

</details>

<details>
<summary>Can I run this over a directory of files in CI?</summary>

Yes — use the CLI form shown above, pass `format=json`, and fail the job when `verdict` is `unsafe` (or when `summary.high` is above zero). The JSON payload is `{ verdict, summary, findings[] }`, where `summary` carries `findings`, `high`, `medium`, `low`, `hidden_by_min_severity` and `not_listed`, and each finding has `line`, `column`, `severity`, `code`, `element`, `attribute`, `message` and `snippet`. `csv` gives you the same findings rows with a header, for attaching to a ticket.

</details>
