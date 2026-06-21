import { test, expect } from './fixtures';

test('todo-organizer page groups tasks by priority', async ({ page }) => {
  await page.goto('/tools/todo-organizer/');
  await page.fill('#in-text', 'Call the bank urgently\nBuy milk\nClean garage someday');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('High priority', { timeout: 15000 });
  await expect(out).toContainText('- [ ] Call the bank urgently');
  await expect(out).toContainText('Low priority');
  await expect(out).toContainText('- [ ] Clean garage someday');
});

test('todo-organizer query-param deep-link with due hint', async ({ page }) => {
  await page.goto('/tools/todo-organizer/?text=' + encodeURIComponent('Submit report by Friday'));
  await expect(page.locator('#in-text')).toHaveValue('Submit report by Friday', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('_(due: Friday)_', { timeout: 15000 });
});

test('todo-organizer numbered + none grouping', async ({ page }) => {
  await page.goto('/tools/todo-organizer/');
  await page.fill('#in-text', 'urgent a\nlater b');
  await page.selectOption('#in-group_by', 'none');
  await page.check('#in-numbered');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1. urgent a', { timeout: 15000 });
  await expect(out).toContainText('2. later b');
  await expect(out).not.toContainText('priority');
});
