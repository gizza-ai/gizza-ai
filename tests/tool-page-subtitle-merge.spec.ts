import { test, expect } from './fixtures';

const overlay = `1
00:00:01,000 --> 00:00:02,000
Hello.

===

1
00:00:03,000 --> 00:00:04,000
Bonjour.
`;

const splitParts = `1
00:00:01,000 --> 00:00:02,000
Part one.

===

1
00:00:01,000 --> 00:00:02,000
Part two.
`;

async function setSubs(page: any, value: string) {
  await page.$eval(
    '#in-subtitles',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('subtitle-merge page merges and renumbers SRT cues', async ({ page }) => {
  await page.goto('/tools/subtitle-merge/');
  await setSubs(page, overlay);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1\n00:00:01,000 --> 00:00:02,000\nHello.', { timeout: 15000 });
  await expect(out).toContainText('2\n00:00:03,000 --> 00:00:04,000\nBonjour.');
});

test('subtitle-merge applies cumulative offset to later files', async ({ page }) => {
  await page.goto('/tools/subtitle-merge/');
  await page.fill('#in-offset_ms', '2000');
  await setSubs(page, splitParts);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1\n00:00:01,000 --> 00:00:02,000\nPart one.', { timeout: 15000 });
  await expect(out).toContainText('2\n00:00:03,000 --> 00:00:04,000\nPart two.');
});

test('subtitle-merge can force WebVTT output', async ({ page }) => {
  await page.goto('/tools/subtitle-merge/');
  await page.selectOption('#in-format', 'vtt');
  await setSubs(page, overlay);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('WEBVTT', { timeout: 15000 });
  await expect(out).toContainText('00:00:01.000 --> 00:00:02.000');
});

test('subtitle-merge query-param deep-link pre-fills and computes', async ({ page }) => {
  await page.goto('/tools/subtitle-merge/?subtitles=' + encodeURIComponent(splitParts) + '&offset_ms=2000&format=srt');
  await expect(page.locator('#in-subtitles')).toHaveValue(splitParts, { timeout: 15000 });
  await expect(page.locator('#in-offset_ms')).toHaveValue('2000');
  await expect(page.locator('#in-format')).toHaveValue('srt');
  await expect(page.locator('#tool-output')).toContainText('00:00:03,000 --> 00:00:04,000', { timeout: 15000 });
});
