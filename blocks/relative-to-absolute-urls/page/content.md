## About this tool

A relative URL only means something next to the page it came from. Move that markup anywhere else — into an email, an RSS item, a scraped archive, a static export, a CMS field, another domain — and `href="../about.html"` either 404s or silently points at a page nobody meant to link to. The fix is mechanical: resolve every relative value against the address the markup actually lived at.

Doing that with search-and-replace is where it goes wrong. A `href="([^"]*)"` pass rewrites links inside comments and `<script>` strings, mangles template placeholders, misses `srcset`, `poster` and `<meta http-equiv="refresh">`, and cannot resolve `../` at all. Running the HTML through a parser instead usually reformats the whole document, so the diff is unreviewable.

This tool takes the narrow contract: **only URL attribute values change**. Same whitespace, same attribute order, same quoting, same indentation. Resolution is the WHATWG algorithm — the one your browser applies when you click a link — so `../`, `/`, `./`, `?query=only` and dot segments all behave the way they do in the address bar.

### Worked example

Input, with the base URL set to `https://example.com/blog/post.html`:

```html
<a href="../about.html">About</a>
<img src="images/logo.png" alt="Logo">
<a href="#top">Back to top</a>
```

Output:

```html
<a href="https://example.com/about.html">About</a>
<img src="https://example.com/blog/images/logo.png" alt="Logo">
<a href="#top">Back to top</a>
```

Three things happened. `../about.html` climbed one directory out of `/blog/`, because the base names a *file* and its directory is `/blog/`. `images/logo.png` resolved inside that same directory. And `#top` was left alone — inside the original page it is a scroll, not a link, so absolutizing it by default would turn a jump into a page load. Tick **Also absolutize bare #anchor links** when the markup is being lifted out of its page and the anchor has to point back at it.

Switch **Output** to the dry run and you get a verdict per URL instead of markup, which is how you check a base before trusting it on a whole document:

```text
line,tag,attribute,original,resolved,action
1,a,href,../about.html,https://example.com/about.html,rewritten
2,img,src,images/logo.png,https://example.com/blog/images/logo.png,rewritten
3,a,href,mailto:hi@example.com,,kept:scheme
4,a,href,#top,,kept:fragment
```

### Controls

- **HTML** — the markup to rewrite, pasted in. Up to 5,000,000 bytes.
- **Base URL** — the absolute address the relative values are relative to, normally where the markup came from. A base ending in a filename (`/blog/post.html`) resolves siblings into `/blog/`; a base ending in a slash (`/blog/`) treats that as the directory. Anything without a scheme, or a `mailto:`-style address that cannot be a base, is rejected with an explanation rather than guessed at.
- **Attributes to rewrite** — *href and src only* is the conservative pass. *Common* (default) adds `srcset` (each candidate, `1x`/`640w` descriptors preserved), `poster`, form `action`, `formaction`, object `data`, the `background` attributes, `ping`, and the URL buried in `<meta http-equiv="refresh" content="5; url=…">`. *All* adds the rarities HTML also defines as URLs: `cite`, `longdesc`, `manifest`, `profile`, `itemtype`, `icon` and the applet/object `archive`, `code`, `codebase` and `object` attributes.
- **Honour a `<base href>`** — on by default, because that is what a browser does. If the pasted document carries `<base href="/assets/">`, its relative URLs resolve against `/assets/`, not against the page address you typed. The `<base href>` itself is still made absolute against your base.
- **Protocol-relative URLs** — `//cdn.example.com/a.png` has a host but no scheme. *Resolve* (default) gives it the base's scheme, which is what an email client or feed reader needs; *keep* leaves it alone, which is right when the output is still served over both http and https.
- **Also absolutize bare #anchor links** — off by default; see the worked example above.
- **Also rewrite `url(…)` and `@import` in CSS** — off by default, so a run is purely an attribute operation. When on, `style="background:url(img/hero.jpg)"` and the contents of `<style>` blocks are rewritten too, keeping the original quoting and spacing.
- **Output** — the rewritten HTML, a report (the base actually used, whether a `<base>` tag was honoured, counts of rewritten versus kept, bytes before and after), or the dry-run listing.

### What it deliberately leaves alone

- **Already-absolute URLs** — anything with a scheme and a host, so a second run over the same document changes nothing.
- **Other schemes** — `mailto:`, `tel:`, `data:`, `javascript:`, `sms:`, `blob:` and friends are not relative and are never touched.
- **Template placeholders** — a value starting `{{`, `{%`, `{#`, `<%`, `${` or `[[` is a hole the template engine still has to fill; baking a base into it would break the build.
- **Comments and raw-text elements** — a link written inside `<!-- … -->`, `<script>`, `<textarea>` or `<title>` is text, not markup. `<style>` is only touched when the CSS pass is on.
- **Empty values** — `href=""` means "this page" and stays as written.

### Limits and edge cases

