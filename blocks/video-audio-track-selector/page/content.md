## About this tool

Some videos ship with more than one audio track — an original-language mix and a
dub, a commentary track, a described-audio track, or a clean stereo alongside a
5.1 mix. Most players let you *switch* between them, but the extra tracks travel
with the file forever, bloating its size and confusing apps that pick the wrong
one by default.

This tool keeps **exactly one** audio track and drops the rest. You choose the
track by its **0-based index** — `0` is the first audio track, `1` the second,
and so on — and the video plus the chosen audio are copied straight through
(`-c copy`), so there's **no re-encode**: the picture and the kept audio stay
byte-for-byte identical and the export is near-instant. Everything happens in
your browser with ffmpeg compiled to WebAssembly; the file never leaves your
device.

**Worked example.** Say you have `movie.mkv` with two audio tracks — track `0`
is the foreign-language original and track `1` is the English dub. Set **Audio
track to keep** to `1`, leave subtitles off, and you get a video that plays the
English dub everywhere, with the original-language track removed. Not sure which
index is which? Open the file in VLC (Audio → Audio Track) or run
`ffprobe yourfile.mp4` — the audio streams are listed in order, and the first one
shown is index `0`.

Works with MP4, MKV, MOV and WebM. Files up to 25 MB are supported in the
browser; larger files are better handled by the `gizza` CLI. If you pick a track
index the video doesn't have (for example `1` on a file with a single audio
track), the export fails with a clear "matches no streams" error rather than
producing a silent file.

## FAQ

<details>
<summary>How do I find out which audio track number I want?</summary>

Audio tracks are numbered from `0` in the order they appear in the file. The
quickest way to see them is `ffprobe yourvideo.mp4`, which lists each stream —
the first `Audio:` line is index `0`, the next is `1`, and so on. In VLC, the
**Audio → Audio Track** menu lists them top to bottom in the same order. Pick the
position (starting at 0) of the one you want to keep.

</details>

<details>
<summary>Does this re-encode the video or lose quality?</summary>

No. Both the video and the audio track you keep are stream-copied (`-c copy`),
so nothing is re-compressed — the output is byte-for-byte identical in quality to
the source, just without the extra audio tracks. That also makes the export very
fast, since there's no encoding step.

</details>

<details>
<summary>What happens to subtitles and other tracks?</summary>

By default only the video and your one chosen audio track are kept; subtitle,
chapter and data tracks are dropped. Tick **Also keep subtitle tracks** to copy
any embedded subtitles through as well. Subtitle copying is skipped without error
when the file has none.

</details>

<details>
<summary>Can I keep two audio tracks at once?</summary>

No — this tool keeps exactly one audio track by design (that's how it shrinks a
multi-track file down to a single, unambiguous audio stream). If you need to keep
a different single track, just run it again with a different index.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The whole thing runs locally in your browser using ffmpeg compiled to
WebAssembly. Your video is never sent to a server, so it stays private and there's
no upload wait — only the size of your own file matters.

</details>
