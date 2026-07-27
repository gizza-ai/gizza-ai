import { test, expect } from './fixtures';

test('value-counts page counts a CSV column with percentages', async ({ page }) => {
  await page.goto('/tools/value-counts/');
  await page.fill('#in-data', 'fruit\napple\nbanana\napple\ncherry\napple\nbanana');
  await page.fill('#in-column', 'fruit');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('apple,3,50%', { timeout: 15_000 });
  expect(await out.textContent()).toBe('value,count,percent\napple,3,50%\nbanana,2,33.33%\ncherry,1,16.67%\n');
});

test('value-counts deep link supports tab delimiter and column index', async ({ page }) => {
  const data = 'id\tcolor\n1\tred\n2\tblue\n3\tred';
  const qs =
    '?data=' + encodeURIComponent(data) +
    '&column=2' +
    '&delimiter=tab';
  await page.goto('/tools/value-counts/' + qs);

  await expect(page.locator('#in-column')).toHaveValue('2', { timeout: 15_000 });
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('red\t2\t66.67%', { timeout: 15_000 });
  expect(await out.textContent()).toBe('value\tcount\tpercent\nred\t2\t66.67%\nblue\t1\t33.33%\n');
});

test('value-counts handles case-insensitive grouping and empty cells', async ({ page }) => {
  await page.goto('/tools/value-counts/');
  await page.fill('#in-data', 'status\nOpen\nopen\n,\nClosed\nOPEN');
  await page.fill('#in-column', 'status');
  await page.uncheck('#in-case_sensitive');
  await page.check('#in-include_empty');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Open,3,60%', { timeout: 15_000 });
  expect(await out.textContent()).toBe('value,count,percent\nOpen,3,60%\n(empty),1,20%\nClosed,1,20%\n');
});

test('value-counts can sort by value', async ({ page }) => {
  await page.goto('/tools/value-counts/');
  await page.fill('#in-data', 'c\nb\na\nb\na\na');
  await page.fill('#in-column', 'c');
  await page.selectOption('#in-sort', 'value');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('a,3,60%', { timeout: 15_000 });
  expect(await out.textContent()).toBe('value,count,percent\na,3,60%\nb,2,40%\n');
});
