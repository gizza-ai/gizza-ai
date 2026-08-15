import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(
  page,
  filter: string,
  mode: string,
  fieldOrder: string,
  applyTo: string,
  inName: string
) {
  return await page.evaluate(
    async ({ filter, mode, fieldOrder, applyTo, inName }) => {
      const mod = await import('/tools/video-deinterlace/gizza_ai_video_deinterlace_web.js');
      await mod.default('/tools/video-deinterlace/gizza_ai_video_deinterlace_web_bg.wasm');
      return mod.build_argv(filter, mode, fieldOrder, applyTo, inName);
    },
    { filter, mode, fieldOrder, applyTo, inName }
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
      v.addEventListener('error', () => reject(new Error('video-deinterlace output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
}

test('video-deinterlace page runs the default bwdif pass and outputs a decodable progressive video', async ({ page }) => {
  await page.goto('/tools/video-deinterlace/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-filter')).toHaveValue('bwdif');
  await expect(page.locator('#in-mode')).toHaveValue('frame');
  await expect(page.locator('#in-field_order')).toHaveValue('auto');
  await expect(page.locator('#in-apply_to')).toHaveValue('all');
  await page.setInputFiles('#in-file', fixture);

  const frame = await decodeOutput(page);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
});

test('video-deinterlace deep link prefills field mode, forced TFF and flagged-frame processing', async ({ page }) => {
  await page.goto('/tools/video-deinterlace/?filter=yadif&mode=field&field_order=tff&apply_to=flagged');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-filter')).toHaveValue('yadif');
  await expect(page.locator('#in-mode')).toHaveValue('field');
  await expect(page.locator('#in-field_order')).toHaveValue('tff');
  await expect(page.locator('#in-apply_to')).toHaveValue('flagged');
  await page.setInputFiles('#in-file', fixture);

  const frame = await decodeOutput(page);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
});

test('video-deinterlace wasm build_argv covers advertised values and output containers', async ({ page }) => {
  await page.goto('/tools/video-deinterlace/');
  await page.waitForSelector('#in-file');

  const def = await buildArgv(page, '', '', '', '', 'in.mp4');
  expect(def.out_name).toBe('out.mp4');
  expect(def.argv[def.argv.indexOf('-vf') + 1]).toBe('bwdif=mode=send_frame:parity=auto:deint=all');
  expect(def.argv).toContain('libx264');
  expect(def.argv).toContain('copy');
  expect(def.argv[def.argv.indexOf('-field_order') + 1]).toBe('progressive');

  const field = await buildArgv(page, 'bwdif', 'field', 'bff', 'all', 'in.mp4');
  expect(field.argv[field.argv.indexOf('-vf') + 1]).toBe('bwdif=mode=send_field:parity=bff:deint=all');

  const yadif = await buildArgv(page, 'yadif', 'frame', 'tff', 'flagged', 'in.mkv');
  expect(yadif.out_name).toBe('out.mkv');
  expect(yadif.argv[yadif.argv.indexOf('-vf') + 1]).toBe('yadif=mode=send_frame:parity=tff:deint=interlaced');

  const webm = await buildArgv(page, 'bwdif', 'frame', 'auto', 'all', 'clip.webm');
  expect(webm.out_name).toBe('out.mp4');
  expect(webm.argv).toContain('aac');

  await expect(buildArgv(page, 'nnedi', 'frame', 'auto', 'all', 'in.mp4')).rejects.toThrow(/not supported/);
});

test('video-deinterlace page ships useful example presets', async ({ page }) => {
  await page.goto('/tools/video-deinterlace/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await page.click('.tool-example-chip:has-text("Broadcast 1080i")');
  await expect(page.locator('#in-filter')).toHaveValue('bwdif');
  await expect(page.locator('#in-mode')).toHaveValue('field');
  await expect(page.locator('#in-field_order')).toHaveValue('tff');
  await expect(page.locator('#in-apply_to')).toHaveValue('all');
});
