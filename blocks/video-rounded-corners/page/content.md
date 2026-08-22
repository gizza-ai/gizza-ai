## About this tool

`video-rounded-corners` turns a rectangular clip into a rounded card. It builds an ffmpeg alpha mask for every frame, so you can export a transparent WebM/MOV for overlays or an MP4 composited over a solid corner background.

### Worked example

Upload `clip.mp4`, choose **Black MP4 for social**, and the generated plan rounds all four corners with a 40 px radius, fills the cut-off corners with black, drops nothing else from the frame, and exports `out.mp4`.

For overlays, choose **Transparent WebM card** instead. That keeps the rounded cut-outs transparent and writes VP9/WebM, which is the most browser-friendly alpha-capable video format.

### Options and limits

- **Corner radius** is pixels by default. Switch **Radius unit** to percent for a resolution-independent radius based on the shorter side; 50% fully rounds the ends.
- **Corners to round** can be all, top, bottom, left, or right.
- **Corner background** accepts `transparent`, CSS color names, or hex colors such as `#111827`. MP4 cannot store transparency, so choose a solid background for MP4.
- **Output format** is WebM/VP9, MP4/H.264, or MOV/ProRes 4444. MOV with alpha is intended for editors and can be large.
- **Quality** maps 1-100 to the encoder CRF for WebM and MP4.
- **Keep audio** maps the original audio track when one exists; silent videos stay silent.
- The input and output caps are 25 MiB each. Browser ffmpeg can be slow on long clips, so trim or resize first for heavy footage.

## FAQ

<details>
<summary>Why does MP4 require a background color?</summary>

Ordinary H.264 MP4 does not preserve an alpha channel in a way browsers and social platforms handle consistently. For MP4 the tool composites the rounded frame over the chosen solid color. Use WebM or MOV when you need true transparent corners.

</details>

<details>
<summary>Which format should I choose?</summary>

Use MP4 with a solid background for social posts and universal playback. Use WebM when the result will be placed over another web background and transparency matters. Use MOV when you need an alpha video for editing software and can tolerate a larger file.

</details>

<details>
<summary>Can I round only the top corners?</summary>

Yes. Set **Corners to round** to top. The same control can round only the bottom, left, or right pair of corners; all four is the default.

</details>

<details>
<summary>Will it change the video size?</summary>

For WebM and MOV it keeps the frame dimensions. For MP4, ffmpeg may crop one row or column from odd-sized inputs because H.264/yuv420p requires even dimensions.

</details>

<details>
<summary>Is my video uploaded?</summary>

No. On the page, ffmpeg runs in your browser. In the CLI, the tool fetches only the URL or reference you provide and processes it locally in the tool runtime.

</details>
