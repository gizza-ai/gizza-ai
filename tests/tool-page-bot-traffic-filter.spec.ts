import { test, expect } from './fixtures';

const UA_LIST = 'Mozilla/5.0 Safari/605.1\nGooglebot/2.1\npython-requests/2.31.0';

const REPORT = `Bot traffic report
==================
Total hits:  3
Human:       1 (33.3%)
Bot:         2 (66.7%)

Bots by category:
  Search engines           1
  HTTP libraries / scripts 1

Top bots:
  Googlebot                1
  python-requests          1`;

test('bot-traffic-filter reports human vs bot split for a user-agent list', async ({ page }) => {
  await page.goto('/tools/bot-traffic-filter/');
  await page.fill('#in-input', UA_LIST);
  await page.selectOption('#in-format', 'plain');
  await page.selectOption('#in-output', 'report');

  await expect(page.locator('#tool-output')).toHaveText(REPORT, { timeout: 15000 });
});

test('bot-traffic-filter deep-link strips bots and keeps only human lines', async ({ page }) => {
  await page.goto(
    '/tools/bot-traffic-filter/?input=' +
      encodeURIComponent(UA_LIST) +
      '&format=plain&output=humans&empty_is_bot=true&limit=500',
  );

  await expect(page.locator('#in-input')).toHaveValue(UA_LIST, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('Mozilla/5.0 Safari/605.1', {
    timeout: 15000,
  });
});

test('bot-traffic-filter treats missing UA as human when checkbox is off', async ({ page }) => {
  await page.goto('/tools/bot-traffic-filter/');
  await page.fill('#in-input', '-\nMozilla/5.0');
  await page.selectOption('#in-format', 'plain');
  await page.selectOption('#in-output', 'table');
  await page.uncheck('#in-empty_is_bot');
  await page.fill('#in-limit', '2');

  await expect(page.locator('#tool-output')).toHaveText(
    `2 hits · 2 human (100.0%) · 0 bot (0.0%)

| # | Class | Category | Bot | User-Agent |
| --- | --- | --- | --- | --- |
| 1 | human | - | - | - |
| 2 | human | - | - | Mozilla/5.0 |`,
    { timeout: 15000 },
  );
});
