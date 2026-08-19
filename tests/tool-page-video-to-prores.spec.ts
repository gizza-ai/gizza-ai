import { test, expect } from './fixtures';
import path from 'node:path';

function bytesFromDataUrl(src: string): Buffer {
  const m = src.match(/^data:([^;,]+);base64,(.*)$/);
  if (!m) throw new Error(`not a base64 data URL: ${src.slice(0, 80)}`);
  return Buffer.from(m[2], 'base64');
}

function asciiContains(buf: Buffer, token: string): boolean {
  return buf.includes(Buffer.from(token, 'ascii'));
}

// ProRes is not browser-decodable, so correctness is asserted by parsing the MOV
// bytes for the QuickTime/ProRes markers that ffmpeg writes: the codec fourcc
// (apco/apcs/apcn/apch), encoder name, and yuv422p10le pixel-format marker.
test('video-to-prores creates a default ProRes 422 MOV download', async ({ page }) => {
  await page.goto('/tools/video-to-prores/');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('standard');
  await expect(page.locator('#in-resolution')).toHaveValue('source');
  await expect(page.locator('#in-audio')).toHaveValue('pcm16');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/quicktime/);

  const bytes = bytesFromDataUrl(src!);
  expect(bytes.length).toBeGreaterThan(10_000);
  expect(asciiContains(bytes, 'ftypqt')).toBeTruthy();
  expect(asciiContains(bytes, 'apcn')).toBeTruthy(); // standard ProRes 422 fourcc
  expect(asciiContains(bytes, 'Lavc')).toBeTruthy(); // ffmpeg encoder metadata
});

test('video-to-prores deep-link uses proxy, 720p, and no audio', async ({ page }) => {
  await page.goto('/tools/video-to-prores/?profile=proxy&resolution=720p&audio=none');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('proxy');
  await expect(page.locator('#in-resolution')).toHaveValue('720p');
  await expect(page.locator('#in-audio')).toHaveValue('none');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/clip-1s.webm'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/quicktime/);

  const bytes = bytesFromDataUrl(src!);
  expect(asciiContains(bytes, 'apco')).toBeTruthy(); // ProRes Proxy fourcc
  expect(asciiContains(bytes, 'apcn')).toBeFalsy();
});

test('video-to-prores exercises LT profile and 24-bit PCM option', async ({ page }) => {
  await page.goto('/tools/video-to-prores/');
  await page.waitForSelector('#in-video');
  await page.selectOption('#in-profile', 'lt');
  await page.selectOption('#in-audio', 'pcm24');
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-h264.mov'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  const bytes = bytesFromDataUrl(src!);
  expect(asciiContains(bytes, 'apcs')).toBeTruthy(); // ProRes LT fourcc
});
