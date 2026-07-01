## About this tool

**HTML Preview Bundler** stitches separate **HTML**, **CSS**, and **JavaScript**
into a single, self-contained `.html` document — everything inlined — so you can
save it, open it in any browser, or share it as one file.

- If your **HTML is a full document** (it contains `<html>`), the CSS is injected
  just before `</head>` and the JS just before `</body>`.
- If your **HTML is a fragment** (just markup, no `<html>`), it's wrapped in a
  clean HTML5 page with a `<meta charset>`, a viewport tag, and the **title** you
  provide.

Everything runs **locally in your browser** via WebAssembly — nothing is
uploaded. Copy the result, or paste it into a `.html` file and open it.

### Handy for

- Sharing a quick demo, snippet, or bug repro as one file.
- Turning a CodePen-style HTML/CSS/JS trio into something portable.
- Producing a single-file preview for email or chat.

> Note: CSS and JS are inlined verbatim. Avoid putting a literal `</script>`
> inside your JavaScript, as it would close the script tag early.

## FAQ

<details>
<summary>Do I need to fill in all three boxes?</summary>

No — any one of HTML, CSS, or JS is enough. A CSS-only or JS-only input still
produces a valid single-file page (the fragment wrapper supplies the document
shell). Only when all three are empty does the tool refuse with "provide at
least some HTML, CSS, or JS".

</details>

<details>
<summary>Where exactly does my CSS and JS end up in the output?</summary>

In a full document (one containing `<html>`), the `<style>` block is inserted
just before `</head>` (falling back to before `</body>`, then to the top of
the file) and the `<script>` goes just before `</body>` (or is appended). A
fragment is wrapped in an HTML5 shell — `<meta charset>`, a viewport tag, and
your **title** (default "Preview") — with the CSS in the head and the JS at
the end of the body.

</details>

<details>
<summary>Are images, fonts, or CDN scripts downloaded and inlined too?</summary>

No. Only the CSS and JS you paste are inlined — external references such as
`<img src>`, `@font-face` URLs, or `<script src>` CDN tags are left exactly as
written, so they will still be fetched from the network when the bundled file
is opened. For a fully offline file, inline those assets (e.g. as data URIs)
yourself first.

</details>

<details>
<summary>The bundled page renders broken — what's the usual cause?</summary>

A literal `</script>` sequence inside your JavaScript (even in a string) — the
code is inlined verbatim, so the browser ends the script tag right there.
Split it as `"</scr" + "ipt>"` or escape it as `<\/script>` and re-bundle.

</details>
