import { test, expect } from './fixtures';

const sample = String.raw`{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}\pard The quick \b brown\b0  fox.\par Caf\'e9 costs 5\'80.\par}`;

test('rtf-to-text page strips controls and decodes escapes', async ({ page }) => {
  await page.goto('/tools/rtf-to-text/');
  await page.fill('#in-rtf', sample);

  await expect(page.locator('#tool-output')).toHaveText('The quick brown fox.\nCafé costs 5€.', { timeout: 15000 });
});

test('rtf-to-text deep-link collapses paragraph breaks', async ({ page }) => {
  const rtf = String.raw`{\rtf1\ansi First line.\par Second line.\par}`;
  await page.goto('/tools/rtf-to-text/?rtf=' + encodeURIComponent(rtf) + '&line_breaks=collapse');

  await expect(page.locator('#in-rtf')).toHaveValue(rtf, { timeout: 15000 });
  await expect(page.locator('#in-line_breaks')).toHaveValue('collapse');
  await expect(page.locator('#tool-output')).toHaveText('First line. Second line.', { timeout: 15000 });
});


