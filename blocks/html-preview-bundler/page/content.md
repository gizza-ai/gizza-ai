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
