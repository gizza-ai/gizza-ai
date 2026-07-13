## About this tool

Voice Note Converter handles the audio format most chat apps use for recorded messages: Opus audio in an Ogg container, usually saved as `.opus` or `.ogg`. Drop in a WhatsApp, Telegram, Signal or browser-recorded voice note and convert it to **MP3** for broad playback or **WAV** for editing. You can also go the other way: upload an MP3/WAV clip and create a compact `.opus` voice note tuned for speech.

Everything runs through ffmpeg in your browser. The audio file stays local to your device; it is not uploaded to gizza.

## Worked examples

- **Voice note to MP3:** choose a `.opus` or `.ogg` voice message, set **Target format** to `MP3`, leave **Mono / voice-tuned** on, and use `128` kbps.
- **MP3 back to a chat-style voice note:** choose an `.mp3`, set **Target format** to `Opus voice note (.opus)`, use `24` or `32` kbps, and keep mono on.
- **Editing workflow:** convert a voice note to `WAV`, edit it in an audio editor, then convert the edited result back to `Opus`.

CLI example:

```bash
gizza tool voice-note-converter url=https://example.com/voice.ogg format=mp3 bitrate=128 mono=true --out voice.mp3
```

## Options and limits

| Option | What it does |
| --- | --- |
| **Target format** | `opus` creates an Ogg/Opus voice note, `mp3` plays almost everywhere, `wav` is uncompressed for editing. |
| **Bitrate** | Used by Opus and MP3. Opus is clamped to 6–256 kbps; MP3 is clamped to 32–320 kbps. Leave blank for defaults (`32` for Opus, `128` for MP3). |
| **Mono / voice-tuned** | Downmixes to one channel and uses Opus' `voip` tuning, matching normal chat voice-note expectations. Turn it off for stereo/music clips. |

Input and output are capped at 10 MiB. Batch conversion is not part of this single-file tool; run it once per voice note.

## FAQ

<details>
<summary>Is an OGG file always a voice note?</summary>

No. Ogg is a container. Chat voice notes normally use the **Opus** codec inside Ogg, while some generic converters create Ogg/Vorbis audio. This tool's `opus` target uses `libopus` and writes a `.opus` file intended for voice-note workflows.

</details>

<details>
<summary>What bitrate should I use for Opus voice messages?</summary>

For speech, `24`–`32` kbps is usually compact and clear. Use `48`–`64` kbps if the recording is noisy or important. For music or stereo clips, choose the Music-quality Opus preset (`96` kbps and mono off).

</details>

<details>
<summary>Can converting to MP3 or WAV improve a low-quality voice note?</summary>

No. Conversion changes the container/codec for compatibility or editing, but it cannot restore detail that was already lost in the original Opus recording. WAV is useful for editing because it avoids an extra lossy encode during your edit step.

</details>

<details>
<summary>Does the file leave my browser?</summary>

On the web page, no: ffmpeg runs locally in the browser and the converted audio is produced on your device. The CLI/chat surfaces may fetch a URL you provide, but the conversion itself still runs in the gizza tool runtime.

</details>