The document is capped at **5,000,000 bytes**; one byte over is refused rather than truncated. Resolution normalizes the way a browser does: spaces become `%20`, dot segments collapse, and a default port disappears — so a rewritten value is a canonical URL, not a concatenation. HTML entities inside a value (`&amp;` in a query string) are preserved as written. An unquoted attribute value is rewritten in place and only gains quotes if the resolved URL would need them. A value that cannot be resolved at all is left untouched and counted as `kept:unresolvable` rather than failing the whole run, so one broken link cannot cost you the document. Line numbers in the dry run are 1-based and refer to the input. External stylesheets are not fetched — URLs inside a linked `.css` file are out of reach; paste its contents through the CSS pass separately or use the `<style>` block. Nothing is fetched or uploaded: the markup is scanned locally in your browser.

## FAQ

<details>
<summary>Why is my base URL's last path segment being dropped?</summary>

Because that is what a relative URL means. In `https://example.com/blog/post.html`, the *directory* is `/blog/` and `post.html` is a file inside it, so `images/logo.png` resolves to `/blog/images/logo.png` — the filename is replaced, not appended to. This is the same rule your browser uses, and it is why a link on that page written as `images/logo.png` works.

If you actually want the last segment treated as a directory, put a trailing slash on it: base `https://example.com/blog/post/` resolves `images/logo.png` to `/blog/post/images/logo.png`. The distinction matters most for clean URLs, where `https://example.com/blog/post` and `https://example.com/blog/post/` are the same page to the server but different bases to the resolver. Use the report output to see the effective base a run actually used.

</details>

<details>
<summary>What happens to links that are already absolute — can I run this twice?</summary>

Yes, safely. A value that already carries a scheme and a host is recognised and left byte-identical, so the operation is idempotent: running it again over its own output changes nothing. That also means you can paste a document that is half-converted — a page whose images are already on a CDN but whose internal links are still relative — and only the relative half moves.

The same protection covers non-web schemes. `mailto:`, `tel:`, `data:`, `javascript:`, `sms:` and anything else with a scheme are not relative references at all, and resolving them would produce nonsense, so they are skipped and counted separately in the report. If you want to see exactly which values were skipped and why, switch **Output** to the dry run — every URL gets a line and a verdict.

</details>

<details>
<summary>Does it handle srcset, and does it keep the 2x / 640w descriptors?</summary>

It does, under the default *Common* attribute set. `srcset` is not a single URL but a candidate list — `photo.jpg 1x, retina/photo@2x.jpg 2x` — so each candidate's URL is resolved on its own and its descriptor is copied through untouched, including the spacing and commas between candidates. A naive whole-attribute rewrite either mangles the descriptors or gives up on the attribute entirely.

Two neighbours get the same treatment: `ping` on `<a>` and `<area>` is a whitespace-separated list of URLs, each resolved independently, and `<meta http-equiv="refresh" content="5; url=next.html">` has a URL hidden inside a directive, so only the part after `url=` is rewritten while the delay and formatting stay as they were. `<picture>` works because its `<source>` elements carry `srcset` too.

</details>

<details>
<summary>I'm converting a newsletter — which options should I turn on?</summary>

For HTML email, three defaults are worth changing. Turn on **Also absolutize bare #anchor links**: in an email client a bare `#unsubscribe` resolves against nothing useful, so it must point back at the hosted version. Turn on **Also rewrite `url(…)` and `@import` in CSS**, because inline `style="background:url(img/hero.jpg)"` is common in email templates and a relative path there will not load. And leave **Protocol-relative URLs** on *resolve*, since some mail clients refuse to load a scheme-less `//cdn…` reference.

Leave the attribute set on *Common* — it already covers the `background` attributes that email templates lean on. If the markup comes out of a template engine, note that `{{ … }}` and `<% … %>` placeholders are deliberately skipped, so you can run this on the template rather than only on rendered output.

</details>

<details>
<summary>My document has a &lt;base href&gt; tag — which base wins?</summary>

The document's, by default — that is what a browser does. If the markup contains `<base href="/assets/">` and you supply `https://example.com/blog/post.html`, the `<base>` is first resolved against your base (giving `https://example.com/assets/`) and every other relative URL is resolved against *that*. It is the only answer that reproduces how the page actually rendered.

The `<base href>` value itself is still made absolute, so the output stands on its own no matter where it is served from. If you would rather ignore the tag — common when you are re-hosting fragments and the old `<base>` is stale — untick **Honour a `<base href>`** and everything resolves against the base you typed. The report output tells you which happened: `base_tag_used` is `yes` or `no`, and `effective_base` shows the base that was actually applied.

</details>

<details>
<summary>Can it go the other way and make absolute URLs relative?</summary>

No — this tool only resolves in the relative → absolute direction, and that is a deliberate limit rather than an oversight. Going the other way is not a pure transformation: it needs a decision per URL about whether to emit a root-relative path, a protocol-relative reference or a dot-segment path, and about which cross-origin links should be left absolute. Guessing that in bulk is how internal links quietly break.

What you can do here is check the result before committing to it. Run the dry run to see every URL and its verdict, or the report to confirm the effective base and the rewritten/kept counts, then run the HTML output through a diff — because nothing but the URL values changed, that diff shows exactly the links you touched and nothing else.

</details>
