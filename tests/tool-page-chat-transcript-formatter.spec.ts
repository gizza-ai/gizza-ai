import { test, expect } from './fixtures';

test('chat-transcript-formatter normalizes mixed chat lines to exact markdown output', async ({ page }) => {
  await page.goto('/tools/chat-transcript-formatter/');
  await page.fill('#in-input', '[2023-01-05, 10:04:11] Alice: hey, running late\n[2023-01-05, 10:04:40] Alice: maybe 10 mins\n[2023-01-05, 10:05:02] Bob: no worries');
  await page.selectOption('#in-output_format', 'markdown');
  await page.selectOption('#in-time_format', '24h');
  await page.check('#in-merge_consecutive');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('[10:04:11] **Alice:** hey, running late maybe 10 mins', { timeout: 15000 });
  await expect(out).toContainText('[10:05:02] **Bob:** no worries');
});

test('chat-transcript-formatter supports non-default toggles and screenplay output', async ({ page }) => {
  await page.goto('/tools/chat-transcript-formatter/');
  await page.fill('#in-input', '05/01/2023, 10:04 AM - Alice: hello\n05/01/2023, 10:05 AM - Bob: hey');
  await page.selectOption('#in-output_format', 'screenplay');
  await page.selectOption('#in-time_format', '12h');
  await page.check('#in-include_dates');
  await page.check('#in-blank_line_between');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('[05/01/2023 10:04 AM] ALICE: hello', { timeout: 15000 });
  await expect(out).toContainText('[05/01/2023 10:05 AM] BOB: hey');
  const text = await out.innerText();
  expect(text).toContain('hello\n\n[05/01/2023 10:05 AM]');
});

test('chat-transcript-formatter supports deep-linked bracketed no-time output', async ({ page }) => {
  const params = new URLSearchParams({
    input: '10:04 Alice: hi everyone\n<Bob> hey Alice\nCarol: joining now\ngot pulled into a meeting',
    output_format: 'bracketed',
    time_format: 'none',
    blank_line_between: 'true',
  });
  await page.goto(`/tools/chat-transcript-formatter/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<Alice> hi everyone', { timeout: 15000 });
  await expect(out).toContainText('<Bob> hey Alice');
  await expect(out).toContainText('<Carol> joining now got pulled into a meeting');
});
