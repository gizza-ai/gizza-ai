import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/extract-audio-from-video/ page pulls the audio track out
// of an uploaded video in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network). The output is audio, so the media element is an <audio> and its src
// is a data:audio/ URL. The fixture has both a video and an AAC audio stream.
test('extract-audio-from-video page produces an audio track from a video', async ({ page }) => {
  await page.goto('/tools/extract-audio-from-video/');
  await page.waitForSelector('#in-video');
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:audio\//);
});
