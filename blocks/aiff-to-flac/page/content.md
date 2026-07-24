## Convert AIFF to FLAC in your browser

Upload an AIFF or AIF file and convert it to FLAC, a lossless compressed audio format. The conversion runs locally with ffmpeg compiled to WebAssembly, so your audio is not uploaded to a server. The output keeps the decoded samples intact and uses `-map_metadata 0` to carry textual tags such as title, artist, album and year when ffmpeg can map them into FLAC Vorbis comments.

### Worked example

Archive a studio bounce: upload `session-mix.aiff`, leave **FLAC compression level** at `5`, and download `session-mix.flac`. The FLAC decodes to the same audio samples as the AIFF source, but it is usually much smaller. For a final archive where encode time matters less, use level `12`; for quick local checks, use level `0`.

### Limits and edge cases

- Input files up to 25 MiB in the chat/CLI block; browser conversions are also limited by your device memory.
- FLAC compression levels are `0` through `12`. Higher levels can make slightly smaller files but take longer to encode. Audio fidelity is identical at every level.
- Textual metadata is preserved where ffmpeg can map it. This is not a tag editor: existing tags are copied, not changed.
- Embedded cover art is dropped because attached pictures ride as a video stream and can break an audio-only FLAC mux.
- The tool handles AIFF/AIF as its intended input, but ffmpeg may also decode other audio formats accepted by the file picker.
- Batch conversion is not supported; run one audio file at a time.

## FAQ

<details>
<summary>Does FLAC change the sound quality?</summary>

No. FLAC is lossless, so decoding the FLAC gives the same PCM samples that ffmpeg decoded from the AIFF source. The compression level only changes how hard the encoder works to shrink the file.

</details>

<details>
<summary>Which compression level should I choose?</summary>

Use `5` for the default balance of speed and size. Use `0` when you want the fastest encode, or `12` when you want the smallest file and do not mind a slower conversion.

</details>

<details>
<summary>Are my AIFF tags preserved?</summary>

Textual tags are copied with ffmpeg's metadata mapping when the source exposes them and FLAC can represent them as Vorbis comments. Cover art and unsupported container data are not preserved.

</details>

<details>
<summary>Can I convert multiple AIFF files at once?</summary>

No. This page converts one uploaded audio file per run. For a batch, run the CLI repeatedly from your shell or process files one at a time in the browser.

</details>
