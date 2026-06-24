import { test, expect } from './fixtures';

const SRT =
  '1\n00:00:01,000 --> 00:00:04,000\nHello.\n\n2\n00:00:05,500 --> 00:00:07,250\nWorld.';
const VTT =
  'WEBVTT\n\n1\n00:00:01.000 --> 00:00:04.000\nHello.\n\n2\n00:00:05.500 --> 00:00:07.250\nWorld.';

test('srt-to-vtt page auto-detects SRT and converts to WebVTT', async ({ page }) => {
  await page.goto('/tools/srt-to-vtt/');
  await page.fill('#in-subtitles', SRT);
  // Default direction is "auto".
  await expect(page.locator('#tool-output')).toHaveText(VTT, { timeout: 15000 });
});

test('srt-to-vtt page converts WebVTT back to SRT', async ({ page }) => {
  await page.goto('/tools/srt-to-vtt/');
  await page.fill('#in-subtitles', VTT);
  await page.selectOption('#in-direction', 'vtt-to-srt');
  await expect(page.locator('#tool-output')).toHaveText(SRT, { timeout: 15000 });
});

test('srt-to-vtt page forces srt-to-vtt direction', async ({ page }) => {
  await page.goto('/tools/srt-to-vtt/');
  await page.fill('#in-subtitles', '1\n00:00:01,000 --> 00:00:02,000\nHi');
  await page.selectOption('#in-direction', 'srt-to-vtt');
  await expect(page.locator('#tool-output')).toHaveText(
    'WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHi',
    { timeout: 15000 },
  );
});

test('srt-to-vtt page query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/srt-to-vtt/?subtitles=' +
      encodeURIComponent('1\n00:00:01,000 --> 00:00:02,000\nHi') +
      '&direction=srt-to-vtt',
  );
  await expect(page.locator('#in-direction')).toHaveValue('srt-to-vtt', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toHaveText(
    'WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHi',
    { timeout: 15000 },
  );
});
