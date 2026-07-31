import { test, expect } from './fixtures';

test('task-format-converter converts todo.txt to exact Markdown checklist output', async ({ page }) => {
  await page.goto('/tools/task-format-converter/');
  await page.fill('#in-input', '(A) 2026-08-01 Draft release notes +work @desk due:2026-08-05');
  await page.selectOption('#in-from', 'todotxt');
  await page.selectOption('#in-to', 'markdown');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('- [ ] (A) Draft release notes +work @desk due:2026-08-05 created:2026-08-01', { timeout: 15000 });
});

test('task-format-converter converts Markdown checklist to JSON', async ({ page }) => {
  await page.goto('/tools/task-format-converter/');
  await page.fill('#in-input', '- [x] (B) Buy milk +errands @store due:2026-08-03');
  await page.selectOption('#in-from', 'markdown');
  await page.selectOption('#in-to', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"text": "Buy milk"', { timeout: 15000 });
  await expect(out).toContainText('"done": true');
  await expect(out).toContainText('"priority": "B"');
  await expect(out).toContainText('"due": "2026-08-03"');
});

test('task-format-converter supports deep-linked CSV to todo.txt', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'text,done,priority,projects,contexts,created,due,completed,tags\nCall supplier,false,A,work,phone,2026-08-01,2026-08-04,,est:15m',
    from: 'csv',
    to: 'todotxt',
    pretty: 'false',
  });
  await page.goto(`/tools/task-format-converter/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('(A) 2026-08-01 Call supplier +work @phone due:2026-08-04 est:15m', { timeout: 15000 });
});
