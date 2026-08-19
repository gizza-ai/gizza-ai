import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/video-fade/ page ramps a clip up from (and down to) a
// solid colour in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network). The specs decode the OUTPUT via <video> + canvas and assert the
// media is actually faded: frame brightness at the head/tail vs the middle,
// the fade COLOUR (black vs white), and the container the plan chose. Bounds
// are pre-measured with local ffmpeg on the same argv against the same
// fixture — see the per-test comments for the measured numbers.
//
// Fixture: 128×128, 2.000 s, 10 fps, H.264 + AAC. Its picture is a static
// stripe pattern whose mean channel value is ~126 with a full-contrast range
// (a pure-black stripe at channel-sum 0 and a pure-white one at 765), which is
// what makes "faded to black", "washed to white" and "untouched" three
// clearly separated measurements.
const FIXTURE = path.resolve(__dirname, 'fixtures/tiny-128x128-audio.mp4');

type FrameStats = { mean: number; minSum: number; maxSum: number };

/// Decode a data:video URL and return its dimensions, duration and per-time
/// frame statistics: the mean channel value over the whole frame plus the
/// darkest/brightest pixel channel-sums (0..765), which distinguish a solid
/// colour wash from real footage.
async function decode(
  page: Page,
  dataUrl: string,
  times: number[]
): Promise<{ w: number; h: number; duration: number; frames: FrameStats[] }> {
  return page.evaluate(
    async ({ dataUrl, times }) => {
      const v = document.createElement('video');
      v.muted = true;
      v.src = dataUrl;
      await new Promise<void>((res, rej) => {
        v.onloadeddata = () => res();
        v.onerror = () => rej(new Error('video-fade output failed to decode'));
      });
      const c = document.createElement('canvas');
      c.width = v.videoWidth;
      c.height = v.videoHeight;
      const ctx = c.getContext('2d')!;
      const frames = [];
      for (const t of times) {
        v.currentTime = t;
        await new Promise<void>((res) => {
          v.onseeked = () => res();
        });
        ctx.drawImage(v, 0, 0);
        const d = ctx.getImageData(0, 0, c.width, c.height).data;
        let total = 0;
        let minSum = 765;
        let maxSum = 0;
        for (let i = 0; i < d.length; i += 4) {
          const sum = d[i] + d[i + 1] + d[i + 2];
          total += sum;
          if (sum < minSum) minSum = sum;
          if (sum > maxSum) maxSum = sum;
        }
        frames.push({ mean: total / (d.length / 4) / 3, minSum, maxSum });
      }
      return { w: v.videoWidth, h: v.videoHeight, duration: v.duration, frames };
    },
    { dataUrl, times }
  );
}

/// Every picture-fading run re-encodes to H.264/AAC in an MP4 at the source
/// geometry and length, whatever the input container was.
async function expectFadedMp4(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = (await media.getAttribute('src'))!;
  expect(src).toMatch(/^data:video\/mp4/);
  // Real encoded bytes, not a stub: the measured outputs are 24–29 kB, so the
  // base64 payload runs to tens of thousands of characters.
  expect(src.length).toBeGreaterThan(10_000);
  return src;
}

// Both sides, both streams, default black. Measured locally on this fixture
// with fade_in=0.5 / fade_out=0.5 / duration=2: mean channel value 0 at the
// very first frame and 25 at 0.1 s, back to full ~126 from 0.45 s to 1.5 s,
// then down through 50 (1.8 s) to 25 (1.9 s). The head/tail bounds are set at
// 90 so they hold whichever side of a 0.1 s frame boundary the browser's seek
// lands on, while still being far below the un-faded middle.
test('video-fade page fades a clip in from and out to black', async ({ page }) => {
  await page.goto('/tools/video-fade/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-fade_in', '0.5');
  await page.fill('#in-fade_out', '0.5');
  await page.fill('#in-duration', '2');
  await page.setInputFiles('#in-file', FIXTURE);

  const src = await expectFadedMp4(page);
  const { w, h, duration, frames } = await decode(page, src, [0.05, 1.0, 1.85]);

  // Same geometry and length as the source — a fade never re-frames the clip.
  expect(w).toBe(128);
  expect(h).toBe(128);
  expect(duration).toBeGreaterThan(1.5);
  expect(duration).toBeLessThan(3);

  const [head, mid, tail] = frames;
  expect(head.mean).toBeLessThan(90); // ramping up out of black
  expect(head.maxSum).toBeLessThan(500); // even the brightest pixel is dimmed
  expect(mid.mean).toBeGreaterThan(110); // untouched in the middle
  expect(mid.maxSum).toBeGreaterThan(700); // full-contrast footage survives
  expect(tail.mean).toBeLessThan(90); // sinking back down to black
});

