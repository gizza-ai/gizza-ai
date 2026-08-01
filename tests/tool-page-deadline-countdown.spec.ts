import { test, expect } from './fixtures';

test('deadline-countdown sorts overdue and upcoming tasks with real output', async ({ page }) => {
  await page.goto('/tools/deadline-countdown/');
  await page.fill('#in-tasks', 'Submit taxes due: 2026-07-30\nShip launch due: 2026-07-31 16:00\nRenew cert due: 2026-08-05\nx Done item due: 2026-07-01');
  await page.fill('#in-now', '2026-07-31 12:00');
  await page.selectOption('#in-format', 'table');
  await page.fill('#in-soon_days', '7');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('OVERDUE', { timeout: 15000 });
  await expect(out).toContainText('Submit taxes');
  await expect(out).toContainText('DUE TODAY');
  await expect(out).toContainText('Ship launch');
  await expect(out).toContainText('in 4h');
  await expect(out).toContainText('Renew cert');
  await expect(out).toContainText('skipped 1 completed task');
  const text = await out.innerText();
  expect(text.indexOf('Submit taxes')).toBeLessThan(text.indexOf('Ship launch'));
  expect(text.indexOf('Ship launch')).toBeLessThan(text.indexOf('Renew cert'));
});

test('deadline-countdown emits markdown and can include completed tasks', async ({ page }) => {
  await page.goto('/tools/deadline-countdown/');
  await page.fill('#in-tasks', 'x Done item due: 2026-07-01\nBook QA review due: 2026-07-31 17:30');
  await page.fill('#in-now', '2026-07-31 12:00');
  await page.selectOption('#in-format', 'markdown');
  await page.check('#in-include_completed');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('| Status | Due | Remaining | Task |', { timeout: 15000 });
  await expect(out).toContainText('Done item');
  await expect(out).toContainText('Book QA review');
  await expect(out).toContainText('in 5h 30m');
});

test('deadline-countdown supports deep-linked csv output', async ({ page }) => {
  const params = new URLSearchParams({
    tasks: 'Call vendor 2026-08-01\nRenew cert 2026-08-05',
    now: '2026-07-31',
    format: 'csv',
    soon_days: '2',
  });
  await page.goto(`/tools/deadline-countdown/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('status,due,remaining,total_minutes,task', { timeout: 15000 });
  await expect(out).toContainText('DUE SOON,2026-08-01,in 1d,1440,"Call vendor"');
  await expect(out).toContainText('LATER,2026-08-05,in 5d,7200,"Renew cert"');
});
