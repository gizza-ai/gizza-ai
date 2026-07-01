## Readability extractor

Paste the HTML of a cluttered web page and get back just the **main article** —
the title and body — with navigation, sidebars, ads, and boilerplate stripped
out. It's the same idea as a browser "reader mode," running locally in your
browser; nothing is uploaded.

### How it works

- Uses a Readability-style algorithm (a Rust port of Mozilla's Readability) to
  score the page's blocks and keep the densest, most article-like content.
- Choose **text** for clean readable plain text, or **html** for the cleaned
  article markup (keeps headings, paragraphs, links, etc.).

### Good for

- Saving an article to read later without the clutter.
- Feeding clean article text into a summarizer, word counter, or notes app.
- Cleaning scraped page source down to the real content.

### FAQ

<details>
<summary>Does it fetch the URL for me?</summary>

No — paste the page's HTML. (Use the web-fetch
tool first if you need to retrieve a page, then pass its HTML here.)

</details>

<details>
<summary>Why do I get "no article content found in the HTML"?</summary>

The Readability scoring couldn't find a dense, article-like block to keep. That
happens on landing pages, search/index pages, and — most often — on JavaScript
apps whose raw source is mostly `<script>` tags with no rendered text. For a
JS-rendered page, copy the *rendered* DOM instead (dev-tools → right-click
`<html>` → Copy → Copy outerHTML) and paste that.

</details>

<details>
<summary>Should I pick text or html output?</summary>

**text** (the default) gives clean plain text with the article title on the first
line — ideal for feeding into a summarizer or word counter. **html** keeps the
cleaned article markup: the title as an `<h1>` plus the surviving headings,
paragraphs, links, and images — better when you want to re-publish or restyle the
article.

</details>

<details>
<summary>Is anything uploaded?</summary>

No. The extractor is compiled to WebAssembly and runs
entirely in your browser tab.

</details>
