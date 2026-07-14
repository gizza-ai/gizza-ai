## Remove Audio Tags Without Re-Encoding

Drop privacy-sensitive tags from MP3, FLAC, OGG, M4A, WAV, and other audio files
directly in your browser. The tool asks ffmpeg to stream-copy the audio, remove
global and stream metadata, drop chapters, and remove embedded cover art by default.
Nothing is uploaded to a server.

## Worked Example

Choose `song.mp3`, leave **Cover art** set to `remove`, and download the cleaned
MP3. The audio frames are copied through unchanged while ID3 tags, comments,
chapter markers, encoder metadata, and the album-art stream are removed. If you
want to keep the picture but remove text tags, set **Cover art** to `keep`.

Command-line equivalent:

`gizza tool audio-metadata-stripper 'url=https://example.com/song.mp3' 'cover_art=remove' --out song-clean.mp3`

## Limits and Edge Cases

- The browser page processes the file locally with ffmpeg.wasm; very large audio
  files may exceed memory limits.
- The tool removes metadata but does not edit or rewrite specific fields. Use a
  tag editor if you want to change artist/album/title values.
- Stream-copy preserves the existing codec/container. If the input container is
  unusual or damaged, ffmpeg may reject it rather than re-encode it.

## FAQ

<details>
<summary>Will this change the sound quality?</summary>

No. The ffmpeg plan uses stream copy (`-c copy`), so the encoded audio packets are
copied into a clean container instead of being decoded and encoded again.

</details>

<details>
<summary>Does it remove album artwork?</summary>

Yes by default. Set **Cover art** to `keep` if you want the embedded picture to
remain while text tags and chapters are stripped.

</details>

<details>
<summary>Which metadata is removed?</summary>

The tool drops container and stream tags such as ID3, Vorbis comments, RIFF/ASF
INFO fields, chapter markers, and ffmpeg muxer metadata where the container allows
it.

</details>

<details>
<summary>Is my audio uploaded?</summary>

No. The standalone page runs ffmpeg in WebAssembly inside your browser tab. The
CLI/chat tool resolves its own provided URL or attachment reference, but the page
does not upload your file anywhere.

</details>
