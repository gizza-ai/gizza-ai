import { test, expect } from './fixtures';

async function buildArgv(page, tonemap: string, peak: number, desat: number, format: string, quality: number, inName: string) {
  return await page.evaluate(async ({ tonemap, peak, desat, format, quality, inName }) => {
    const mod = await import('/tools/video-hdr-to-sdr/gizza_ai_video_hdr_to_sdr_web.js');
    await mod.default('/tools/video-hdr-to-sdr/gizza_ai_video_hdr_to_sdr_web_bg.wasm');
    return mod.build_argv(tonemap, peak, desat, format, quality, inName);
  }, { tonemap, peak, desat, format, quality, inName });
}

test('video-hdr-to-sdr page defaults build the Hable SDR MP4 ffmpeg plan', async ({ page }) => {
  await page.goto('/tools/video-hdr-to-sdr/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-tonemap')).toHaveValue('hable');
  await expect(page.locator('#in-peak')).toHaveValue('100');
  await expect(page.locator('#in-desat')).toHaveValue('0');
  await expect(page.locator('#in-format')).toHaveValue('mp4');
  await expect(page.locator('#in-quality')).toHaveValue('75');

  const plan = await buildArgv(page, 'hable', 100, 0, 'mp4', 75, 'in.mp4');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv).toContain('-vf');
  expect(plan.argv).toContain('libx264');
  expect(plan.argv).toContain('+faststart');
  const filter = plan.argv[plan.argv.indexOf('-vf') + 1];
  expect(filter).toContain('zscale=t=linear:npl=100');
  expect(filter).toContain('tonemap=tonemap=hable:desat=0');
  expect(filter).toContain('zscale=t=bt709:m=bt709:r=tv');
  expect(filter).toContain('format=yuv420p');
});

test('video-hdr-to-sdr deep-link and non-default WebM plan are honored', async ({ page }) => {
  await page.goto('/tools/video-hdr-to-sdr/?tonemap=mobius&peak=1000&desat=0.5&format=webm&quality=80');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-tonemap')).toHaveValue('mobius');
  await expect(page.locator('#in-peak')).toHaveValue('1000');
  await expect(page.locator('#in-desat')).toHaveValue('0.5');
  await expect(page.locator('#in-format')).toHaveValue('webm');
  await expect(page.locator('#in-quality')).toHaveValue('80');

  const plan = await buildArgv(page, 'mobius', 1000, 0.5, 'webm', 80, 'in.webm');
  expect(plan.out_name).toBe('out.webm');
  expect(plan.argv).toContain('libvpx-vp9');
  expect(plan.argv).toContain('libopus');
  expect(plan.argv).toContain('0'); // VP9 constant-quality bitrate marker via -b:v 0
  const filter = plan.argv[plan.argv.indexOf('-vf') + 1];
  expect(filter).toContain('zscale=t=linear:npl=1000');
  expect(filter).toContain('tonemap=tonemap=mobius:desat=0.5');
});
