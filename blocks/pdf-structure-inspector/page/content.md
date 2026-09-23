## About this tool

PDF Structure Inspector reads the bytes of a PDF and reports how the file is assembled: the header version, trailer entries, cross-reference style, object population, declared `/Type` values, dictionary keys, and stream filters and lengths. It is for debugging producer output, checking damaged files, and comparing low-level structure without rendering pages or extracting document text.

Paste the PDF as base64, a plain hex dump, or a `data:application/pdf;base64,…` URL. The report is local to your browser for the page surface. Use the section selector to jump straight to summary, trailer, object, or stream views; use `object_id` for a single object such as `12 0`; use `filter_key` for dictionary keys or types such as `Font`, `Page`, `Resources`, or `/XObject`.

### Worked example

1. Base64-encode a small PDF.
2. Paste the encoded bytes into **PDF bytes**.
3. Choose **Summary only** for a quick count, or **Streams** to see stream dictionaries and `/Filter` chains.
4. Set **Output format** to JSON when you need a scriptable object table.

The tool intentionally does not dump arbitrary stream bodies, decrypt password-protected PDFs, run JavaScript, repair broken files, or render page content. It caps the listed object rows at `max_objects` (1–5000) while still counting the whole file.

## FAQ

<details>
<summary>What input formats can I paste?</summary>

Paste the PDF bytes as base64, as a whitespace-free or whitespace-wrapped hex dump, or as a `data:application/pdf;base64,…` URL. The decoded bytes must contain a `%PDF` header.

</details>

<details>
<summary>Does this show the actual text or images in the PDF?</summary>

No. This is a structural inspector, not a renderer or extractor. It reports object dictionaries, `/Type` values, stream filters, and byte lengths. Use a text or image extraction tool when you need page content.

</details>

<details>
<summary>How do I inspect one PDF object?</summary>

Enter the object number in `object_id`, for example `12`, or include the generation number as `12 0`. You can also filter by a dictionary key or `/Type` value with `filter_key`, such as `Font`, `Page`, or `/XObject`.

</details>

<details>
<summary>Why are some stream lengths or filters missing?</summary>

PDF stream dictionaries may omit a `/Filter`, use a raw stream, or store `/Length` as an indirect reference. The inspector reports the encoded byte count it sees and the declared `/Length` when it can resolve it.

</details>
