import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/video-title-card/ page burns a styled caption onto an
// uploaded video in-browser via ffmpeg drawtext (@ffmpeg/core from jsDelivr —
// needs network). The bundled font + the caption text are shipped to the
// browser ffmpeg FS as extra virtual-FS inputs, so drawtext resolves the same
// way it does on the CLI. The specs decode the OUTPUT via <video> + canvas and
// assert the text is really drawn — the right color, at the right vertical
// half, and ONLY inside the enable (start..end) window.
//
// Fixture: a solid-BLACK 240×180 3 s clip, so any non-black pixel is text/box
// and the enable window is trivially checkable (before it → pure black).

const BLACK = path.resolve(__dirname, 'fixtures/title-black-3s.mp4'); // 240×180, 3 s, black
const WEBM = path.resolve(__dirname, 'fixtures/clip-1s.webm');

interface Stats {
  w: number;
  h: number;
  white: number;
  whiteTop: number;
  whiteBottom: number;
  yellow: number;
  yellowTop: number;
  yellowBottom: number;
  navy: number;
  navyTop: number;
  navyBottom: number;
  nonBlack: number;
}

/// Seek a data:video URL to `time` seconds, draw that frame to a canvas, and
/// count pixels matching a few fixed color predicates (whole frame + split by
/// top/bottom half). One getImageData call → fast.
async function frameStats(page: Page, dataUrl: string, time: number): Promise<Stats> {
  return page.evaluate(
    async ({ dataUrl, time }) => {
      const v = document.createElement('video');
      v.muted = true;
      v.src = dataUrl;
      await new Promise<void>((res, rej) => {
        v.onloadeddata = () => res();
        v.onerror = () => rej(new Error('video decode failed'));
      });
      v.currentTime = time;
      await new Promise<void>((res) => {
        v.onseeked = () => res();
      });
      // onseeked can fire before the seeked frame has actually been painted, and
      // drawImage then captures an unpainted (black) canvas. That is the flake
      // that made video-aspect-pad report r=0 on bar pixels in one nightly and
      // pass unchanged in the next. Wait for a presented frame, fall back to a
      // rAF pair where requestVideoFrameCallback is unavailable, and always time
      // out so a stalled decode fails the assertion rather than hanging.
      await new Promise<void>((res) => {
        const anyV = v as any;
        if (typeof anyV.requestVideoFrameCallback === 'function') {
          anyV.requestVideoFrameCallback(() => res());
        } else {
          requestAnimationFrame(() => requestAnimationFrame(() => res()));
        }
        setTimeout(() => res(), 2000);
      });
      const c = document.createElement('canvas');
      c.width = v.videoWidth;
      c.height = v.videoHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(v, 0, 0);
      const { data } = ctx.getImageData(0, 0, c.width, c.height);
      const mid = c.height / 2;
      const s: any = {
        w: v.videoWidth,
        h: v.videoHeight,
        white: 0, whiteTop: 0, whiteBottom: 0,
        yellow: 0, yellowTop: 0, yellowBottom: 0,
        navy: 0, navyTop: 0, navyBottom: 0,
        nonBlack: 0,
      };
      for (let i = 0; i < data.length; i += 4) {
        const r = data[i], g = data[i + 1], b = data[i + 2];
        const row = ((i / 4) / c.width) | 0;
        const top = row < mid;
        if (r > 40 || g > 40 || b > 40) s.nonBlack++;
        if (r > 180 && g > 180 && b > 180) { s.white++; top ? s.whiteTop++ : s.whiteBottom++; }
        if (r > 170 && g > 170 && b < 110) { s.yellow++; top ? s.yellowTop++ : s.yellowBottom++; }
        if (b > 50 && r < 90 && g < 90) { s.navy++; top ? s.navyTop++ : s.navyBottom++; }
      }
      return s;
    },
    { dataUrl, time }
  );
}

async function outputSrc(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
  return src!;
}

