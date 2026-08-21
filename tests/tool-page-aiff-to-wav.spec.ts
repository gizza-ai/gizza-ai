import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/aiff-to-wav/ page re-containers an uploaded AIFF as a
// RIFF/WAVE .wav in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network). The page wasm export is
// `build_argv(bit_depth, sample_rate, channels, keep_metadata, in_name)` — the
// meta.toml field order MUST match, since tool.js calls
// `build_argv(...fieldArgs, inName)`.
//
// The argv tests exercise the page's OWN wasm module directly, so every
// advertised enum value and the non-default checkbox state are covered without
// paying for a real ffmpeg run each. Two end-to-end tests then prove the plan
// actually produces decodable audio, including through a deep link.

const AIFF = path.resolve(__dirname, 'fixtures/tone-3s.aif'); // 3 s, 8 kHz mono, pcm_s16be
const WAV = path.resolve(__dirname, 'fixtures/tone-3s.wav');

type Plan = { argv: string[]; out_name: string };

// Load the page's own wasm module and build the ffmpeg plan for given fields.
async function buildArgv(
  page: Page,
  bitDepth: string,
  sampleRate: string,
  channels: string,
  keepMetadata: string,
  inName: string,
): Promise<Plan> {
  return page.evaluate(async (a: string[]) => {
    const mod = await import('/tools/aiff-to-wav/gizza_ai_aiff_to_wav_web.js');
    await mod.default('/tools/aiff-to-wav/gizza_ai_aiff_to_wav_web_bg.wasm');
    return mod.build_argv(a[0], a[1], a[2], a[3], a[4]);
  }, [bitDepth, sampleRate, channels, keepMetadata, inName]);
}

async function decodeStats(
  page: Page,
  src: string,
): Promise<{ channels: number; duration: number; sampleRate: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return {
      channels: decoded.numberOfChannels,
      duration: decoded.duration,
      sampleRate: decoded.sampleRate,
    };
  }, src);
}

test('aiff-to-wav wasm build_argv defaults to explicit 24-bit and keeps rate, layout and tags', async ({
  page,
}) => {
  await page.goto('/tools/aiff-to-wav/');
  await page.waitForSelector('#in-file');

  // Blank fields must fall back to the descriptor defaults, not error.
  const plan = await buildArgv(page, '', '', '', '', 'in.aiff');
  expect(plan.out_name).toBe('out.wav');
  expect(plan.argv).toEqual([
    '-i',
    'in.aiff',
    '-vn',
    '-map_metadata',
    '0',
    '-c:a',
    'pcm_s24le',
    'out.wav',
  ]);
  // "keep" must not resample or remap channels at all.
  expect(plan.argv).not.toContain('-ar');
  expect(plan.argv).not.toContain('-ac');
});

test('aiff-to-wav wasm build_argv covers every advertised bit depth', async ({ page }) => {
  await page.goto('/tools/aiff-to-wav/');
  await page.waitForSelector('#in-file');

  const codecs: Record<string, string> = {
    '16': 'pcm_s16le',
    '24': 'pcm_s24le',
    '32': 'pcm_s32le',
    float32: 'pcm_f32le',
    alaw: 'pcm_alaw',
    mulaw: 'pcm_mulaw',
  };
  for (const [depth, codec] of Object.entries(codecs)) {
    const plan = await buildArgv(page, depth, 'keep', 'keep', 'true', 'in.aiff');
    expect(plan.argv, `bit_depth=${depth}`).toEqual([
      '-i',
      'in.aiff',
      '-vn',
      '-map_metadata',
      '0',
      '-c:a',
      codec,
      'out.wav',
    ]);
  }
});

