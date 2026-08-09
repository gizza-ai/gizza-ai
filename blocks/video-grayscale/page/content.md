## Make a video black-and-white locally

Use this tool when you need a quick monochrome version of a clip: a classic
black-and-white cutaway, a sepia social preview, a blue cyanotype treatment, or a
half-faded desaturation pass. The video stays in your browser. The page builds an
ffmpeg `colorchannelmixer` graph from the controls, runs it locally, and returns a
playable video plus a download link.

### Worked example

For a punchy black-and-white clip, choose the **High-contrast B&W** preset or use:

- Gray mix: `bt709`
- Strength: `100`
- Tone: `none`
- Contrast: `1.45`
- Quality: `balanced`
- Keep the audio track: on

For an old-photo look, try the **Sepia** preset. For a faded colour-to-mono
transition, set **Strength** to `50`; the output keeps half of the original colour
while still taking on the selected tone.

### What the controls mean

**Gray mix** picks how red, green and blue become luma. `bt709` is the HD/sRGB
default, `bt601` matches older SD weighting, `average` treats all channels equally,
and the `red`, `green` and `blue` options mimic darkroom colour filters. **Strength**
is a true 0–100 blend between the original and the monochrome result. **Tone** adds
sepia, warm, cool or cyanotype colouring to the gray values in the same filter
pass. **Contrast** appends an `eq=contrast=` stage only when it is not `1`. **Quality**
sets the H.264 CRF/preset trade-off, and **Keep the audio track** can be disabled
for a silent-film output.

### Limits and edge cases

The browser cap is 25 MB so ffmpeg-wasm has enough memory to decode, filter and
re-encode the clip. Resolution and frame rate are preserved; this tool changes
colour only. MP4, MOV, M4V and MKV outputs keep their container when possible.
WebM and less common inputs are written as MP4 because this block always encodes
H.264 video. If you need a specific output container or codec, run a video
transcode/remux tool after this grayscale pass. A black-and-white video is not
automatically much smaller than the original because the picture is still fully
re-encoded.

## FAQ

<details>
<summary>Does this upload my video?</summary>

No. The page runs ffmpeg in your browser tab and creates the result locally. The
CLI can fetch a public URL when you provide one, but the standalone page works
from the file you choose on your machine.

</details>

<details>
<summary>Which gray mix should I pick?</summary>

Use `bt709` for most HD and phone footage. Use `bt601` when matching older SD
video. The red, green and blue channel modes are creative filters: for example,
the red channel often darkens blue skies, while the green channel can brighten
foliage and skin.

</details>

<details>
<summary>Why did my WebM become an MP4?</summary>

The filter output is encoded as H.264 for broad playback. Containers that commonly
hold H.264, such as MP4, MOV, M4V and MKV, are kept; WebM is changed to MP4 so the
codec/container pair remains valid.

</details>

<details>
<summary>Can this make the video smaller?</summary>

Sometimes, depending on the quality tier and the source. Grayscale alone does not
guarantee a smaller file: the same resolution, frame rate and audio are preserved
unless you disable audio, and the picture is re-encoded at the selected quality.

</details>
