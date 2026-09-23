import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/wav-to-alac/ page encodes an uploaded WAV to ALAC (Apple
// Lossless) inside an .m4a container in-browser via ffmpeg (@ffmpeg/core from
// jsDelivr — needs network), so the media src is a data:audio/mp4 URL. ALAC has
// no quality knob — the PCM is carried through untouched — so there is nothing
// lossy to measure; what matters is that a real ALAC-in-MP4 file came back and
// that the deep-linked settings reached the form. Chromium's AudioContext
// cannot decode ALAC, so the output is verified at the CONTAINER level instead:
// ISO BMFF `ftyp` near the start, an M4A/mp42/isom compatible brand, and the
// `alac` sample-entry fourcc in the `stsd` box. The exact numeric settings
// (sample_rate, channels, …) are proven through the pure wasm `build_argv`,
// which is shared with the chat block via core.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav'); // 3 s tone

type AlacProbe = {
  length: number;
  ftypOffset: number;
  majorBrand: string;
  compatibleBrands: string[];
  hasAlacFourcc: boolean;
};

/// Base64-decode the data: URL in the page and probe the raw bytes for the
/// ISO BMFF / ALAC markers. No AudioContext — Chromium can't decode ALAC.
async function probeAlac(page: Page, dataUrl: string): Promise<AlacProbe> {
  return page.evaluate((url) => {
    const b64 = url.slice(url.indexOf(',') + 1);
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);

    const ascii = (start: number, len: number) =>
      String.fromCharCode(...bytes.subarray(start, start + len));

    // `ftyp` is the first box in a well-formed MP4 (offset 4), but tolerate a
    // small leading box (e.g. a free/skip atom) before it.
    let ftypOffset = -1;
    for (let i = 0; i < Math.min(64, bytes.length - 4); i++) {
      if (ascii(i, 4) === 'ftyp') {
        ftypOffset = i;
        break;
      }
    }

    let majorBrand = '';
    const compatibleBrands: string[] = [];
    if (ftypOffset >= 0) {
      majorBrand = ascii(ftypOffset + 4, 4);
      // ftyp box: size(4) type(4) major(4) minor(4) then 4-byte compat brands.
      const boxStart = ftypOffset - 4;
      const boxSize =
        (bytes[boxStart] << 24) |
        (bytes[boxStart + 1] << 16) |
        (bytes[boxStart + 2] << 8) |
        bytes[boxStart + 3];
      const end = Math.min(boxStart + boxSize, bytes.length);
      for (let i = ftypOffset + 12; i + 4 <= end; i += 4) {
        compatibleBrands.push(ascii(i, 4));
      }
    }

    // The ALAC sample entry fourcc lives in stsd; scan the whole buffer.
    let hasAlacFourcc = false;
    for (let i = 0; i + 4 <= bytes.length; i++) {
      if (
        bytes[i] === 0x61 && // a
        bytes[i + 1] === 0x6c && // l
        bytes[i + 2] === 0x61 && // a
        bytes[i + 3] === 0x63 // c
      ) {
        hasAlacFourcc = true;
        break;
      }
    }

    return { length: bytes.length, ftypOffset, majorBrand, compatibleBrands, hasAlacFourcc };
  }, dataUrl);
}

/// Assert the output is a non-trivial ALAC-in-MP4 file.
async function expectAlacM4a(page: Page, dataUrl: string): Promise<void> {
  const probe = await probeAlac(page, dataUrl);
  expect(probe.length, 'encoded .m4a is implausibly small').toBeGreaterThan(1000);
  expect(probe.ftypOffset, 'no ISO BMFF ftyp box near the start').toBeGreaterThanOrEqual(0);
  expect(probe.ftypOffset).toBeLessThan(64);
  const brands = [probe.majorBrand, ...probe.compatibleBrands];
  expect(brands, 'no M4A/mp42/isom brand in ftyp').toEqual(
    expect.arrayContaining([expect.stringMatching(/^(M4A |mp42|isom)$/)])
  );
  expect(probe.hasAlacFourcc, 'no `alac` fourcc in the container').toBe(true);
}

/// Wait for the output player and return its data:audio/mp4 src.
async function alacOutputSrc(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/mp4/);
  return src!;
}

async function buildArgv(
  page: Page,
  bitDepth: string,
  sampleRate: string,
  channels: string,
  keepMetadata: string,
  inName: string
): Promise<{ argv: string[]; out_name: string }> {
  return page.evaluate(
    async ({ bitDepth, sampleRate, channels, keepMetadata, inName }) => {
      const mod = await import('/tools/wav-to-alac/gizza_ai_wav_to_alac_web.js');
      await mod.default('/tools/wav-to-alac/gizza_ai_wav_to_alac_web_bg.wasm');
      return mod.build_argv(bitDepth, sampleRate, channels, keepMetadata, inName);
    },
    { bitDepth, sampleRate, channels, keepMetadata, inName }
  );
}

