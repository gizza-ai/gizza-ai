import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

function bytesFromDataUrl(src: string): Buffer {
  const m = src.match(/^data:([^;,]+);base64,(.*)$/);
  if (!m) throw new Error(`not a base64 data URL: ${src.slice(0, 80)}`);
  return Buffer.from(m[2], 'base64');
}

async function buildArgv(
  page,
  videoCodec: string,
  resolution: string,
  fps: string,
  videoBitrate: number,
  keyframeSeconds: number,
  audioCodec: string,
  audioBitrate: number,
  inName: string
) {
  return await page.evaluate(
    async ({ videoCodec, resolution, fps, videoBitrate, keyframeSeconds, audioCodec, audioBitrate, inName }) => {
      const mod = await import('/tools/mp4-to-flv/gizza_ai_mp4_to_flv_web.js');
      await mod.default('/tools/mp4-to-flv/gizza_ai_mp4_to_flv_web_bg.wasm');
      return mod.build_argv(videoCodec, resolution, fps, videoBitrate, keyframeSeconds, audioCodec, audioBitrate, inName);
    },
    { videoCodec, resolution, fps, videoBitrate, keyframeSeconds, audioCodec, audioBitrate, inName }
  );
}

test('mp4-to-flv wasm build_argv emits the default RTMP-friendly FLV encode plan', async ({ page }) => {
  await page.goto('/tools/mp4-to-flv/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 'h264', 'source', 'source', 2500, 2, 'aac', 128, 'in.mp4');
  expect(plan.out_name).toBe('out.flv');
  expect(plan.argv).toEqual([
    '-i', 'in.mp4',
    '-vf', "scale='2*trunc(iw/2)':'2*trunc(ih/2)'",
    '-map', '0:v:0',
    '-map', '0:a:0?',
    '-c:v', 'libx264',
    '-preset', 'veryfast',
    '-pix_fmt', 'yuv420p',
    '-b:v', '2500k',
    '-maxrate', '2500k',
    '-bufsize', '5000k',
    '-force_key_frames', 'expr:gte(t,n_forced*2)',
    '-c:a', 'aac',
    '-b:a', '128k',
    '-f', 'flv',
    'out.flv'
  ]);
});

test('mp4-to-flv page deep-link prefills legacy Flash settings', async ({ page }) => {
  await page.goto('/tools/mp4-to-flv/?video_codec=flv1&resolution=360p&fps=15fps&video_bitrate=800&keyframe_seconds=2&audio_codec=mp3&audio_bitrate=96');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-video_codec')).toHaveValue('flv1');
  await expect(page.locator('#in-resolution')).toHaveValue('360p');
  await expect(page.locator('#in-fps')).toHaveValue('15fps');
  await expect(page.locator('#in-video_bitrate')).toHaveValue('800');
  await expect(page.locator('#in-keyframe_seconds')).toHaveValue('2');
  await expect(page.locator('#in-audio_codec')).toHaveValue('mp3');
  await expect(page.locator('#in-audio_bitrate')).toHaveValue('96');
});

test('mp4-to-flv wasm build_argv covers advertised enum choices and boundaries', async ({ page }) => {
  await page.goto('/tools/mp4-to-flv/');
  await page.waitForSelector('#in-file');

  const flv1 = await buildArgv(page, 'flv1', '360p', '15fps', 800, 2, 'mp3', 96, 'clip.mov');
  expect(flv1.argv[flv1.argv.indexOf('-c:v') + 1]).toBe('flv');
  expect(flv1.argv).not.toContain('-preset');
  expect(flv1.argv[flv1.argv.indexOf('-vf') + 1]).toBe("scale=-2:'2*trunc(min(ih,360)/2)'");
  expect(flv1.argv[flv1.argv.indexOf('-r') + 1]).toBe('15');
  expect(flv1.argv[flv1.argv.indexOf('-c:a') + 1]).toBe('libmp3lame');
  expect(flv1.argv[flv1.argv.indexOf('-ar') + 1]).toBe('44100');

  for (const res of ['1080p', '720p', '576p', '480p', '360p', '240p']) {
    const planned = await buildArgv(page, 'h264', res, 'source', 1200, 2, 'aac', 128, 'in.webm');
    expect(planned.argv[planned.argv.indexOf('-vf') + 1]).toContain(res.replace('p', ''));
  }
  for (const [fps, rate] of [['60fps', '60'], ['30fps', '30'], ['25fps', '25'], ['24fps', '24'], ['15fps', '15']]) {
    const planned = await buildArgv(page, 'h264', 'source', fps, 1200, 2, 'aac', 128, 'in.mp4');
    expect(planned.argv[planned.argv.indexOf('-r') + 1]).toBe(rate);
  }

  const noAudio = await buildArgv(page, 'h264', 'source', 'source', 100, 10, 'none', 32, 'in.mp4');
  expect(noAudio.argv).toContain('-an');
  expect(noAudio.argv).not.toContain('-c:a');
  expect(noAudio.argv[noAudio.argv.indexOf('-b:v') + 1]).toBe('100k');
  expect(noAudio.argv[noAudio.argv.indexOf('-force_key_frames') + 1]).toBe('expr:gte(t,n_forced*10)');

  await expect(buildArgv(page, 'h264', 'source', 'source', 99, 2, 'aac', 128, 'in.mp4')).rejects.toThrow(/video_bitrate/);
});

test('mp4-to-flv page encodes a real FLV payload with an FLV header', async ({ page }) => {
  await page.goto('/tools/mp4-to-flv/?video_codec=h264&resolution=240p&fps=15fps&video_bitrate=300&keyframe_seconds=2&audio_codec=none&audio_bitrate=32');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', fixture);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  // Chromium does not natively know FLV, so the generated page may surface the
  // download as application/octet-stream even though the payload itself is FLV.
  expect(src).toMatch(/^data:(video\/x-flv|application\/octet-stream)/);
  const bytes = bytesFromDataUrl(src!);
  expect(bytes.length).toBeGreaterThan(1_000);
  expect(bytes.subarray(0, 3).toString('ascii')).toBe('FLV');
});
