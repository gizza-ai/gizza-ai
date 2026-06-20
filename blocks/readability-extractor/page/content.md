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

**Does it fetch the URL for me?** No — paste the page's HTML. (Use the web-fetch
tool first if you need to retrieve a page, then pass its HTML here.)

**Is anything uploaded?** No. The extractor is compiled to WebAssembly and runs
entirely in your browser tab.
