import { test, expect } from './fixtures';

const tool = '/tools/regex-bulk-match/';

test('regex-bulk-match page reports match and no-match per line', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-lines', 'ada@example.com\nnot-an-email\nbo@test.org');
  await page.fill('#in-pattern', '^[\\w.+-]+@([\\w-]+\\.[\\w.]+)$');
  await page.check('#in-full_match');
  await expect(page.locator('#tool-output')).toContainText('Lines tested: 3', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Matched: 2');
  await expect(page.locator('#tool-output')).toContainText('line 2: NO MATCH "not-an-email"');
  await expect(page.locator('#tool-output')).toContainText('1=example.com');
});

test('regex-bulk-match page filters non-matching rows', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-lines', 'AB-123\nwrong\nCD-456');
  await page.fill('#in-pattern', '^[A-Z]{2}-\\d{3}$');
  await page.check('#in-full_match');
  await page.selectOption('#in-show', 'non-matching');
  await expect(page.locator('#tool-output')).toContainText('Matched: 2', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('line 2: NO MATCH "wrong"');
  await expect(page.locator('#tool-output')).not.toContainText('line 1: MATCH');
});

test('regex-bulk-match page emits csv with offsets and captures', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-lines', 'error: disk full\ninfo: ok');
  await page.fill('#in-pattern', '^([a-z]+):\\s*(.*)$');
  await page.check('#in-show_position');
  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toHaveText(
    'line,text,matched,match,start,end,group_1,group_2\n1,error: disk full,true,error: disk full,0,16,error,disk full\n2,info: ok,true,info: ok,0,8,info,ok',
    { timeout: 15000 },
  );
});

test('regex-bulk-match query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    tool +
      '?lines=' +
      encodeURIComponent('WARN disk\ninfo ok\nWarning cpu') +
      '&pattern=' +
      encodeURIComponent('warn(ing)?') +
      '&ignore_case=true&show=matching',
  );
  await expect(page.locator('#in-lines')).toHaveValue('WARN disk\ninfo ok\nWarning cpu', {
    timeout: 15000,
  });
  await expect(page.locator('#in-pattern')).toHaveValue('warn(ing)?');
  await expect(page.locator('#in-ignore_case')).toBeChecked();
  await expect(page.locator('#in-show')).toHaveValue('matching');
  await expect(page.locator('#tool-output')).toContainText('Matched: 2');
  await expect(page.locator('#tool-output')).toContainText('line 1: MATCH');
  await expect(page.locator('#tool-output')).toContainText('line 3: MATCH');
});
