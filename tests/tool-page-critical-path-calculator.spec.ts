import { test, expect } from './fixtures';

const sampleTasks = 'A, 3\nB, 4, A\nC, 2, A\nD, 5, B, C\nE, 1, D';

test('critical-path-calculator reports project duration and critical path', async ({ page }) => {
  await page.goto('/tools/critical-path-calculator/');
  await page.fill('#in-tasks', sampleTasks);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Project duration: 13', { timeout: 15000 });
  await expect(out).toContainText('Critical path: A -> B -> D -> E');
  // C is the only non-critical task, carrying 2 units of total float.
  await expect(out).toContainText('C');
});

test('format=json emits machine-readable schedule', async ({ page }) => {
  await page.goto('/tools/critical-path-calculator/');
  await page.fill('#in-tasks', sampleTasks);
  await page.selectOption('#in-format', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"project_duration": 13', { timeout: 15000 });
  await expect(out).toContainText('"critical_path": ["A", "B", "D", "E"]');
});

test('query-param deep-link prefills and runs', async ({ page }) => {
  await page.goto(
    '/tools/critical-path-calculator/?tasks=' +
      encodeURIComponent(sampleTasks) +
      '&format=json',
  );
  await expect(page.locator('#in-tasks')).toHaveValue(sampleTasks, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"project_duration": 13', { timeout: 15000 });
  await expect(out).toContainText('"critical_path": ["A", "B", "D", "E"]');
});