// Deep link: every field of the page is prefilled from the query string, and
// the run honours them. fade_out=0 needs no clip length, so duration stays 0.
// Measured locally with fade_in=1 / streams=video / color=white / quality=high:
// the head is a near-white wash (mean 241 at 0.1 s, darkest pixel channel-sum
// 681 — i.e. NOTHING is dark), while by 1.5 s the picture is back to mean 126
// with its pure-black stripe (channel-sum 0) restored. A black fade would
// invert both numbers, so this pins the colour, not just "something faded".
test('video-fade deep link prefills every field and fades from white', async ({ page }) => {
  await page.goto(
    '/tools/video-fade/?fade_in=1&fade_out=0&duration=0&streams=video&color=white&quality=high'
  );
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-fade_in')).toHaveValue('1', { timeout: 15_000 });
  await expect(page.locator('#in-fade_out')).toHaveValue('0');
  await expect(page.locator('#in-duration')).toHaveValue('0');
  await expect(page.locator('#in-streams')).toHaveValue('video');
  await expect(page.locator('#in-color')).toHaveValue('white');
  await expect(page.locator('#in-quality')).toHaveValue('high');
  await page.setInputFiles('#in-file', FIXTURE);

  const src = await expectFadedMp4(page);
  const { w, h, frames } = await decode(page, src, [0.05, 1.5]);
  expect(w).toBe(128);
  expect(h).toBe(128);

  const [head, mid] = frames;
  expect(head.mean).toBeGreaterThan(200); // washed towards white, not black
  expect(head.minSum).toBeGreaterThan(500); // no dark pixel left anywhere
  expect(mid.mean).toBeGreaterThan(110); // full picture once the ramp is over
  expect(mid.minSum).toBeLessThan(150); // the black stripe is back
});

// Sound-only is the lossless path: `-c:v copy` keeps the input container and
// leaves the picture bit-for-bit identical, so the head frame must still be
// the original full-contrast stripe pattern (measured mean 126, darkest pixel
// 0, brightest 765) even though fade_in is 1 s. Under the picture-fading path
// that same frame measures mean 0–25 with a maxSum of 165, so this assertion
// is what separates the two branches.
test('video-fade sound-only run copies the picture and keeps the container', async ({ page }) => {
  await page.goto('/tools/video-fade/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-fade_in', '1');
  await page.fill('#in-fade_out', '0');
  await page.selectOption('#in-streams', 'audio');
  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = (await media.getAttribute('src'))!;
  // mp4 in → mp4 out: the container survives a stream copy.
  expect(src).toMatch(/^data:video\/mp4/);
  expect(src.length).toBeGreaterThan(10_000);

  const { w, h, frames } = await decode(page, src, [0.05]);
  expect(w).toBe(128);
  expect(h).toBe(128);
  const [head] = frames;
  expect(head.mean).toBeGreaterThan(90); // NOT dimmed — the picture is untouched
  expect(head.minSum).toBeLessThan(150); // black stripe intact
  expect(head.maxSum).toBeGreaterThan(700); // white stripe intact
});

// Static page contract: the example chips are the documented presets, and the
// colour chip drives both the colour text input and the streams select.
test('video-fade page ships its example presets', async ({ page }) => {
  await page.goto('/tools/video-fade/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await page.click('.tool-example-chip:has-text("Fade to white")');
  await expect(page.locator('#in-color')).toHaveValue('#ffffff');
  await expect(page.locator('#in-streams')).toHaveValue('video');
  await expect(page.locator('#in-fade_in')).toHaveValue('1');
  await expect(page.locator('#in-fade_out')).toHaveValue('1');
  await expect(page.locator('#tool-reset')).toBeVisible();
});