test('wav-to-alac page encodes a WAV to Apple Lossless .m4a', async ({ page }) => {
  await page.goto('/tools/wav-to-alac/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const src = await alacOutputSrc(page);

  await expectAlacM4a(page, src);
});

test('wav-to-alac deep link resamples to 44.1 kHz', async ({ page }) => {
  await page.goto('/tools/wav-to-alac/?bit_depth=16&sample_rate=44100&channels=source');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-bit_depth')).toHaveValue('16', { timeout: 15_000 });
  await expect(page.locator('#in-sample_rate')).toHaveValue('44100');
  await expect(page.locator('#in-channels')).toHaveValue('source');

  await page.setInputFiles('#in-audio', FIXTURE);
  const src = await alacOutputSrc(page);

  // The 44.1 kHz resample itself is proven by build_argv's `-ar` below; here we
  // only prove the deep-linked run produced a real ALAC .m4a.
  await expectAlacM4a(page, src);
});

test('wav-to-alac deep link folds to mono', async ({ page }) => {
  await page.goto('/tools/wav-to-alac/?channels=mono');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-channels')).toHaveValue('mono', { timeout: 15_000 });

  await page.setInputFiles('#in-audio', FIXTURE);
  const src = await alacOutputSrc(page);

  // The mono fold itself is proven by build_argv's `-ac 1` below.
  await expectAlacM4a(page, src);
});

test('wav-to-alac wasm build_argv covers every advertised choice', async ({ page }) => {
  await page.goto('/tools/wav-to-alac/');
  await page.waitForSelector('#in-audio');

  // Default: straight lossless re-wrap — no depth/rate/channel flags at all.
  const base = await buildArgv(page, 'source', 'source', 'source', 'true', 'in.wav');
  expect(base.out_name).toBe('out.m4a');
  expect(base.argv[base.argv.indexOf('-c:a') + 1]).toBe('alac');
  expect(base.argv).not.toContain('aac');
  expect(base.argv).toContain('-vn');
  expect(base.argv[base.argv.indexOf('-movflags') + 1]).toBe('+faststart');
  expect(base.argv[base.argv.indexOf('-map_metadata') + 1]).toBe('0');
  expect(base.argv).not.toContain('-sample_fmt');
  expect(base.argv).not.toContain('-ar');
  expect(base.argv).not.toContain('-ac');

  // bit_depth → the encoder's two sample formats.
  for (const [depth, fmt] of [
    ['16', 's16p'],
    ['24', 's32p'],
  ] as const) {
    const plan = await buildArgv(page, depth, 'source', 'source', 'true', 'in.wav');
    expect(plan.out_name).toBe('out.m4a');
    expect(plan.argv[plan.argv.indexOf('-sample_fmt') + 1]).toBe(fmt);
    expect(plan.argv[plan.argv.indexOf('-c:a') + 1]).toBe('alac');
  }

  // Every advertised sample rate reaches -ar.
  for (const hz of ['44100', '48000', '88200', '96000', '176400', '192000']) {
    const plan = await buildArgv(page, 'source', hz, 'source', 'true', 'in.wav');
    expect(plan.out_name).toBe('out.m4a');
    expect(plan.argv[plan.argv.indexOf('-ar') + 1]).toBe(hz);
  }

  // channels → -ac counts.
  for (const [choice, count] of [
    ['mono', '1'],
    ['stereo', '2'],
  ] as const) {
    const plan = await buildArgv(page, 'source', 'source', choice, 'true', 'in.wav');
    expect(plan.argv[plan.argv.indexOf('-ac') + 1]).toBe(count);
  }

  // keep_metadata flips -map_metadata between copy and none.
  const dropped = await buildArgv(page, 'source', 'source', 'source', 'false', 'in.wav');
  expect(dropped.argv[dropped.argv.indexOf('-map_metadata') + 1]).toBe('-1');

  // Everything at once, on a non-.wav input name — output is always .m4a.
  const full = await buildArgv(page, '24', '192000', 'stereo', 'false', 'clip.aiff');
  expect(full.out_name).toBe('out.m4a');
  expect(full.argv[full.argv.indexOf('-c:a') + 1]).toBe('alac');
  expect(full.argv[full.argv.indexOf('-sample_fmt') + 1]).toBe('s32p');
  expect(full.argv[full.argv.indexOf('-ar') + 1]).toBe('192000');
  expect(full.argv[full.argv.indexOf('-ac') + 1]).toBe('2');
  expect(full.argv[full.argv.indexOf('-map_metadata') + 1]).toBe('-1');

  // Unsupported values are rejected by the shared plan().
  await expect(buildArgv(page, '32', 'source', 'source', 'true', 'in.wav')).rejects.toThrow(
    /bit_depth/
  );
  await expect(buildArgv(page, 'source', '8000', 'source', 'true', 'in.wav')).rejects.toThrow(
    /sample_rate/
  );
  await expect(buildArgv(page, 'source', 'source', 'surround', 'true', 'in.wav')).rejects.toThrow(
    /channels/
  );
});

test('wav-to-alac generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/wav-to-alac/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool wav-to-alac');
  expect(cli).toContain('url=https://example.com/input');
  expect(cli).toContain('bit_depth=source');
  expect(cli).toContain('sample_rate=source');
  expect(cli).toContain('channels=source');
  expect(cli).toContain('keep_metadata=true');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
