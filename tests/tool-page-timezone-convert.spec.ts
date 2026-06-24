import { test, expect } from './fixtures';

// /tools/timezone-convert/ converts a wall-clock time between IANA zones (pure wasm).
test('timezone-convert converts New York to Tokyo', async ({ page }) => {
  await page.goto('/tools/timezone-convert/');
  await page.fill('#in-datetime', '2024-01-10 14:30');
  await page.fill('#in-from', 'America/New_York');
  await page.fill('#in-to', 'Asia/Tokyo');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2024-01-11T04:30:00+09:00', { timeout: 15000 });
  await expect(out).toContainText('-05:00'); // source offset (EST)
  await expect(out).toContainText('+09:00'); // target offset (JST)
  await expect(out).toContainText('Thursday'); // target weekday
});

test('timezone-convert handles a half-hour zone (India)', async ({ page }) => {
  await page.goto('/tools/timezone-convert/');
  await page.fill('#in-datetime', '2024-06-01 00:00');
  await page.fill('#in-from', 'UTC');
  await page.fill('#in-to', 'Asia/Kolkata');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2024-06-01T05:30:00+05:30', { timeout: 15000 });
  await expect(out).toContainText('330'); // offset diff minutes
});

test('timezone-convert rejects an unknown zone', async ({ page }) => {
  await page.goto('/tools/timezone-convert/');
  await page.fill('#in-datetime', '2024-01-10 14:30');
  await page.fill('#in-from', 'Mars/Olympus');
  await page.fill('#in-to', 'UTC');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('unknown source timezone', { timeout: 15000 });
});

test('timezone-convert query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/timezone-convert/?datetime=' +
      encodeURIComponent('2024-01-10 14:30') +
      '&from=' + encodeURIComponent('America/New_York') +
      '&to=' + encodeURIComponent('Asia/Tokyo'),
  );
  await expect(page.locator('#in-datetime')).toHaveValue('2024-01-10 14:30', {
    timeout: 15000,
  });
  await expect(page.locator('#in-to')).toHaveValue('Asia/Tokyo');
  await expect(page.locator('#tool-output')).toContainText(
    '2024-01-11T04:30:00+09:00',
    { timeout: 15000 },
  );
});
