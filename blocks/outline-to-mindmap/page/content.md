## About this tool

Paste an **indented text outline** and get back a clean, scalable **mind-map
SVG** — laid out and drawn entirely in your browser with a pure-Rust engine, so
there's no sign-up and nothing is ever uploaded.

Every line becomes a node. The amount you indent a line — with **spaces or
tabs** — decides where it hangs in the tree, so the structure you already type
becomes the structure of the map. Leading bullet or number markers
(`-`, `*`, `+`, `•`, `1.`) are stripped automatically, so you can paste a
to-do list, a meeting agenda, or a Markdown outline straight in.

A single top-level line becomes the **central topic**. If your outline has
several top-level lines, they're grouped under one central node — set its label
with the **central topic** field.

## Options

- **Layout direction** — *right* puts the central topic on the left with
  branches fanning out to the right (the classic mind-map look), or *down* puts
  it on top with the tree growing downward like an org chart.
- **Color each branch** — give every top-level branch and its descendants their
  own color, or turn it off for a clean monochrome diagram.
- **Dark mode** — recolor the whole map with light text on a dark canvas, ready
  to drop onto a dark slide or page.

## Example

```text
Launch plan
  Product
    Pricing
    Onboarding
  Marketing
    Landing page
    Email list
  Support
    Docs
    Help desk
```

The result is a standalone `.svg` you can save, embed in a page, or drop into a
slide — it stays crisp at any size because it's vector, not a bitmap.

## FAQ

<details>
<summary>What indentation should I use?</summary>
<p>Anything consistent works — two spaces, four spaces, or tabs. Deeper
indentation simply means deeper nesting; the tool measures each line's leading
whitespace (tabs count as up to four columns) and builds the tree from that, so
mixed or irregular indents are handled gracefully.</p>
</details>

<details>
<summary>Can I paste a bullet list?</summary>
<p>Yes. Leading <code>-</code>, <code>*</code>, <code>+</code>, <code>•</code>
bullets and numbered markers like <code>1.</code> are removed automatically, so
a Markdown or to-do list maps cleanly.</p>
</details>

<details>
<summary>Is my outline uploaded anywhere?</summary>
<p>No. Parsing, layout, and SVG rendering all run locally in your browser. Your
text never leaves the device.</p>
</details>

<details>
<summary>What can I do with the SVG?</summary>
<p>Because it's a vector image, you can scale it to any size without blur, embed
it in a web page or README, open it in a vector editor, or convert it to PNG or
PDF with the other gizza tools.</p>
</details>

## Privacy

The mind map is parsed, laid out, and rendered locally in your browser. Your
outline is never sent to a server.
