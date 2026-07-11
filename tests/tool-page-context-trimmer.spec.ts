import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const SENT = 'the quick brown fox jumps over the lazy dog';

test('context-trimmer page — default-ish head trim keeps the beginning', async ({ page }) => {
  await page.goto('/tools/context-trimmer/');
  await page.fill('#in-text', SENT);
  await page.fill('#in-max_tokens', '3');
  await page.fill('#in-chars_per_token', '4.0');
  await page.selectOption('#in-keep', 'head');
  await page.fill('#in-marker', '…');
  await expect(page.locator('#tool-output')).toContainText('the quick', { timeout: 15000 });
  expect(await outputText(page)).toBe('the quick…');
});

test('context-trimmer page — head_tail keeps both ends', async ({ page }) => {
  await page.goto('/tools/context-trimmer/');
  await page.fill('#in-text', SENT);
  await page.fill('#in-max_tokens', '5');
  await page.fill('#in-chars_per_token', '4.0');
  await page.selectOption('#in-keep', 'head_tail');
  await page.fill('#in-marker', '…');
  await page.fill('#in-head_ratio', '0.5');
  await expect(page.locator('#tool-output')).toContainText('lazy dog', { timeout: 15000 });
  expect(await outputText(page)).toBe('the quick…lazy dog');
});

test('context-trimmer page — query-param deep-link and break_words hard cut', async ({ page }) => {
  const url =
    '/tools/context-trimmer/?text=' +
    encodeURIComponent(SENT) +
    '&max_tokens=3&chars_per_token=4.0&keep=head&marker=&head_ratio=0.5&break_words=true';
  await page.goto(url);
  await expect(page.locator('#in-text')).toHaveValue(SENT, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('the quick br', { timeout: 15000 });
  expect(await outputText(page)).toBe('the quick br');
});
