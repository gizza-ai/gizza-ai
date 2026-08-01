## Google Drive link converter

Google Drive share links open a preview page — not the file. Paste any Drive
link here and get the link you actually need, generated locally in your browser.
The tool reads the **file ID** out of the link and rebuilds it in the form you
pick; nothing is uploaded and Google is never contacted.

### What you can convert to

- **Direct download link** — `https://drive.google.com/uc?export=download&id=FILE_ID`.
  Starts the download straight away instead of the preview page. Best for files
  under Google's ~100&nbsp;MB virus-scan threshold.
- **Direct download — large files** — `https://drive.usercontent.google.com/download?id=FILE_ID&export=download&confirm=t`.
  For big files Google shows a "can't scan for viruses" page; the `confirm=t`
  form skips it so `wget`/`curl` and download managers get the bytes.
- **Inline image embed** — `https://drive.google.com/uc?export=view&id=FILE_ID`.
  Drop it into an `<img src="…">` or Markdown `![](…)` to show a Drive-hosted
  image on a page.
- **Preview iframe URL** — `https://drive.google.com/file/d/FILE_ID/preview`.
  Embeds PDFs, video, and docs inside an `<iframe>`.
- **Thumbnail image** — `https://drive.google.com/thumbnail?id=FILE_ID&sz=w1000`.
  A resizable preview image; set the size with `w<width>`, `w<width>-h<height>`,
  or `s<pixels>`.
- **Share / view link** — the reverse conversion: turn a download or `id=` link
  back into `https://drive.google.com/file/d/FILE_ID/view?usp=sharing`.
- **File ID only** — just the raw `FILE_ID`, for scripts and API calls.

### Links it understands

You can paste any of these — the tool finds the file ID in each:

- `https://drive.google.com/file/d/FILE_ID/view?usp=sharing`
- `https://drive.google.com/open?id=FILE_ID`
- `https://drive.google.com/uc?export=download&id=FILE_ID`
- `https://drive.usercontent.google.com/download?id=FILE_ID&export=download`
- `https://drive.google.com/thumbnail?id=FILE_ID&sz=w400`
- `https://docs.google.com/document/d/FILE_ID/edit` (Docs, Sheets, Slides)
- `https://drive.google.com/drive/folders/FOLDER_ID`
- a bare `FILE_ID`

### Worked example

Paste `https://drive.google.com/file/d/1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW/view?usp=sharing`
with **Convert to → Direct download link** and you get:

`https://drive.google.com/uc?export=download&id=1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW`

Switch **Convert to → File ID only** on the same link and you get the bare
`1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW`.

### Tips

- Turn on **batch** to convert a whole list of links at once, one per line —
  each is converted with the same output type and blank lines are kept.
- The **Thumbnail size** box only affects the *Thumbnail* output; a larger `sz`
  (e.g. `w1600`) avoids cropping.

### FAQ

<details>
<summary>Is my link sent anywhere?</summary>

No. The converter is compiled to WebAssembly and runs entirely in your browser
tab — it only rearranges the text of the link and never contacts Google or any
server.

</details>

<details>
<summary>Why does my direct link show a "Google Drive can't scan this file for viruses" warning?</summary>

For files bigger than roughly 100&nbsp;MB, Google can't virus-scan them and shows
an interstitial page instead of the file. Choose **Direct download — large
files** to get the `drive.usercontent.google.com/download?id=…&export=download&confirm=t`
form, which skips that page for most downloads. Very large files may still ask
for a one-time confirmation token that no static URL can include.

</details>

<details>
<summary>How do I embed a Google Drive image in HTML or Markdown?</summary>

Convert the share link to **Inline image embed** to get a
`uc?export=view&id=FILE_ID` URL, then use it as `<img src="…">` in HTML or
`![alt](…)` in Markdown. For a smaller, resizable copy use **Thumbnail** with a
size like `w800`. The file's sharing must be set to *Anyone with the link*.

</details>

<details>
<summary>The file still won't download or embed — what's wrong?</summary>

The most common cause is permissions: in Drive open **Share → General access**
and set it to *Anyone with the link*. A restricted file needs a sign-in, so no
direct link can reach it. Also check that the ID is correct — this tool builds a
well-formed link from whatever ID it finds, but it can't verify the file exists.

</details>

<details>
<summary>Can I turn a native Google Doc, Sheet, or Slide into a download link?</summary>

Those aren't stored as files, so `uc?export=download` doesn't apply — Drive
exports them on the fly. Use Google's export endpoint instead, e.g.
`https://docs.google.com/document/d/FILE_ID/export?format=pdf` (or `docx`,
`xlsx`, `pptx`). This tool still extracts the **File ID** from a Docs link so you
can build that URL. Uploaded files (PDFs, images, zips, videos) work with the
direct-download options directly.

</details>

<details>
<summary>Can I convert a whole list of links at once?</summary>

Yes — switch on **Convert each line separately (batch)** and paste one Drive
link per line. Each non-empty line is converted to the chosen output type and
blank lines are preserved, so the result stays aligned with your input.

</details>
