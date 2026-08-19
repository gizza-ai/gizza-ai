## About this tool

Audio Reverse plays an uploaded clip backwards in the browser. Use it for reverse cymbal swells, backwards vocal textures, sound-design transitions, backmasking checks, or quick before/after edits without uploading the file to a server.

Choose one of three output modes:

- `reverse` writes only the backwards clip.
- `forward-reverse` writes the original clip followed by its reversal, creating a palindrome-style effect.
- `reverse-forward` writes the reversal first and then the original, which is useful for risers that swell into the downbeat.

The output can be MP3, WAV, OGG, FLAC, or M4A. MP3 and OGG are encoded at 192 kbps; WAV and FLAC are lossless. Album art/video streams are dropped so the result is a plain audio file.

### Worked example

Upload a cymbal hit, set `mode=reverse-forward`, and keep `format=mp3`. The page first creates the reversed swell, then appends the original cymbal hit so the sound rises into the transient.

For a lossless edit, choose `format=wav` or `format=flac` before uploading. For only part of a song, trim the source clip first with an audio trimming tool and then reverse the shorter segment here.

### Limits and edge cases

- Browser and chat runs are capped at 10 MiB input and output to keep the ffmpeg runtime responsive.
- Combined modes are roughly twice the input duration, so long inputs can also exceed the output cap.
- The tool reverses decoded samples; it does not change pitch, tempo, or sample rate intentionally.
- Attached cover art and video streams are removed from the output.

## FAQ

<details>
<summary>Does reversing audio change the pitch?</summary>

No. The tool reverses sample order with ffmpeg's `areverse` filter. The waveform plays backwards, so attacks and decays swap direction, but the pitch content is not shifted.

</details>

<details>
<summary>What is the difference between forward-reverse and reverse-forward?</summary>

`forward-reverse` plays the original clip first and then the backwards version. `reverse-forward` starts with the backwards version and ends on the original clip, which is the common reverse-cymbal riser shape.

</details>

<details>
<summary>Which output format should I use?</summary>

Use MP3 for a small shareable file. Use WAV or FLAC when you plan to edit the result again and want a lossless file. OGG and M4A are useful when your target player or workflow prefers those containers.

</details>

<details>
<summary>Can I reverse only a section of a longer recording?</summary>

This tool reverses the whole uploaded clip. Trim the audio to the section you need first, then upload the trimmed clip here.

</details>
