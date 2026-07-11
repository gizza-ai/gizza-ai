import { test, expect } from './fixtures';

const sampleLog = [
  '9:00-12:30 Acme kickoff call',
  '13:00-17:15 Acme build feature',
  '2024-01-15 10:00-11:00 #Beta review',
].join('\n');

test('timesheet-calculator page totals projects and billable amount', async ({ page }) => {
  await page.goto('/tools/timesheet-calculator/');
  await page.fill('#in-log', sampleLog);
  await page.fill('#in-rate', '100');
  await page.fill('#in-currency', '$');
  await page.selectOption('#in-round', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"project": "Acme"', { timeout: 15000 });
  await expect(out).toContainText('"minutes": 465');
  await expect(out).toContainText('"project": "Beta"');
  await expect(out).toContainText('"total_hours": 8.75');
  await expect(out).toContainText('"total_amount": 875');
  await expect(out).toContainText('3 entries across 2 project(s) · 8.75 h · $875.00');
});

test('timesheet-calculator query-param deep-link prefills and computes rounding', async ({ page }) => {
  await page.goto(
    '/tools/timesheet-calculator/?log=' +
      encodeURIComponent('9:00-9:04 Smith intake call') +
      '&rate=300&round=6&currency=' +
      encodeURIComponent('$'),
  );

  await expect(page.locator('#in-log')).toHaveValue('9:00-9:04 Smith intake call', {
    timeout: 15000,
  });
  await expect(page.locator('#in-rate')).toHaveValue('300');
  await expect(page.locator('#in-round')).toHaveValue('6');
  await expect(page.locator('#tool-output')).toContainText('"minutes": 6', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"total_amount": 30');
});
