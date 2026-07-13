import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-eq-adjust/ page adjusts an uploaded video's
// brightness / contrast / saturation / gamma in one ffmpeg `eq` pass in-browser
// (@ffmpeg/core from jsDelivr — needs network) and re-encodes to H.264 + AAC.
// Output keeps the mp4 container, so the media src is a data:video/mp4 URL. The
// wasm `build_argv` is pure and shared with the chat block via core, so we also
// assert the argv/out-name plan directly the same way the nearby ffmpeg page
// specs do.
const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(
  page,
  brightness: number,
  contrast: number,
  saturation: number,
  gamma: number,
  inName: string,
) {
  return await page.evaluate(
    async ({ brightness, contrast, saturation, gamma, inName }) => {
      const mod = await import('/tools/video-eq-adjust/gizza_ai_video_eq_adjust_web.js');
      await mod.default('/tools/video-eq-adjust/gizza_ai_video_eq_adjust_web_bg.wasm');
      return mod.build_argv(brightness, contrast, saturation, gamma, inName);
    },
    { brightness, contrast, saturation, gamma, inName },
  );
}

async function expectPlayableVideoDataUrl(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  const frame = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-eq-adjust output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-eq-adjust page brightens a clip with the sliders', async ({ page }) => {
  await page.goto('/tools/video-eq-adjust/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-brightness', '0.15');
  await page.fill('#in-contrast', '1.2');
  await page.fill('#in-saturation', '1.4');
  await page.fill('#in-gamma', '0.9');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-eq-adjust page honors ?brightness=&contrast=&saturation=&gamma= deep link', async ({ page }) => {
  await page.goto('/tools/video-eq-adjust/?brightness=0.05&contrast=0.85&saturation=0&gamma=1.05');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-brightness')).toHaveValue('0.05');
  await expect(page.locator('#in-contrast')).toHaveValue('0.85');
  await expect(page.locator('#in-saturation')).toHaveValue('0');
  await expect(page.locator('#in-gamma')).toHaveValue('1.05');

  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-eq-adjust wasm build_argv builds the expected eq plan and validates ranges', async ({ page }) => {
  await page.goto('/tools/video-eq-adjust/');
  await page.waitForSelector('#in-file');

  // Worked example → single eq pass, H.264 + AAC re-encode, mp4 container kept.
  const plan = await buildArgv(page, 0.1, 1.2, 1.4, 0.9, 'in.mp4');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv[plan.argv.indexOf('-vf') + 1]).toBe(
    'eq=brightness=0.1:contrast=1.2:saturation=1.4:gamma=0.9',
  );
  expect(plan.argv).toContain('libx264');
  expect(plan.argv).toContain('aac');

  // Identity (defaults) leaves every term at its no-op value.
  const identity = await buildArgv(page, 0, 1, 1, 1, 'in.mp4');
  expect(identity.argv[identity.argv.indexOf('-vf') + 1]).toBe(
    'eq=brightness=0:contrast=1:saturation=1:gamma=1',
  );

  // webm can't hold H.264/AAC → output switches to mp4.
  const webm = await buildArgv(page, 0, 1, 0, 1, 'clip.webm');
  expect(webm.out_name).toBe('out.mp4');

  // Out-of-range values are rejected by the shared plan() (brightness > 1).
  await expect(buildArgv(page, 2, 1, 1, 1, 'in.mp4')).rejects.toThrow(/brightness/);
});
