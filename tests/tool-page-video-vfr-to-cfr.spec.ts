import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(
  page,
  fps: string,
  quality: string,
  resyncAudio: string,
  inName: string
) {
  return await page.evaluate(
    async ({ fps, quality, resyncAudio, inName }) => {
      const mod = await import('/tools/video-vfr-to-cfr/gizza_ai_video_vfr_to_cfr_web.js');
      await mod.default('/tools/video-vfr-to-cfr/gizza_ai_video_vfr_to_cfr_web_bg.wasm');
      return mod.build_argv(fps, quality, resyncAudio, inName);
    },
    { fps, quality, resyncAudio, inName }
  );
}

async function decodeOutput(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  expect(src!.length).toBeGreaterThan(8_000);
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-vfr-to-cfr output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
}

test('video-vfr-to-cfr page runs the default auto CFR conversion and outputs decodable video', async ({ page }) => {
  await page.goto('/tools/video-vfr-to-cfr/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-fps')).toHaveValue('auto');
  await expect(page.locator('#in-quality')).toHaveValue('balanced');
  await expect(page.locator('#in-resync_audio')).toBeChecked();
  await page.setInputFiles('#in-file', fixture);

  const frame = await decodeOutput(page);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
});

test('video-vfr-to-cfr deep link prefills 60 fps high quality with audio copy off', async ({ page }) => {
  await page.goto('/tools/video-vfr-to-cfr/?fps=60&quality=high&resync_audio=false');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-fps')).toHaveValue('60');
  await expect(page.locator('#in-quality')).toHaveValue('high');
  await expect(page.locator('#in-resync_audio')).not.toBeChecked();
  await page.setInputFiles('#in-file', fixture);

  const frame = await decodeOutput(page);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
});

test('video-vfr-to-cfr wasm build_argv covers advertised rates, quality presets, checkbox states and containers', async ({ page }) => {
  await page.goto('/tools/video-vfr-to-cfr/');
  await page.waitForSelector('#in-file');

  const def = await buildArgv(page, '', '', '', 'in.mp4');
  expect(def.out_name).toBe('out.mp4');
  expect(def.argv[def.argv.indexOf('-fps_mode') + 1]).toBe('cfr');
  expect(def.argv).not.toContain('-r');
  expect(def.argv[def.argv.indexOf('-crf') + 1]).toBe('20');
  expect(def.argv[def.argv.indexOf('-af') + 1]).toBe('aresample=async=1:first_pts=0');
  expect(def.argv[def.argv.indexOf('-c:a') + 1]).toBe('aac');

  for (const rate of ['23.976', '24', '25', '29.97', '30', '50', '59.94', '60']) {
    const planned = await buildArgv(page, rate, 'balanced', 'true', 'in.mp4');
    expect(planned.argv[planned.argv.indexOf('-r') + 1]).toBe(rate);
  }

  const high = await buildArgv(page, '60', 'high', 'true', 'in.mp4');
  expect(high.argv[high.argv.indexOf('-crf') + 1]).toBe('18');

  const small = await buildArgv(page, '30', 'small', 'true', 'in.mp4');
  expect(small.argv[small.argv.indexOf('-crf') + 1]).toBe('24');

  const audioCopy = await buildArgv(page, '25', 'balanced', 'false', 'clip.mov');
  expect(audioCopy.out_name).toBe('out.mov');
  expect(audioCopy.argv).not.toContain('-af');
  expect(audioCopy.argv[audioCopy.argv.indexOf('-c:a') + 1]).toBe('copy');

  const webm = await buildArgv(page, '30', 'balanced', 'false', 'clip.webm');
  expect(webm.out_name).toBe('out.mp4');
  expect(webm.argv[webm.argv.indexOf('-c:a') + 1]).toBe('aac');

  await expect(buildArgv(page, '42', 'balanced', 'true', 'in.mp4')).rejects.toThrow(/fps must be one of/);
});

test('video-vfr-to-cfr page ships useful example presets', async ({ page }) => {
  await page.goto('/tools/video-vfr-to-cfr/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await page.click('.tool-example-chip:has-text("Lock gameplay")');
  await expect(page.locator('#in-fps')).toHaveValue('60');
  await expect(page.locator('#in-quality')).toHaveValue('high');
  await expect(page.locator('#in-resync_audio')).toBeChecked();
});