// 1. Core correctness: white text is drawn ONLY inside the 1–2 s enable window,
//    the frame size is preserved, and the non-default "no background bar" state
//    works. Long-hex #ffffff for the text color.
test('video-title-card draws white text only within the enable window', async ({ page }) => {
  await page.goto('/tools/video-title-card/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-text', 'HELLO');
  await page.selectOption('#in-position', 'center');
  await page.fill('#in-font_color', '#ffffff');
  await page.uncheck('#in-background'); // non-default checkbox state
  await page.fill('#in-start', '1');
  await page.fill('#in-end', '2');
  await page.setInputFiles('#in-file', BLACK);

  const src = await outputSrc(page);

  // Inside the window (t=1.5): white text present, size preserved.
  const inWin = await frameStats(page, src, 1.5);
  expect(inWin.w).toBe(240);
  expect(inWin.h).toBe(180);
  expect(inWin.white).toBeGreaterThan(20);

  // Before the window (t=0.3): the frame is untouched → (near) pure black, no
  // caption. A caption is hundreds of pixels; allow a couple of stray decoder
  // pixels but assert there is no text/box.
  const outWin = await frameStats(page, src, 0.3);
  expect(outWin.white).toBeLessThan(5);
  expect(outWin.nonBlack).toBeLessThan(20);
});

// 2. Deep link prefills every param; a SHORT-hex (#ff0) text color must render
//    true yellow (a broken hex path would fall back to white/black), and the
//    top-left position must put the text in the TOP half.
test('video-title-card deep link renders short-hex yellow text at the top', async ({ page }) => {
  await page.goto('/tools/video-title-card/?text=LIVE&position=top-left&font_color=%23ff0&background=false&start=0&end=3');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-text')).toHaveValue('LIVE', { timeout: 15_000 });
  await expect(page.locator('#in-position')).toHaveValue('top-left');
  await expect(page.locator('#in-font_color')).toHaveValue('#ff0');
  await expect(page.locator('#in-background')).not.toBeChecked();
  await page.setInputFiles('#in-file', BLACK);

  const src = await outputSrc(page);
  const f = await frameStats(page, src, 1.0);
  expect(f.yellow).toBeGreaterThan(10); // short-hex → real yellow, not white/black
  expect(f.white).toBeLessThan(f.yellow); // the glyphs are yellow, not white
  expect(f.yellowTop).toBeGreaterThan(0); // top-left → text in the top half
  expect(f.yellowBottom).toBe(0);
});

// 3. Background bar (default ON) with a NAMED color (navy) and the bottom-right
//    position: the navy box must appear in the BOTTOM half with white text on it.
test('video-title-card draws a named-color background bar at bottom-right', async ({ page }) => {
  await page.goto('/tools/video-title-card/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-text', 'NEWS');
  await page.selectOption('#in-position', 'bottom-right');
  await page.fill('#in-font_color', 'white');
  await expect(page.locator('#in-background')).toBeChecked(); // default ON
  await page.fill('#in-background_color', 'navy');
  await page.fill('#in-start', '0');
  await page.fill('#in-end', '3');
  await page.setInputFiles('#in-file', BLACK);

  const src = await outputSrc(page);
  const f = await frameStats(page, src, 1.5);
  expect(f.navyBottom).toBeGreaterThan(20); // navy box in the bottom half
  expect(f.navyTop).toBe(0);
  expect(f.whiteBottom).toBeGreaterThan(0); // white text sits on the bar
});

// 4. Secondary input format: a WebM (VP8/Opus) must be accepted and converted —
//    output is a playable data:video/ (mp4). Exercises the h264_out_ext webm→mp4
//    + AAC path on the real surface.
test('video-title-card accepts a WebM input and returns a playable video', async ({ page }) => {
  await page.goto('/tools/video-title-card/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-text', 'Hi');
  await page.setInputFiles('#in-file', WEBM);

  const src = await outputSrc(page);
  const dims = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.src = dataUrl;
    await new Promise<void>((res, rej) => { v.onloadeddata = () => res(); v.onerror = () => rej(new Error('decode')); });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
  expect(dims.w).toBeGreaterThan(0);
  expect(dims.h).toBeGreaterThan(0);
});

// 5. Cap boundary (font_size=400, the exact max) still succeeds, plus the static
//    page contract: a runnable CLI example, platform-labeled position options,
//    and the four example chips.
test('video-title-card honors the font-size cap and ships a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/video-title-card/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-text', 'A');
  await page.selectOption('#in-position', 'center');
  await page.fill('#in-font_size', '400'); // exact cap → accepted
  await page.setInputFiles('#in-file', BLACK);
  await outputSrc(page); // succeeds at the boundary

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool video-title-card 'url=https://example.com/input' 'text=Jane Doe — Chief Engineer' 'position=bottom-center' 'font_size=48' 'background=true' 'background_opacity=0.5' 'start=0' 'end=5'"
  );
  await expect(page.locator('#in-position option[value="bottom-left"]')).toHaveText('Bottom left (lower third)');
  await expect(page.locator('#in-position option[value="center"]')).toHaveText('Center');
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
});
