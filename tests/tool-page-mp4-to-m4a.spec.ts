import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');

async function buildArgv(page, inName: string) {
  return await page.evaluate(async ({ inName }) => {
    const mod = await import('/tools/mp4-to-m4a/gizza_ai_mp4_to_m4a_web.js');
    await mod.default('/tools/mp4-to-m4a/gizza_ai_mp4_to_m4a_web_bg.wasm');
    return mod.build_argv(inName);
  }, { inName });
}

async function decodeAudio(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const a = document.createElement('audio');
    a.preload = 'metadata';
    await new Promise((resolve, reject) => {
      a.addEventListener('loadedmetadata', resolve, { once: true });
      a.addEventListener('error', () => reject(new Error('mp4-to-m4a output failed to decode')), { once: true });
      a.src = dataUrl;
    });
    return { duration: a.duration };
  }, src);
}

test('mp4-to-m4a wasm build_argv builds the exact lossless audio remux plan', async ({ page }) => {
  await page.goto('/tools/mp4-to-m4a/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 'in.mp4');
  expect(plan.out_name).toBe('out.m4a');
  expect(plan.argv).toEqual(['-i', 'in.mp4', '-vn', '-map', '0:a:0', '-c:a', 'copy', 'out.m4a']);
});

test('mp4-to-m4a page extracts an audio track to playable M4A', async ({ page }) => {
  await page.goto('/tools/mp4-to-m4a/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', fixture);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/(mp4|x-m4a|mpeg|)/);

  const meta = await decodeAudio(page, src!);
  expect(meta.duration).toBeGreaterThan(0);
});
