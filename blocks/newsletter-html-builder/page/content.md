## About this tool

Email clients are not browsers. Gmail strips `<style>` blocks in forwarded messages, Outlook renders
with Word's layout engine, and flexbox or grid simply do not exist in most inboxes. This builder
takes a plain list of sections and returns the markup those clients actually understand: nested
`role="presentation"` tables, every declaration **inlined** on the element it styles, an Outlook
ghost-table wrapper in a conditional comment, `mso-table-lspace` resets, and a single mobile media
query that drops the fixed-width card to 100%.

You write content, not markup:

```
heading | What shipped in March
text | Hi {{first_name}}, three things went live this month.
columns | **Faster exports** — 4x quicker. | **New API keys** — scoped and revocable.
button | Read the release notes | https://example.com/changelog
divider
footer | [Unsubscribe]({{unsubscribe_url}})
```

### Worked example

**Input** — one section, defaults everywhere else:

```
button | Read more | https://example.com
```

**Output** (the button row inside the generated document):

```html
<tr>
  <td align="center" style="padding:24px 32px;">
    <table role="presentation" cellpadding="0" cellspacing="0" border="0">
      <tr>
        <td align="center" bgcolor="#2563eb" style="border-radius:6px;background-color:#2563eb;">
          <a href="https://example.com" style="display:inline-block;padding:14px 28px;font-family:-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;font-size:16px;line-height:20px;color:#ffffff; font-weight:600;color:#ffffff;text-decoration:none;border-radius:6px;">Read more</a>
        </td>
      </tr>
    </table>
  </td>
</tr>
```

That is the "bulletproof button" pattern — a background-coloured table cell with a padded
`display:inline-block` link inside it, so the whole rectangle stays clickable in clients that ignore
padding on anchors, and it degrades to a plain coloured cell where `border-radius` is unsupported.

### Section types

| Type | Line | Notes |
| --- | --- | --- |
| `heading` | `heading \| Big title` | 28px, bold |
| `subheading` | `subheading \| Smaller title` | 20px, semi-bold |
| `text` | `text \| A paragraph` | 16px/26px body copy |
| `button` | `button \| Label \| https://…` | bulletproof button, accent-coloured |
| `image` | `image \| https://…png \| Alt \| https://…` | full-bleed, fluid; 4th part is an optional link |
| `columns` | `columns \| Left \| Right` | two 50% cells that stack on mobile |
| `divider` | `divider` | 1px rule |
| `spacer` | `spacer \| 24` | blank vertical space, px |
| `footer` | `footer \| Small print` | 12px, muted, centred |
| `html` | `html \| <p>…</p>` | raw markup, passed through verbatim |

Inside any text you can use `**bold**`, `*italic*`, `[label](https://example.com)` and `\n` for a
line break. Merge tags such as `{{first_name}}`, `{{unsubscribe_url}}` or `*|FNAME|*` pass through
untouched, so the output can be pasted straight into Mailchimp, Klaviyo, Brevo, Postmark or your own
sending code. Lines beginning with `#` are comments.

### Limits and edge cases

- **Max 200 sections** per newsletter; the 201st is an error rather than a truncated document.
- **Content width 320–900px**, default 600 — the width every desktop client renders without a
  horizontal scrollbar. Below that width the card goes fluid.
- **Spacer height is capped at 200px.**
- **Only `https://`, `http://`, `mailto:`, `tel:`, `#anchor` and merge-tag URLs are accepted.** A
  `javascript:` URL is rejected, not silently escaped.
- **Colours must be hex (`#f4f4f5`, 3/4/6/8 digits) or a plain CSS colour name** — anything else is
  rejected so a broken value can't leak into an inline `style` attribute.
- Text is HTML-escaped; the `html` section type is the deliberate escape hatch and is **not**
  sanitised — only put markup you trust there.
- No web fonts, no background images, no `<script>`: all three are ignored or blocked by major
  clients, so the builder does not emit them.

## FAQ

<details>
<summary>Why is all the CSS inline instead of a style block?</summary>

Because several clients delete `<style>` blocks. Gmail's web client strips them when a message is
forwarded, and some corporate filters remove them outright. Anything that must survive — colours,
fonts, padding, widths — is written on the element itself. The `<style>` block is used only for the
things that *cannot* be inlined: the mobile media query and the dark-mode block. If both are
stripped, the email still renders correctly at its fixed width.

</details>

<details>
<summary>Will this render correctly in Outlook?</summary>

Outlook on Windows renders with Word, which ignores `max-width`, `border-radius` and most modern
CSS. The output handles that with a conditional-comment ghost table (`<!--[if mso]>`) that pins the
content to a real fixed-width table, an `o:PixelsPerInch` setting so images are not scaled up, and
`mso-table-lspace`/`mso-table-rspace` resets that remove Word's phantom table gutters. Buttons fall
back to square corners there — that is expected, not a bug.

</details>

<details>
<summary>What is a preheader, and why is it padded with strange characters?</summary>

The preheader is the short line most inboxes show after the subject. It is rendered as a
zero-height, transparent, `mso-hide:all` div at the top of the body. Without padding, the client
would continue pulling in whatever comes next — usually "View in browser" or your first heading —
and append it to the preview. The run of `&#847;&zwnj;&nbsp;&#8199;&#65279;` characters is
invisible filler that consumes the rest of the preview slot.

</details>

<details>
<summary>How does the dark-mode option work?</summary>

With the box ticked, the document gets `color-scheme` / `supported-color-schemes` meta tags and a
`@media (prefers-color-scheme: dark)` block that swaps the page, card, text, muted and rule colours
for dark equivalents. Apple Mail, Outlook on macOS/iOS and several others honour it. Gmail and
Outlook.com apply their own colour inversion regardless — the meta tags make that inversion less
aggressive. Untick the box if you want the light palette everywhere.

</details>

<details>
<summary>Can I use merge tags and personalisation variables?</summary>

Yes. Text is escaped for HTML but `{{first_name}}`, `*|FNAME|*` and similar tags contain no
HTML-special characters, so they survive verbatim. Merge tags are also accepted as URLs — for
example `[Unsubscribe]({{unsubscribe_url}})` — because most sending platforms substitute the real
link at send time.

</details>

<details>
<summary>Do the two-column sections stack on phones?</summary>

Yes. The columns are 50%-width table cells carrying an `sm-stack` class; the media query switches
them to `display:block; width:100%` below your chosen content width. Clients that drop the media
query (older Gmail app versions on some Android builds) show them side by side, which is why each
column is kept narrow enough to remain readable at half width.

</details>

<details>
<summary>How do I preview the result before sending?</summary>

Copy the output, save it as a `.html` file and open it in a browser for the layout, then send
yourself a real test message through your sending platform — a browser cannot reproduce Outlook's
Word engine or Gmail's CSS stripping. Sending one test to a Gmail address, one to Outlook and one
to Apple Mail covers most of the market.

</details>
