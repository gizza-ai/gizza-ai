// Shared transparency check for image-edit tool pages.
//
// fixtures/alpha-square-64.png is 64x64: fully transparent everywhere
// (with GREEN stored under the transparent pixels, so a tool that drops alpha
// visibly turns the background green) except an opaque red square covering
// x,y in [16, 48). An edit that keeps transparency must leave that alpha mask
// exactly as it was: 0 outside the square, 255 inside.
import type { Page } from '@playwright/test';
import path from 'node:path';

export const ALPHA_SQUARE = path.resolve(__dirname, 'fixtures/alpha-square-64.png');

export type AlphaReport = {
  w: number;
  h: number;
  /** Pixels whose alpha differs from the fixture's mask (0 outside, 255 inside). */
  mismatches: number;
  /** RGBA at a few landmarks: background, the square's corner, its center. */
  corner: number[];
  squareCorner: number[];
  center: number[];
};

/** Decode a tool's output image and compare its alpha against the fixture's mask. */
export async function alphaReport(page: Page, src: string): Promise<AlphaReport> {
  return page.evaluate(async (s) => {
    const img = new Image();
    await new Promise((res, rej) => {
      img.onload = res;
      img.onerror = rej;
      img.src = s;
    });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
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
      w: c.width,
      h: c.height,
      mismatches,
      corner: at(1, 1),
      squareCorner: at(16, 16),
      center: at(32, 32),
    };
  }, src);
}
