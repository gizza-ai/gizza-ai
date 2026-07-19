## Remove embedded subtitle tracks without re-encoding

Many videos ship with one or more **soft subtitle streams** baked into the container —
closed captions, forced foreign-language subtitles, SDH tracks, or leftover tracks from a
download. This tool remuxes the file locally and drops **every** embedded subtitle/caption
stream while copying the video and audio through untouched, so there is no quality loss and
no waiting on a re-encode.

Everything runs in your browser with ffmpeg (WebAssembly). The video never leaves your
device — nothing is uploaded.

### Worked example

Upload a `movie.mkv` that carries an English SDH subtitle track and two forced foreign
subtitle tracks. Leave **Output container** on `keep` and run it. The tool executes the
equivalent of:

```
ffmpeg -i movie.mkv -map 0 -map -0:s -sn -c copy movie-no-subs.mkv
```

The result is a `.mkv` with the same video and audio bitstreams and **zero** subtitle
streams. Switch **Output container** to `mp4` to get `movie-no-subs.mp4` instead (as long as
the remaining streams are MP4-compatible), or `mkv` to force Matroska.

### How it works

`-map 0` selects every stream in the input, then `-map -0:s` negatively unmaps the subtitle
streams, and `-sn` disables subtitle recording as a safeguard. `-c copy` stream-copies the
survivors, so attachments (embedded fonts) and data streams are preserved — only subtitles
are removed, bit-for-bit lossless for what remains.

<details>
<summary>Does this remove hardcoded (burned-in) subtitles?</summary>

No. Hardcoded subtitles are painted directly onto the video pixels, so they are part of the
image and cannot be removed by remuxing. This tool only removes **soft** subtitle streams —
separate tracks stored alongside the video in the container. Removing burned-in text requires
frame-by-frame inpainting, which is out of scope here.

</details>

<details>
<summary>Will the video or audio quality change?</summary>

No. The tool uses `-c copy`, which copies the video and audio streams byte-for-byte without
decoding or re-encoding. Only the subtitle streams are dropped, so the picture and sound are
identical to the source.

</details>

<details>
<summary>Which formats are supported?</summary>

Any container ffmpeg can read and remux — MP4, MKV, MOV, WebM, AVI, and more. Keep the input
container with `keep`, or force `mp4` / `mkv`. Note that MP4 only accepts MP4-compatible
codecs, so if your source uses a codec MP4 can't hold, choose `keep` or `mkv` instead.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. Processing happens entirely in your browser via ffmpeg compiled to WebAssembly. The file
is read locally and the subtitle-free copy is produced on your device — nothing is sent to a
server.

</details>

### Limits

- Only **soft** (stream-based) subtitles are removed; burned-in subtitles cannot be stripped.
- `mp4` output requires MP4-compatible video/audio codecs; use `keep` or `mkv` otherwise.
- Very large files are limited by your browser's available memory.
