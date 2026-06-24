import { test, expect } from './fixtures';

// /tools/timezone-convert/ converts a wall-clock time between IANA zones (pure wasm).
test('timezone-convert converts New York to Tokyo', async ({ page }) => {
  await page.goto('/tools/timezone-convert/');
  await page.fill('#in-datetime', '2024-01-10 14:30');
  await page.fill('#in-from', 'America/New_York');
  await page.fill('#in-to', 'Asia/Tokyo');
  
  const out = page.locator('#tool-output');
  // Check that the beautiful target card is rendered
  await expect(out.locator('.tz-card-zone')).toContainText('Asia/Tokyo', { timeout: 15000 });
  await expect(out.locator('.tz-card-time')).toContainText('04:30:00');
  await expect(out.locator('.tz-card-date')).toContainText('Thu, 11 Jan 2024');
  
  // Check badges
  await expect(out.locator('.tz-badge-offset')).toContainText('+09:00');
  await expect(out.locator('.tz-badge-diff')).toContainText('+14h');
  
  // Check planner title and table
  await expect(out.locator('.tz-planner-title')).toContainText('Meeting Planner Grid');
  await expect(out.locator('.tz-planner-table')).toBeVisible();
});

test('timezone-convert handles a half-hour zone (India)', async ({ page }) => {
  await page.goto('/tools/timezone-convert/');
  await page.fill('#in-datetime', '2024-06-01 00:00');
  await page.fill('#in-from', 'UTC');
  await page.fill('#in-to', 'Asia/Kolkata');
  
  const out = page.locator('#tool-output');
  await expect(out.locator('.tz-card-zone')).toContainText('Asia/Kolkata', { timeout: 15000 });
  await expect(out.locator('.tz-card-time')).toContainText('05:30:00');
  await expect(out.locator('.tz-badge-diff')).toContainText('+5.5h');
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
  
  const out = page.locator('#tool-output');
  await expect(out.locator('.tz-card-zone')).toContainText('Asia/Tokyo', { timeout: 15000 });
  await expect(out.locator('.tz-card-time')).toContainText('04:30:00');
});

test('timezone-convert supports lenient parsing and multi-target planner', async ({ page }) => {
  await page.goto('/tools/timezone-convert/');
  
  // Test slash and AM/PM parsing
  await page.fill('#in-datetime', '2024/03/10 2:30 PM');
  await page.fill('#in-from', 'America/New_York');
  await page.fill('#in-to', 'Asia/Tokyo, Europe/London');
  
  const out = page.locator('#tool-output');
  
  // Both target cards should be visible
  await expect(out.locator('.tz-card-zone').nth(0)).toContainText('Asia/Tokyo', { timeout: 15000 });
  await expect(out.locator('.tz-card-zone').nth(1)).toContainText('Europe/London');
  
  // London time should be 18:30:00
  await expect(out.locator('.tz-card-time').nth(1)).toContainText('18:30:00');
  
  // Check the table has column headers for both targets
  const table = out.locator('.tz-planner-table');
  await expect(table.locator('th').nth(0)).toContainText('America/New_York');
  await expect(table.locator('th').nth(1)).toContainText('Asia/Tokyo');
  await expect(table.locator('th').nth(2)).toContainText('Europe/London');
  
  // The row corresponding to source 14:30 (hour 14) should be highlighted
  const selectedRow = table.locator('.tz-planner-row.selected');
  await expect(selectedRow.locator('td').nth(0)).toContainText('14:00'); // Midnight base to hourly index
});

test('timezone-convert page has smart defaults on load', async ({ page }) => {
  await page.goto('/tools/timezone-convert/');
  
  // Datetime input should be pre-filled and match the format YYYY-MM-DD HH:MM
  const dtVal = await page.inputValue('#in-datetime');
  expect(dtVal).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
  
  // From input should default to browser timezone or fallback
  const fromVal = await page.inputValue('#in-from');
  expect(fromVal.length).toBeGreaterThan(0);
  
  // To input should default to UTC
  const toVal = await page.inputValue('#in-to');
  expect(toVal).toBe('UTC');
  
  // The targets grid/card and planner table should render automatically without clicking run
  const out = page.locator('#tool-output');
  await expect(out.locator('.tz-card-zone')).toContainText('UTC', { timeout: 15000 });
  await expect(out.locator('.tz-card-time')).toBeVisible();
  await expect(out.locator('.tz-planner-table')).toBeVisible();
});

