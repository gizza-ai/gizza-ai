## About this tool

Levels adjustment is the classic black-point, white-point, and gamma transfer
curve used in photo editors. This tool applies that same deterministic math to a
list of numeric samples rather than to a raster image. Paste tone values, luma
samples, sensor readings, or any other finite numbers, then choose the input
range that should become the output range.

The formula is:

1. Normalize: `(value - input_black) / (input_white - input_black)`.
2. If **Clamp** is on, clamp that normalized value to `[0, 1]`.
3. Apply midtone gamma as `normalized^(1 / gamma)`.
4. Remap to `output_black + gamma_value * (output_white - output_black)`.

### Worked example

Input values:

```text
64, 128, 192
```

With `input_black=64`, `input_white=192`, `gamma=1`, `output_black=0`, and
`output_white=255`, the output is:

```text
levels: input 64–192, gamma 1, output 0–255

0
127.5
255
```

Use gamma above `1` to brighten midtones, below `1` to darken midtones, and swap
`output_black` / `output_white` to invert values. This is a numeric transfer
function; it does not inspect image pixels, draw histograms, or auto-pick black
and white points.

## FAQ

<details>
<summary>Is this an image editor?</summary>

No. It implements the Levels transfer math for numeric lists. If you already
have pixel channel values or measurements, paste them here. Raster-image features
such as histograms, eyedroppers, and per-channel previews are outside this pure
numeric model.

</details>

<details>
<summary>What does gamma do?</summary>

Gamma controls the midtones after the input range is normalized. Values greater
than `1` brighten midtones; values between `0` and `1` darken them. The black and
white endpoints stay fixed when clamping is enabled.

</details>

<details>
<summary>What happens outside the input black and white points?</summary>

With **Clamp** enabled, values below the input black point become the output
black level, and values above the input white point become the output white
level. Disable **Clamp** to extrapolate beyond those endpoints instead.

</details>

<details>
<summary>Can I invert the values?</summary>

Yes. Set `output_black` higher than `output_white`, for example `255` and `0`.
The input black point then maps to `255`, input white maps to `0`, and the values
between them are inverted along the same gamma curve.

</details>
