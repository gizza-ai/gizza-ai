import { test, expect, readResult } from './fixtures';
import * as path from 'path';
import { ALPHA_SQUARE, ALPHA_SQUARE_GIF, ALPHA_SQUARE_PAL, alphaReport, resultUrl } from './alpha-square';

test('image-grayscale page', async ({ page }) => {
  await page.goto('/tools/image-grayscale/');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90000 });
  expect((await readResult(page, await media.getAttribute('src'))).mime).toMatch(/^image\//);
});

// A transparent image must come back gray + alpha, not flattened: a bare
// `format=gray` dropped the alpha plane (the green stored under the
// transparent background surfaced as an opaque gray square), and a
// `ya8|gray` format list still picked `gray` for a palette PNG with tRNS.
// A GIF keeps its transparent index instead of ffmpeg's opaque bgr8 palette.
for (const [name, fixture, mime] of [
  ['RGBA PNG', ALPHA_SQUARE, 'image/png'],
  ['palette PNG', ALPHA_SQUARE_PAL, 'image/png'],
  ['GIF', ALPHA_SQUARE_GIF, 'image/gif'],
] as const) {
  test(`image-grayscale keeps transparency of a transparent ${name}`, async ({ page }) => {
    await page.goto('/tools/image-grayscale/');
    await page.setInputFiles('#in-file', fixture);
    const r = await alphaReport(page, await resultUrl(page));
    expect(r.mime).toBe(mime);
    expect(r.corner[3]).toBe(0);
    expect(r.mismatches).toBe(0);
    // The opaque square is still converted: red (200,30,30) -> luma ~81.
    const [cr, cg, cb] = r.center;
    expect(cr).toBe(cg);
    expect(cg).toBe(cb);
    expect(cr).toBeGreaterThan(70);
    expect(cr).toBeLessThan(92);
  });
}
