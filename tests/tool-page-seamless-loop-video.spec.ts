import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(page, crossfade: number, quality: number, inName: string) {
  return await page.evaluate(async ({ crossfade, quality, inName }) => {
    const mod = await import('/tools/seamless-loop-video/gizza_ai_seamless_loop_video_web.js');
    await mod.default('/tools/seamless-loop-video/gizza_ai_seamless_loop_video_web_bg.wasm');
    return mod.build_argv(crossfade, quality, inName);
  }, { crossfade, quality, inName });
}

async function decodeOutput(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('seamless-loop-video output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
}

test('seamless-loop-video page crossfades a short clip into a decodable mp4', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-crossfade', '0.5');
  await page.fill('#in-quality', '75');
  await page.setInputFiles('#in-file', fixture);
  const clip = await decodeOutput(page);
  expect(clip.w).toBe(128);
  expect(clip.h).toBe(128);
  // Input is 2.0s; output should be roughly input minus the 0.5s overlap.
  expect(clip.duration).toBeGreaterThan(1.0);
  expect(clip.duration).toBeLessThan(1.8);
});

test('seamless-loop-video page honors deep-linked crossfade and quality controls', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/?crossfade=0.3&quality=85');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-crossfade')).toHaveValue('0.3');
  await expect(page.locator('#in-quality')).toHaveValue('85');
  await page.setInputFiles('#in-file', fixture);
  const clip = await decodeOutput(page);
  expect(clip.w).toBe(128);
  expect(clip.h).toBe(128);
  // A shorter 0.3s overlap keeps more of the 2s source than the default test.
  expect(clip.duration).toBeGreaterThan(1.4);
  expect(clip.duration).toBeLessThan(2.0);
});

test('seamless-loop-video wasm build_argv covers defaults, caps and validation', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/');
  await page.waitForSelector('#in-file');

  const def = await buildArgv(page, 0, 0, 'in.mp4');
  expect(def.out_name).toBe('out.mp4');
  const graph = def.argv[def.argv.indexOf('-filter_complex') + 1];
  expect(graph).toContain('trim=start=0.5');
  expect(graph).toContain('trim=end=0.5');
  expect(graph).toContain('fade=t=out:st=0:d=0.5:alpha=1');
  expect(def.argv).toContain('-an');
  expect(def.argv).toContain('libx264');
  expect(def.argv).toContain('yuv420p');

  const min = await buildArgv(page, 0.1, 1, 'in.webm');
  expect(min.out_name).toBe('out.mp4');
  expect(min.argv[min.argv.indexOf('-filter_complex') + 1]).toContain('trim=start=0.1');
  expect(min.argv[min.argv.indexOf('-crf') + 1]).toBe('40');

  const max = await buildArgv(page, 5, 100, 'in.mov');
  expect(max.argv[max.argv.indexOf('-filter_complex') + 1]).toContain('trim=start=5');
  expect(max.argv[max.argv.indexOf('-crf') + 1]).toBe('18');

  await expect(buildArgv(page, 5.1, 75, 'in.mp4')).rejects.toThrow(/crossfade must be <= 5s/);
});
