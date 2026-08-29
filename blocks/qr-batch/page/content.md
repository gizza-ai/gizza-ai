## About this tool

Use this batch QR generator when a spreadsheet or inventory list needs one scannable code per row. Paste a plain list, CSV, or TSV; choose how columns map to filenames and payloads; then download a single ZIP containing PNG files, SVG files, or both. The archive can include `index.csv` so each generated filename can be audited against the value that was encoded.

A worked example: paste `homepage,https://example.com` and `support,mailto:support@example.com`, keep `input_format=csv`, `columns=name-value`, and download `homepage.png`, `support.png`, and `index.csv` in one archive. Rows with bad data are reported in the index instead of being silently skipped.

The tool is intended for static QR codes such as links, mailto links, ticket IDs, asset tags, Wi-Fi provisioning strings, or plain text labels. It runs locally in the browser and has guardrails for the current WebAssembly sandbox: 500 rows per batch, 4,096 characters per pasted row before QR-capacity checks, 64-2048 px output size, and a 32 MiB ZIP output cap.

## FAQ

<details>
<summary>Can I use a CSV where the first column is the filename?</summary>

Yes. Set `input_format` to `csv` and `columns` to `name-value`. A row such as `front-door,https://example.com/door/front` becomes `front-door.png` (or `.svg`) and the URL is what scanners read.

</details>

<details>
<summary>What happens if one row is too long for a QR code?</summary>

The rest of the batch still runs as long as at least one row is valid. The failing row is listed in `index.csv` with its row number and reason, so you can fix only that row and rerun the batch.

</details>

<details>
<summary>Should I choose PNG or SVG?</summary>

Use PNG when another system expects raster images or thumbnails. Use SVG for print workflows that may scale the code. Choose `both` when the archive is shared with people who need different formats.

</details>

<details>
<summary>Can this make dynamic QR codes with analytics?</summary>

No. It creates static QR images that encode the exact text in each row. Dynamic links, scan analytics, short-link hosting, and expiry dates require a server-side redirect service and are outside this local tool.

</details>
