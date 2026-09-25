import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/gif-frame-deduplicator/ page drops near-duplicate frames
// from an uploaded GIF with ffmpeg's mpdecimate (@ffmpeg/core from jsDelivr —
// needs network) and re-encodes the GIF through palettegen/paletteuse. Output is
// a GIF rendered as an image, so the media src is a data:image/gif URL.
const fixture = path.resolve(__dirname, 'fixtures/tiny.gif');

async function buildArgv(page, threshold: number, inName: string) {
  return await page.evaluate(
    async ({ threshold, inName }) => {
      const mod = await import(
        '/tools/gif-frame-deduplicator/gizza_ai_gif_frame_deduplicator_web.js'
      );
      await mod.default(
        '/tools/gif-frame-deduplicator/gizza_ai_gif_frame_deduplicator_web_bg.wasm',
      );
      return mod.build_argv(threshold, inName);
    },
    { threshold, inName },
  );
}

test('gif-frame-deduplicator page deduplicates an uploaded GIF', async ({ page }) => {
  await page.goto('/tools/gif-frame-deduplicator/?threshold=98');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-threshold')).toHaveValue('98');

  await page.setInputFiles('#in-file', fixture);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/gif/);
});

test('gif-frame-deduplicator wasm build_argv maps the threshold and rejects non-GIFs', async ({
  page,
}) => {
  await page.goto('/tools/gif-frame-deduplicator/');
  await page.waitForSelector('#in-file');

  // Default threshold: 16320 × 2% ≈ 326, lo at ffmpeg's 12:5 ratio.
  const dflt = await buildArgv(page, 98, 'in.gif');
  expect(dflt.out_name).toBe('optimized.gif');
  expect(dflt.argv[dflt.argv.indexOf('-filter_complex') + 1]).toContain(
    'mpdecimate=hi=326:lo=136:frac=0.33',
  );
  expect(dflt.argv[dflt.argv.indexOf('-filter_complex') + 1]).toContain('palettegen');
  expect(dflt.argv[dflt.argv.indexOf('-filter_complex') + 1]).toContain('paletteuse');
  // -fps_mode vfr is what actually drops the marked frames.
  expect(dflt.argv[dflt.argv.indexOf('-fps_mode') + 1]).toBe('vfr');
  expect(dflt.argv[dflt.argv.indexOf('-loop') + 1]).toBe('0');

  // A lower threshold decimates harder; an empty field (0) means "unset".
  const loose = await buildArgv(page, 90, 'in.gif');
  expect(loose.argv[loose.argv.indexOf('-filter_complex') + 1]).toContain('hi=1632:lo=680');
  const unset = await buildArgv(page, 0, 'in.gif');
  expect(unset.argv).toEqual(dflt.argv);

  await expect(buildArgv(page, 98, 'in.png')).rejects.toThrow(/GIF/);
});
