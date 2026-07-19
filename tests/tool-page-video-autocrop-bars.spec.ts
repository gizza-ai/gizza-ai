import { test, expect } from './fixtures';
import path from 'node:path';

// The /tools/video-autocrop-bars/ page is a TWO-pass ffmpeg flow owned by
// page/custom.js: pass 1 runs cropdetect (log only), the shared core decides
// crop / no-bars / error, pass 2 applies the crop. ffmpeg.wasm loads from
// jsDelivr — needs network.
//
// Fixtures (generated with ffmpeg, bounds PRE-MEASURED on these exact files):
//   letterbox-320x240.mp4  — 320x180 picture + 30px black bars top/bottom
//       round 2/4 → crop=320:180:0:30, round 8/16 → crop=320:176:0:32
//   pillarbox-320x180.mp4  — 240x180 picture + 40px black bars left/right
//       round 2/4 → crop=240:180:40:0, round 8/16 → crop=240:176:40:2
//   letterbox-320x240.mov  — same letterbox in a MOV container (kept: out.mov)
//   tiny-128x128.mp4       — full-frame content, no bars.

const LETTERBOX = path.resolve(__dirname, 'fixtures/letterbox-320x240.mp4');
const PILLARBOX = path.resolve(__dirname, 'fixtures/pillarbox-320x180.mp4');
const LETTERBOX_MOV = path.resolve(__dirname, 'fixtures/letterbox-320x240.mov');
const NOBARS = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

// Decode the produced video: real dimensions + duration prove the crop
// happened. `forceMp4Mime` relabels the data URL for the probe element only —
// Chromium's ISO-BMFF demuxer handles MOV bytes, but may refuse the
// `video/quicktime` MIME on a data: URL.
async function decodeVideo(page, src: string, forceMp4Mime = false) {
  return page.evaluate(async ({ dataUrl, force }: { dataUrl: string; force: boolean }) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = force ? dataUrl.replace(/^data:video\/[^;]+/, 'data:video/mp4') : dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video failed to decode')), { once: true });
    });
    const b64 = dataUrl.split(',')[1] || '';
    return { w: v.videoWidth, h: v.videoHeight, dur: v.duration, bytes: atob(b64).length };
  }, { dataUrl: src, force: forceMp4Mime });
}

async function runFixture(page, url: string, fixture: string, fields: Record<string, string> = {}) {
  await page.goto(url);
  await page.waitForSelector('#in-file');
  for (const [name, value] of Object.entries(fields)) {
    if (name === 'round') await page.selectOption('#in-round', value);
    else await page.fill('#in-' + name, value);
  }
  await page.setInputFiles('#in-file', fixture);
}

test('letterbox bars are detected and cropped (defaults: threshold 24, round 2)', async ({ page }) => {
  await runFixture(page, '/tools/video-autocrop-bars/', LETTERBOX);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  const r = await decodeVideo(page, src!);
  expect(r.w).toBe(320); // 320x240 → 320x180: 30px top+bottom bars removed
  expect(r.h).toBe(180);
  expect(r.dur).toBeGreaterThan(1); // ~2s clip survived the re-encode
  expect(r.bytes).toBeGreaterThan(500);
  await expect(page.locator('#tool-output')).toHaveText(
    'Removed bars: 320×240 → 320×180 (crop offset x=0, y=30).'
  );
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.mp4');
});

test('pillarbox bars, round=4: side bars removed exactly', async ({ page }) => {
  await runFixture(page, '/tools/video-autocrop-bars/', PILLARBOX, { round: '4' });
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const r = await decodeVideo(page, (await media.getAttribute('src'))!);
  expect(r.w).toBe(240); // 320x180 → 240x180: 40px left+right bars removed
  expect(r.h).toBe(180);
});

test('round=8 snaps the cropped height down (320x176)', async ({ page }) => {
  await runFixture(page, '/tools/video-autocrop-bars/', LETTERBOX, { round: '8' });
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const r = await decodeVideo(page, (await media.getAttribute('src'))!);
  expect(r.w).toBe(320); // 180 is not a multiple of 8 → snapped to 176
  expect(r.h).toBe(176);
});

test('round=16 (encoder-friendly) snaps pillarbox crop to 240x176', async ({ page }) => {
  await runFixture(page, '/tools/video-autocrop-bars/', PILLARBOX, { round: '16' });
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const r = await decodeVideo(page, (await media.getAttribute('src'))!);
  expect(r.w).toBe(240); // 240 = 15×16 kept; 180 → 176
  expect(r.h).toBe(176);
});

test('mov input (secondary container) keeps the container: out.mov', async ({ page }) => {
  await runFixture(page, '/tools/video-autocrop-bars/', LETTERBOX_MOV);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/quicktime/); // h264_out_ext keeps mov
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.mov');
  const r = await decodeVideo(page, src!, true); // probe relabeled as mp4 (see helper)
  expect(r.w).toBe(320);
  expect(r.h).toBe(180);
});

test('full-frame video reports "no bars" instead of re-encoding', async ({ page }) => {
  await runFixture(page, '/tools/video-autocrop-bars/', NOBARS);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('No black bars detected', { timeout: 120_000 });
  await expect(out).toContainText('128×128'); // real detected frame size
  await expect(out).not.toHaveClass(/error/); // friendly outcome, not an error
  await expect(page.locator('#tool-output-media')).toBeHidden();
});

test('threshold=0 (min boundary): limited-range black bars are NOT black at 0', async ({ page }) => {
  // Y=16 bars are above a 0 threshold — the strictest setting finds no bars.
  await runFixture(page, '/tools/video-autocrop-bars/', LETTERBOX, { threshold: '0' });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('No black bars detected', { timeout: 120_000 });
  await expect(out).toContainText('320×240');
});

test('threshold=255 (max boundary): whole frame reads as black → clear error', async ({ page }) => {
  await runFixture(page, '/tools/video-autocrop-bars/', LETTERBOX, { threshold: '255' });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('whole frame reads as black', { timeout: 120_000 });
  await expect(out).toHaveClass(/error/);
});

test('deep-link pre-fills params and runs (?threshold=48&round=2)', async ({ page }) => {
  await page.goto('/tools/video-autocrop-bars/?threshold=48&round=2');
  await page.waitForSelector('#in-file');
  // Scalar params are prefilled by the shared driver before custom.setup().
  await expect(page.locator('#in-threshold')).toHaveValue('48');
  await page.setInputFiles('#in-file', LETTERBOX);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const r = await decodeVideo(page, (await media.getAttribute('src'))!);
  expect(r.w).toBe(320); // pure-black bars are still bars at threshold 48
  expect(r.h).toBe(180);
});
