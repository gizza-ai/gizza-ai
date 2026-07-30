import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/mp4-to-mov/ page rewraps an uploaded MP4 into a QuickTime
// .mov in-browser via ffmpeg. It is a lossless remux: `-i in.mp4 -map 0 -c copy
// -movflags +faststart out.mov`, no params. There are no query params, so no
// deep-link test applies.

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(page: Page, inName: string) {
  return await page.evaluate(async ({ inName }) => {
    const mod = await import('/tools/mp4-to-mov/gizza_ai_mp4_to_mov_web.js');
    await mod.default('/tools/mp4-to-mov/gizza_ai_mp4_to_mov_web_bg.wasm');
    return mod.build_argv(inName);
  }, { inName });
}

async function inspectMovDataUrl(page: Page, src: string) {
  return await page.evaluate((dataUrl: string) => {
    const [, b64 = ''] = dataUrl.split(',', 2);
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const ascii = (start: number, len: number) => String.fromCharCode(...bytes.slice(start, start + len));
    const boxTypes: string[] = [];
    for (let p = 0; p + 8 <= bytes.length && boxTypes.length < 24;) {
      const size = (bytes[p] << 24) | (bytes[p + 1] << 16) | (bytes[p + 2] << 8) | bytes[p + 3];
      const typ = ascii(p + 4, 4);
      boxTypes.push(typ);
      if (size < 8) break;
      p += size;
    }
    return {
      bytes: bytes.length,
      ftyp: ascii(4, 4),
      majorBrand: ascii(8, 4),
      hasMoov: boxTypes.includes('moov'),
      hasMdat: boxTypes.includes('mdat'),
      hasFaststartLayout: boxTypes.indexOf('moov') >= 0 && boxTypes.indexOf('mdat') >= 0 && boxTypes.indexOf('moov') < boxTypes.indexOf('mdat'),
    };
  }, src);
}

test('mp4-to-mov wasm build_argv builds the exact lossless QuickTime remux plan', async ({ page }) => {
  await page.goto('/tools/mp4-to-mov/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 'in.mp4');
  expect(plan.out_name).toBe('out.mov');
  expect(plan.argv).toEqual(['-i', 'in.mp4', '-map', '0', '-c', 'copy', '-movflags', '+faststart', 'out.mov']);
});

test('mp4-to-mov page remuxes an MP4 to MOV losslessly', async ({ page }) => {
  await page.goto('/tools/mp4-to-mov/');
  await page.waitForSelector('#in-file');

  await page.setInputFiles('#in-file', fixture);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/(quicktime|mp4|)/);

  // Chromium does not reliably decode QuickTime .mov in <video>, so inspect the
  // ISO-BMFF boxes directly: the page produced a real QuickTime container with
  // both metadata and media data, and +faststart placed moov before mdat.
  const mov = await inspectMovDataUrl(page, src!);
  expect(mov.bytes).toBeGreaterThan(1000);
  expect(mov.ftyp).toBe('ftyp');
  expect(mov.majorBrand).toBe('qt  ');
  expect(mov.hasMoov).toBe(true);
  expect(mov.hasMdat).toBe(true);
  expect(mov.hasFaststartLayout).toBe(true);
});
