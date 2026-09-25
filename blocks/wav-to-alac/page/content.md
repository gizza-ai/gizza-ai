## Convert WAV to Apple Lossless in your browser

Upload a WAV file and encode it to **ALAC — Apple Lossless — inside an `.m4a` container**, the lossless form iOS, iTunes and Apple Music import. The conversion runs locally with ffmpeg compiled to WebAssembly, so your audio is never uploaded to a server.

ALAC has no quality or bitrate setting: the decoded samples are bit-for-bit identical to the WAV source, just packed smaller. The only real choices are *which* PCM gets encoded — bit depth, sample rate, channel count — and whether the source's textual tags ride along. Every selector starts at **Same as source**, which leaves that property untouched, so a plain conversion needs no configuration at all.

The result is always muxed with the moov atom at the front (`-movflags +faststart`), so the file starts playing before it has fully downloaded, and any embedded cover art is dropped so the audio-only `.m4a` mux never fails on it.

### Worked example

Move a studio master onto an iPhone: upload `session-master.wav`, leave **Bit depth**, **Sample rate** and **Channels** at *Same as source*, keep **Keep title/artist/album tags** checked, and download `session-master.m4a`. It decodes to exactly the same audio as the WAV — typically at 50-60% of the size — and imports straight into Apple Music as a lossless track.

Two variations worth knowing:

- **A 24-bit/96 kHz master for a device that only takes CD audio:** set **Bit depth** to `16-bit (CD depth)` and **Sample rate** to `44.1 kHz (CD)`. This one *is* a quality reduction — it is the only combination on this page that is not bit-for-bit lossless.
- **A multi-channel master that stutters on Apple playback:** set **Channels** to `Stereo (2 channels)` to fold it down. Everything else stays untouched.

### Limits and edge cases

- Inputs up to 25 MiB in the chat/CLI block; results up to 60 MiB. Browser conversions are additionally limited by your device's memory. That is roughly two minutes of 16-bit 44.1 kHz stereo WAV — server-side converters that accept hundreds of megabytes can stream to disk, and an in-browser sandbox cannot.
- **Bit depth** offers only 16 and 24 because those are the only two sample formats the ALAC encoder accepts. Leaving it at *Same as source* lets ffmpeg pick the closest match to the input.
- **Sample rate** offers the music/Apple-Music-lossless family (44.1 kHz through 192 kHz). Telephony rates such as 8 kHz are deliberately absent — downsampling an archival lossless target to voice quality is never the intent. Note that resampling is *not* lossless: keep *Same as source* unless a target device needs a specific rate.
- Textual tags are copied when the source exposes them and the `.m4a` container can represent them. This is not a tag editor: existing tags are copied, not edited. Embedded cover art is always dropped.
- The tool handles WAV as its intended input, but ffmpeg will also decode other formats the file picker accepts (AIFF, FLAC, MP3, …). Encoding an already-lossy source to ALAC cannot restore what the lossy encoder discarded — it just stores the lossy result losslessly, in a bigger file.
- DRM-protected input cannot be decoded; ffmpeg's error is surfaced as-is.
- Trimming and batch conversion are out of scope. This page converts one uploaded file per run.

## FAQ

<details>
<summary>Is the conversion really lossless?</summary>

Yes, as long as you leave **Bit depth**, **Sample rate** and **Channels** at *Same as source*. ALAC stores the decoded PCM samples exactly, so decoding the `.m4a` gives back the same samples ffmpeg read from the WAV. Changing bit depth, sample rate or channel count changes the audio itself before it is encoded — the encoding stays lossless, but the audio is no longer identical to the source.

</details>

<details>
<summary>Why ALAC instead of just renaming the file to .m4a?</summary>

An `.m4a` container defaults to AAC, which is lossy. This tool pins the encoder with `-c:a alac`, so the output is genuinely Apple Lossless rather than a lossy file with a lossless-sounding extension. If you want a smaller lossy file instead, that is a different conversion entirely.

</details>

<details>
<summary>What is the difference between ALAC and FLAC?</summary>

Both are lossless and compress to a similar size. FLAC has wider support on desktop and Android; ALAC is what Apple's ecosystem imports natively, which is why it is the right target for iPhone, iPad, iTunes and Apple Music libraries.

</details>

<details>
<summary>Which bit depth and sample rate should I pick?</summary>

*Same as source* for both, in almost every case — it keeps the file bit-for-bit lossless and lets the encoder match the input. Pick `16-bit` and `44.1 kHz` only when a specific device or workflow requires CD audio, and `24-bit` with `88.2`-`192 kHz` only when you are conforming a file to a hi-res target that demands it.

</details>

<details>
<summary>Are my tags preserved?</summary>

Textual tags such as title, artist, album and year are copied into the `.m4a` when ffmpeg can map them. Uncheck **Keep title/artist/album tags** for a clean, tag-free file. Embedded cover art is always dropped, because an attached picture rides as a video stream and can break an audio-only mux.

</details>

<details>
<summary>Can I convert several WAV files at once?</summary>

Not on this page — it converts one uploaded file per run. For a batch, run the command-line version in a shell loop over your files.

</details>
