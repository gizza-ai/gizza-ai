## Silence one part of a video, keep the rest

Pick a video, type the **start** and **end** of the moment you want to hush, and
get the same clip back with the audio **silenced only between those two times**
— everything before and after keeps its original sound. It's the surgical
version of muting: perfect for killing a swear word, a copyright-flagged song in
the background, a burst of mic noise, or an off-hand comment, without wiping the
whole soundtrack.

The picture is **copied untouched** — the video stream is stream-copied
losslessly (`-c:v copy`), so there is no quality loss and no re-render of the
image. Only the audio is re-encoded, because the samples inside your window are
being zeroed. Everything runs with ffmpeg inside your browser tab, so **nothing
is uploaded**, and the tool is free with no watermark.

### How the times work

Both **Mute from** and **Mute until** are **seconds from the start of the clip**.
They can be whole numbers or decimals:

- `5` and `10` silence the sixth through tenth second.
- `0` and `3` silence the first three seconds.
- `12.5` and `13.2` silence a fraction-of-a-second window — enough to drop a
  single spoken word.

**Mute until** must be greater than **Mute from**; the range in between is
silenced and the audio outside it is passed through unchanged.

### Worked example

You recorded a product demo and someone says a name you'd rather not publish at
around the 12-second mark. Load `demo.mp4`, set **Mute from** to `12` and **Mute
until** to `13.5`, and run it. You get `demo-muted.mp4` — the same video, same
picture, with just that 1.5-second window silenced and the rest of the narration
intact.

On the command line the same job is
`gizza tool video-mute-section url=https://example.com/demo.mp4 start=12 end=13.5 --out demo-muted.mp4`,
and the page URL `/tools/video-mute-section/?start=12&end=13.5` deep-links to
the same pre-filled settings.

### Notes and limits

- **One range per run.** To silence several separate moments, run the tool once
  per range (feed the output of one run into the next), or pick a single window
  that covers them all.
- The **whole audio track** is silenced in that window — this mutes everything
  (voice, music, ambience) between the two times; it can't isolate one voice or
  one instrument (that needs an AI model).
- The video is **copied losslessly**; only the audio is re-encoded. mp4, mov,
  m4v and mkv keep their container and use AAC audio; webm keeps webm and uses
  Opus; other inputs come out as MP4.
- If the video has **no audio track** there is nothing to silence and ffmpeg
  will report an error — use a clip that actually has sound.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.
Nothing is uploaded and nothing is stored.

</details>

<details>
<summary>Does muting a section lower the video quality?</summary>

No. The **picture** is stream-copied (`-c:v copy`), so it is byte-for-byte
identical to the source — there is no image re-encode and no quality loss. Only
the **audio** is re-encoded, which is unavoidable because the samples inside your
window are being set to silence.

</details>

<details>
<summary>How do I mute more than one part of the video?</summary>

Run the tool once per range. Silence the first window, download the result, then
load that file back in and silence the next window. Because the picture is copied
losslessly each time, repeating the process does not degrade the image. If the
gaps between your ranges are small, it's often easier to pick one wider window
that covers them all.

</details>

<details>
<summary>What times should I enter — seconds or a timecode like 00:01:30?</summary>

Enter **seconds from the start of the clip** as plain numbers — `90` for one
minute thirty, or `12.5` for twelve and a half seconds. Decimals are allowed, so
you can trim a window down to a fraction of a second to drop a single word.

</details>

<details>
<summary>Can it mute just the music, or just one person's voice?</summary>

No. This silences the **entire** soundtrack inside your time window — voice,
music and ambience together. Separating one voice or one instrument from a mixed
track needs an AI source-separation model, which is out of scope here.

</details>

<details>
<summary>Which formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, m4v, mkv and webm are the common cases.
mp4/mov/m4v/mkv keep their container (AAC audio); webm keeps webm (Opus audio);
other inputs come out as MP4. The input and output are each capped at 25 MB.

</details>
