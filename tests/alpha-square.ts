// Shared transparency check for image-edit tool pages.
//
// fixtures/alpha-square-64.png is 64x64: fully transparent everywhere
// (with GREEN stored under the transparent pixels, so a tool that drops alpha
// visibly turns the background green) except an opaque red (200,30,30) square
// covering x,y in [16, 48). An edit that keeps transparency must leave that
// alpha mask exactly as it was: 0 outside the square, 255 inside.
//
// The same picture also exists as an indexed PNG whose palette carries the
// transparency (tRNS — ffmpeg decodes it as `pal8`, which its format
// negotiation does not count as alpha) and as a GIF with a transparent index.
import { expect, type Page } from '@playwright/test';
import path from 'node:path';

export const ALPHA_SQUARE = path.resolve(__dirname, 'fixtures/alpha-square-64.png');
export const ALPHA_SQUARE_PAL = path.resolve(__dirname, 'fixtures/alpha-square-64-pal.png');
export const ALPHA_SQUARE_GIF = path.resolve(__dirname, 'fixtures/alpha-square-64.gif');

export type AlphaReport = {
  /** The result's MIME type, from the fetched blob. */
  mime: string;
  w: number;
  h: number;
  /** Pixels whose alpha differs from the fixture's mask (0 outside, 255 inside). */
  mismatches: number;
  /** RGBA at a few landmarks: background, the square's corner, its center. */
  corner: number[];
  squareCorner: number[];
  center: number[];
};

/** Wait for the tool's output image and return its URL (data: or blob:). */
export async function resultUrl(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  return (await media.getAttribute('src'))!;
}

/**
 * Fetch a tool's output image in-page (works for data: and blob: URLs),
 * decode it without premultiplying, and compare its alpha against the
 * fixture's mask.
 */
export async function alphaReport(page: Page, url: string): Promise<AlphaReport> {
  return page.evaluate(async (u) => {
    const blob = await (await fetch(u)).blob();
    const bmp = await createImageBitmap(blob, { premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
    const c = new OffscreenCanvas(bmp.width, bmp.height);
    const ctx = c.getContext('2d')!;
    ctx.drawImage(bmp, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    let mismatches = 0;
    for (let y = 0; y < c.height; y++) {
      for (let x = 0; x < c.width; x++) {
        const inside = x >= 16 && x < 48 && y >= 16 && y < 48;
        if (d[(y * c.width + x) * 4 + 3] !== (inside ? 255 : 0)) mismatches++;
      }
    }
    const at = (x: number, y: number) => Array.from(d.slice((y * c.width + x) * 4, (y * c.width + x) * 4 + 4));
    return {
      mime: blob.type,
      w: c.width,
      h: c.height,
      mismatches,
      corner: at(1, 1),
      squareCorner: at(16, 16),
      center: at(32, 32),
    };
  }, url);
}

/** The opaque square's RGB is still (roughly) the fixture's red. */
export function expectRed(rgba: number[], tolerance = 12) {
  const [r, g, b, a] = rgba;
  expect(a).toBe(255);
  expect(Math.abs(r - 200)).toBeLessThanOrEqual(tolerance);
  expect(Math.abs(g - 30)).toBeLessThanOrEqual(tolerance);
  expect(Math.abs(b - 30)).toBeLessThanOrEqual(tolerance);
}

/** A JPEG result: transparency was flattened onto white, not the hidden color. */
export function expectFlattenedOnWhite(r: AlphaReport) {
  expect(r.mime).toBe('image/jpeg');
  const [cr, cg, cb, ca] = r.corner;
  expect(ca).toBe(255);
  for (const v of [cr, cg, cb]) expect(v).toBeGreaterThanOrEqual(245);
}