test('aiff-to-wav wasm build_argv covers every advertised sample rate and channel layout', async ({
  page,
}) => {
  await page.goto('/tools/aiff-to-wav/');
  await page.waitForSelector('#in-file');

  const rates = ['keep', '8000', '16000', '22050', '44100', '48000', '88200', '96000', '192000'];
  for (const rate of rates) {
    const plan = await buildArgv(page, '24', rate, 'keep', 'true', 'in.aiff');
    if (rate === 'keep') {
      expect(plan.argv, 'keep must omit -ar').not.toContain('-ar');
    } else {
      const i = plan.argv.indexOf('-ar');
      expect(i, `sample_rate=${rate} must emit -ar`).toBeGreaterThan(-1);
      expect(plan.argv[i + 1]).toBe(rate);
    }
  }

  const layouts: Record<string, string | null> = { keep: null, mono: '1', stereo: '2' };
  for (const [layout, count] of Object.entries(layouts)) {
    const plan = await buildArgv(page, '24', 'keep', layout, 'true', 'in.aiff');
    if (count === null) {
      expect(plan.argv, 'keep must omit -ac').not.toContain('-ac');
    } else {
      const i = plan.argv.indexOf('-ac');
      expect(i, `channels=${layout} must emit -ac`).toBeGreaterThan(-1);
      expect(plan.argv[i + 1]).toBe(count);
    }
  }
});

test('aiff-to-wav wasm build_argv strips tags when the checkbox is unchecked', async ({ page }) => {
  await page.goto('/tools/aiff-to-wav/');
  await page.waitForSelector('#in-file');

  // The page marshals a cleared checkbox as the string "false" (readField).
  const off = await buildArgv(page, '16', '44100', 'stereo', 'false', 'in.aifc');
  expect(off.argv).toEqual([
    '-i',
    'in.aifc',
    '-vn',
    '-map_metadata',
    '-1',
    '-c:a',
    'pcm_s16le',
    '-ar',
    '44100',
    '-ac',
    '2',
    'out.wav',
  ]);

  // A checked box sends "true"; a deep link may also send 1/on/yes.
  for (const truthy of ['true', '1', 'on', 'yes']) {
    const on = await buildArgv(page, '24', 'keep', 'keep', truthy, 'in.aif');
    expect(on.argv.slice(3, 5), `keep_metadata=${truthy}`).toEqual(['-map_metadata', '0']);
  }
});

test('aiff-to-wav wasm build_argv rejects a value outside the advertised enums', async ({
  page,
}) => {
  await page.goto('/tools/aiff-to-wav/');
  await page.waitForSelector('#in-file');

  await expect(buildArgv(page, '12', 'keep', 'keep', 'true', 'in.aiff')).rejects.toThrow(
    /bit_depth must be one of/,
  );
});

test('aiff-to-wav page converts an uploaded AIFF to WAV with defaults', async ({ page }) => {
  await page.goto('/tools/aiff-to-wav/');
  await page.waitForSelector('#in-file');
  // The default-true boolean renders as a checked box.
  await expect(page.locator('#in-keep_metadata')).toBeChecked({ timeout: 15_000 });
  await expect(page.locator('#in-bit_depth')).toHaveValue('24');

  await page.setInputFiles('#in-file', AIFF);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);

  // "keep" defaults must preserve the fixture's 8 kHz mono 3 s content.
  const stats = await decodeStats(page, src!);
  expect(stats.channels).toBe(1);
  expect(stats.duration).toBeGreaterThan(2.9);
  expect(stats.duration).toBeLessThan(3.2);
});

test('aiff-to-wav deep link prefills 16-bit 44100 Hz stereo and converts', async ({ page }) => {
  await page.goto(
    '/tools/aiff-to-wav/?bit_depth=16&sample_rate=44100&channels=stereo&keep_metadata=false',
  );
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-bit_depth')).toHaveValue('16', { timeout: 15_000 });
  await expect(page.locator('#in-sample_rate')).toHaveValue('44100');
  await expect(page.locator('#in-channels')).toHaveValue('stereo');
  await expect(page.locator('#in-keep_metadata')).not.toBeChecked();

  // A .wav secondary input proves ffmpeg probes the bytes, not the extension.
  await page.setInputFiles('#in-file', WAV);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);

  // The prefilled resample + upmix must actually take effect.
  const stats = await decodeStats(page, src!);
  expect(stats.channels).toBe(2);
  expect(stats.duration).toBeGreaterThan(2.9);
  expect(stats.duration).toBeLessThan(3.2);
});
