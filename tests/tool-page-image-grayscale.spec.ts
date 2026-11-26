import { test, expect } from './fixtures';
import * as path from 'path';
import { ALPHA_SQUARE, alphaReport } from './alpha-square';

test('image-grayscale page', async ({ page }) => {
  await page.goto('/tools/image-grayscale/');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90000 });
  expect(await media.getAttribute('src')).toMatch(/^data:image\//);
});

// A transparent PNG must come back gray + alpha, not flattened: a bare
// `format=gray` dropped the alpha plane, so the green stored under the
// transparent background surfaced as an opaque gray square.
test('image-grayscale keeps transparency of a transparent PNG', async ({ page }) => {
  await page.goto('/tools/image-grayscale/');
  await page.setInputFiles('#in-file', ALPHA_SQUARE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90000 });
  const src = (await media.getAttribute('src'))!;
  expect(src).toMatch(/^data:image\/png/);
  const r = await alphaReport(page, src);
  expect(r.corner[3]).toBe(0);
  expect(r.mismatches).toBe(0);
  // The opaque square is still converted: red (200,30,30) -> luma ~81.
  const [cr, cg, cb] = r.center;
  expect(cr).toBe(cg);
  expect(cg).toBe(cb);
  expect(cr).toBeGreaterThan(70);
  expect(cr).toBeLessThan(92);
});
