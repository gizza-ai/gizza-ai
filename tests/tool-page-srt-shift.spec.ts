import { test, expect } from './fixtures';

const SAMPLE =
  '1\n00:00:01,000 --> 00:00:04,000\nHello.\n\n2\n00:00:05,500 --> 00:00:07,250\nWorld.';

test('srt-shift page shifts forward by seconds (default unit)', async ({ page }) => {
  await page.goto('/tools/srt-shift/');
  await page.fill('#in-srt', SAMPLE);
  await page.fill('#in-offset', '2.5');
  // Default unit select is "seconds".
  await expect(page.locator('#tool-output')).toHaveText(
    '1\n00:00:03,500 --> 00:00:06,500\nHello.\n\n2\n00:00:08,000 --> 00:00:09,750\nWorld.',
    { timeout: 15000 },
  );
});

test('srt-shift page shifts backward in milliseconds', async ({ page }) => {
  await page.goto('/tools/srt-shift/');
  await page.fill('#in-srt', '1\n00:00:01,000 --> 00:00:04,000\nHi');
  await page.fill('#in-offset', '-500');
  await page.selectOption('#in-unit', 'milliseconds');
  await expect(page.locator('#tool-output')).toHaveText(
    '1\n00:00:00,500 --> 00:00:03,500\nHi',
    { timeout: 15000 },
  );
});

test('srt-shift page clamps a negative result to zero', async ({ page }) => {
  await page.goto('/tools/srt-shift/');
  await page.fill('#in-srt', '1\n00:00:01,000 --> 00:00:04,000\nHi');
  await page.fill('#in-offset', '-10');
  await expect(page.locator('#tool-output')).toHaveText(
    '1\n00:00:00,000 --> 00:00:00,000\nHi',
    { timeout: 15000 },
  );
});

test('srt-shift page query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/srt-shift/?srt=' +
      encodeURIComponent('1\n00:00:01,000 --> 00:00:02,000\nHi') +
      '&offset=1',
  );
  await expect(page.locator('#in-offset')).toHaveValue('1', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    '1\n00:00:02,000 --> 00:00:03,000\nHi',
    { timeout: 15000 },
  );
});
