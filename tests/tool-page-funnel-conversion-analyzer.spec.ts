import { test, expect } from './fixtures';

const CSV = 'user,event\nu1,view\nu1,signup\nu1,purchase\nu2,view\nu2,signup\nu3,view';

test('funnel-conversion-analyzer page reports conversion and drop-off table', async ({ page }) => {
  await page.goto('/tools/funnel-conversion-analyzer/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-steps', 'view,signup,purchase');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Funnel: 3 step(s), 3 total user(s)', { timeout: 15000 });
  await expect(out).toContainText('1. view: 3 users | 100% of top');
  await expect(out).toContainText('2. signup: 2 users | 66.67% of top | 66.67% from prev | drop 1 (33.33%)');
  await expect(out).toContainText('3. purchase: 1 users | 33.33% of top | 50% from prev | drop 1 (50%)');
  await expect(out).toContainText('Overall conversion: 33.33%');
});

test('funnel-conversion-analyzer honors deep-link params and JSON output', async ({ page }) => {
  const qs =
    '?data=' + encodeURIComponent(CSV + '\nu4,view\nu4,purchase') +
    '&steps=' + encodeURIComponent('view,signup,purchase') +
    '&ordered=false&header=true&delimiter=comma&format=json';
  await page.goto('/tools/funnel-conversion-analyzer/' + qs);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"total_users": 4', { timeout: 15000 });
  await expect(out).toContainText('"step": "purchase"');
  await expect(out).toContainText('"users": 2');
  await expect(out).toContainText('"overall_conversion": 50');
});
