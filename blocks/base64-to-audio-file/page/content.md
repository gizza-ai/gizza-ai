## About this tool

Audio travels through APIs, JSON payloads, databases and HTML attributes as Base64 text.
This tool turns that text back into the file it came from: paste the Base64 (or the whole
`data:audio/…;base64,…` URI), and the decoded bytes come back as a `data:` URL you can play,
save, or paste into the address bar.

Nothing is re-encoded. The bytes that come out are byte-for-byte the bytes that were encoded —
the work is the tolerant cleanup of the Base64 text plus reading the decoded file's magic
header so the result gets the correct MIME type and extension.

### What gets detected

With **Format** left on `auto`, the first bytes of the decoded payload decide the container:

| Container | Header it is recognized by | Result |
| --- | --- | --- |
| WAV | `RIFF` … `WAVE` | `audio/wav` · `.wav` |
| MP3 | an `ID3` tag, or a bare MPEG frame sync | `audio/mpeg` · `.mp3` |
| Ogg (Vorbis/Opus/FLAC) | `OggS` | `audio/ogg` · `.ogg` |
| FLAC (native) | `fLaC` | `audio/flac` · `.flac` |
| MP4 / M4A | `ftyp` at offset 4 | `audio/mp4` · `.m4a` |
| AAC (ADTS) | a `0xFFF…` ADTS sync word | `audio/aac` · `.aac` |
| WebM | the `1A 45 DF A3` EBML marker | `audio/webm` · `.webm` |
| AIFF / AIFC | `FORM` … `AIFF` | `audio/aiff` · `.aiff` |
| AMR | `#!AMR` | `audio/amr` · `.amr` |
| WMA / ASF | the ASF header GUID | `audio/x-ms-wma` · `.wma` |
| MIDI | `MThd` (or a `RIFF` … `RMID`) | `audio/midi` · `.mid` |

### Input the decoder forgives

- Line breaks, spaces and tabs anywhere in the payload.
- A single layer of wrapping `"` or `'` quotes, as pasted out of JSON or a shell command.
- A `data:` URI prefix — its declared MIME type is reported when it disagrees with the bytes,
  but the sniffed bytes win.
- The URL-safe alphabet (`-` and `_` instead of `+` and `/`).
- Missing `=` padding.

### Worked example

Input (truncated):

```
UklGRuwAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YcgAAACAob/W…
```

With **File name** `beep`, **Format** `auto` and strict checking on, the result is a
`data:audio/wav;base64,…` URL. The chat and CLI surfaces return the same bytes as a real
download named `beep.wav`; `gizza tool base64-to-audio-file data=… filename=beep --out beep.wav`
writes the file directly.

### Limits & edge cases

- Payloads that decode to more than **32 MiB** are rejected rather than truncated.
- **Reject bytes that aren't audio** (strict, on by default) applies only when Format is `auto`.
  A payload that decodes cleanly but carries, say, a PNG header is refused with the type it
  actually looks like. Turn it off to save the bytes as `application/octet-stream` instead.
- Choosing a Format explicitly always wins — use it for headerless payloads such as raw AAC or
  a bare PCM dump, where there is no header to sniff. The summary notes when the forced format
  disagrees with what the bytes look like.
- A file name is reduced to a bare stem: directories are stripped, unusual characters become
  `-`, and any extension you type is replaced by the one the resolved format implies.
- Only `data:` URIs with `;base64` are accepted; percent-encoded `data:` URIs are not audio in
  practice and are rejected with a clear message.

## FAQ

<details>
<summary>Does this convert between audio formats, for example Base64 MP3 to WAV?</summary>

No. This tool only reverses the Base64 encoding — the decoded bytes are exactly what was
encoded, so a Base64 MP3 comes back as an MP3. Setting **Format** changes the MIME type and
file extension that get attached, not the audio data itself. To actually re-encode between
codecs, use an audio conversion tool that runs a real encoder.

</details>

<details>
<summary>My Base64 decodes but the tool says it isn't audio. What now?</summary>

The bytes carry no header this tool recognizes. The error names what they look like instead
(a PNG, a PDF, plain text, and so on), which usually means the wrong field was copied. If you
know the payload really is audio but has no container header — raw AAC frames or a headerless
PCM dump — set **Format** to that container explicitly, or turn off **Reject bytes that aren't
audio** to save the bytes as-is.

</details>

<details>
<summary>Can I paste a whole data: URI, or must I strip the prefix first?</summary>

Paste the whole thing. A leading `data:audio/mpeg;base64,` (or any other media type) is
stripped for you. Its declared type is only a hint: if it disagrees with the decoded bytes,
the summary says so and the bytes decide the extension — that mismatch is exactly why a file
saved from a `data:` URI sometimes ends up with the wrong suffix.

</details>

<details>
<summary>Why does my Base64 fail with "invalid Base64" when it looks fine?</summary>

Something in the payload is not a Base64 character. The error names the offending character
and its position, which is usually a stray `<`, a `%`, a `\n` escape that stayed literal, or
HTML entities from a copy out of a rendered page. Whitespace, quotes, the URL-safe `-_`
alphabet and missing `=` padding are all handled already, so anything else that trips the
decoder is genuinely not part of the data.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page runs the decoder as WebAssembly inside your browser, and the command-line tool
runs it locally. The Base64 you paste never leaves the device.

</details>

<details>
<summary>How large a payload can I decode?</summary>

Up to 32 MiB of decoded audio, which is roughly 43 MiB of Base64 text. Larger payloads are
rejected with a clear message rather than silently truncated — a half-decoded audio file is
worse than none.

</details>
