import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/heic-to-jpg/ page converts an uploaded HEIC/HEIF photo to
// JPEG/PNG in-browser via ffmpeg (single-threaded @ffmpeg/core from jsDelivr —
// needs network). The conversion auto-runs on file `change`.
//
// IMPORTANT — DRAFT-BROKEN: the bundled @ffmpeg/core (0.12.10 = ffmpeg 5.1.4) is
// built WITHOUT libheif and predates ffmpeg's native HEIF demuxer (7.0), so it
// CANNOT decode HEIC — ffmpeg aborts with "moov atom not found". The native
// `ffmpeg` 6.1.1 used by the chat/CLI path also lacks HEIF support. So this spec
// asserts the CURRENT, real behavior: the page renders + accepts the file but the
// decode fails and the error is surfaced in #tool-output. When a HEIF-enabled
// ffmpeg core ships, flip this to assert #tool-output-media becomes visible with
// a `data:image/` src (the image-resize spec is the template for that).
test('heic-to-jpg page renders inputs and surfaces the ffmpeg HEIF-decode failure', async ({ page }) => {
  await page.goto('/tools/heic-to-jpg/');
  await page.waitForSelector('#in-file');

  // Both inputs are wired: the HEIC file input and the format field.
  await expect(page.locator('#in-file')).toHaveAttribute('accept', /image\/heic/);
  await expect(page.locator('#in-format')).toBeVisible();

  // Uploading the file auto-runs the conversion (file `change` triggers run()).
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/sample.heic'));

  // ffmpeg loads from CDN on first run; allow generous time. The bundled core
  // can't decode HEIC, so the page surfaces a failure rather than output media.
  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 90_000 });
  await expect(page.locator('#tool-output-media')).toBeHidden();
});
