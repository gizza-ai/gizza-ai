import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/redblue-64.mp4');

async function buildArgv(page, mode: string, profile: string, fragmentDuration: number, keyframeInterval: number, segmentIndex: string, inName: string) {
  return await page.evaluate(async ({ mode, profile, fragmentDuration, keyframeInterval, segmentIndex, inName }) => {
    const mod = await import('/tools/video-fragmented-mp4/gizza_ai_video_fragmented_mp4_web.js');
    await mod.default('/tools/video-fragmented-mp4/gizza_ai_video_fragmented_mp4_web_bg.wasm');
    return mod.build_argv(mode, profile, fragmentDuration, keyframeInterval, segmentIndex, inName);
  }, { mode, profile, fragmentDuration, keyframeInterval, segmentIndex, inName });
}

async function decodeVideo(page) {
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
      v.addEventListener('error', () => reject(new Error('fragmented MP4 output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
}

test('video-fragmented-mp4 wasm builds the exact default fMP4 remux plan', async ({ page }) => {
  await page.goto('/tools/video-fragmented-mp4/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, '', '', 0, 2, 'false', 'in.mp4');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv).toEqual([
    '-i', 'in.mp4',
    '-map', '0',
    '-c', 'copy',
    '-movflags', '+frag_keyframe+empty_moov+default_base_moof',
    'out.mp4',
  ]);
});

test('video-fragmented-mp4 wasm covers profiles, duration boundary, h264 mode, and sidx', async ({ page }) => {
  await page.goto('/tools/video-fragmented-mp4/');
  await page.waitForSelector('#in-file');

  const dash = await buildArgv(page, 'copy', 'dash', 2, 2, 'true', 'clip.mov');
  expect(dash.argv).toEqual(expect.arrayContaining([
    '-min_frag_duration', '2000000',
    '-movflags', '+frag_keyframe+empty_moov+default_base_moof+dash+negative_cts_offsets+global_sidx',
  ]));

  const cmaf = await buildArgv(page, 'copy', 'cmaf', 30, 0, 'false', 'clip.mp4');
  expect(cmaf.argv).toEqual(expect.arrayContaining([
    '-min_frag_duration', '30000000',
    '-movflags', '+frag_keyframe+empty_moov+default_base_moof+cmaf+negative_cts_offsets',
  ]));

  const h264 = await buildArgv(page, 'h264', 'mse', 0, 1.5, 'false', 'clip.webm');
  expect(h264.argv).toEqual(expect.arrayContaining([
    '-c:v', 'libx264',
    '-pix_fmt', 'yuv420p',
    '-force_key_frames', 'expr:gte(t,n_forced*1.5)',
    '-c:a', 'aac',
    '-b:a', '128k',
  ]));

  await expect(buildArgv(page, 'copy', 'mse', 31, 2, 'false', 'in.mp4'))
    .rejects.toThrow(/fragment_duration must be 0-30/);
});

test('video-fragmented-mp4 page renders a playable fragmented MP4', async ({ page }) => {
  await page.goto('/tools/video-fragmented-mp4/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', fixture);

  const frame = await decodeVideo(page);
  expect(frame.w).toBe(64);
  expect(frame.h).toBe(64);
  expect(frame.duration).toBeGreaterThan(0);
});

test('video-fragmented-mp4 deep-link prefills controls and runs h264 cmaf', async ({ page }) => {
  await page.goto('/tools/video-fragmented-mp4/?mode=h264&profile=cmaf&fragment_duration=2&keyframe_interval=2&segment_index=true');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-mode')).toHaveValue('h264');
  await expect(page.locator('#in-profile')).toHaveValue('cmaf');
  await expect(page.locator('#in-fragment_duration')).toHaveValue('2');
  await expect(page.locator('#in-keyframe_interval')).toHaveValue('2');
  await expect(page.locator('#in-segment_index')).toBeChecked();
  await page.setInputFiles('#in-file', fixture);
  const frame = await decodeVideo(page);
  expect(frame.w).toBe(64);
  expect(frame.h).toBe(64);
});
