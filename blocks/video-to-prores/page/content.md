## About this tool

Use this when you need a real editing intermediate instead of a small delivery file. The output is always a QuickTime `.mov` encoded as Apple ProRes 422-family video with 10-bit 4:2:2 pixels and an editor-friendly vendor tag. That makes the file easy for NLEs to scrub, trim, grade, and re-export.

A typical default run takes a camera or phone clip, chooses **422 — default general editing tier**, leaves **Resolution cap** at **Source size**, and writes 16-bit PCM audio. For example, a 128×128 MP4 test clip becomes a `.mov` whose video stream identifies as `apcn` (plain ProRes 422), uses `yuv422p10le`, and keeps uncompressed PCM audio.

ProRes is intentionally large. Plain ProRes 422 is roughly 147 Mbps at 1080p29.97 — about 18 MB per second before audio. Choose **Proxy** or a lower **Resolution cap** for offline edits, quick review files, or anything long. The page runs locally in the browser; the CLI/chat path caps input at 32 MiB and output at 128 MiB to avoid surprise memory use.

## Limits and edge cases

- This is for ProRes **422** (`proxy`, `lt`, `standard`, `hq`). It does not create ProRes 4444, 4444 XQ, ProRes RAW, alpha-channel intermediates, or web-delivery H.264/WebM files.
- **Resolution cap** only downscales. Selecting 1080p for a 720p source leaves it at 720p instead of inventing pixels.
- Browser previews usually cannot play ProRes. Download the `.mov` and open it in an editor, media inspector, or `ffprobe`.
- Converting to ProRes does not restore detail, dynamic range, or color information that was not present in the source.

## FAQ

<details>
<summary>Which ProRes tier should I pick?</summary>

Use **422** for general editing. Pick **Proxy** when you want small offline files, **LT** when storage matters but you still want a nicer intermediate, and **HQ** when the clip will be graded heavily or re-encoded several times.

</details>

<details>
<summary>Why is the MOV much larger than my MP4?</summary>

ProRes is an intra-frame editing codec: every frame carries enough information to be decoded and scrubbed independently. That makes timeline work smooth, but it trades compression efficiency for editability. Use the Proxy tier or a lower resolution cap if size matters.

</details>

<details>
<summary>Can browsers play the output directly?</summary>

Usually no. Most browsers do not decode ProRes even though the file is a standard `.mov`. The page provides a download; open the result in Final Cut Pro, Premiere Pro, DaVinci Resolve, Avid, QuickTime-compatible tools, or inspect it with `ffprobe`.

</details>

<details>
<summary>Does this improve the visual quality of my source video?</summary>

No. ProRes preserves quality well during editing and repeated exports, but it cannot recover detail lost to a compressed source. It is best used before an editing or grading step, not as a magic quality enhancer.

</details>
