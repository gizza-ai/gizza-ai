import { test, expect } from './fixtures';

test('natural-language-task page converts an urgent dated task to exact todo.txt', async ({ page }) => {
  await page.goto('/tools/natural-language-task/');
  await page.fill('#in-text', 'Call the plumber urgent tomorrow +house @phone');
  await page.fill('#in-reference_date', '2026-07-28');

  await expect(page.locator('#tool-output')).toHaveText(
    '(A) 2026-07-28 Call the plumber +house @phone due:2026-07-29',
    { timeout: 15_000 },
  );
});

test('natural-language-task deep link disables priority and adds default tags', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'urgent file receipts next Friday',
    reference_date: '2026-07-28',
    detect_priority: 'false',
    project: 'admin work',
    context: 'desk',
  });

  await page.goto(`/tools/natural-language-task/?${params.toString()}`);
  await expect(page.locator('#in-detect_priority')).not.toBeChecked({ timeout: 15_000 });
  await expect(page.locator('#in-project')).toHaveValue('admin work');

  await expect(page.locator('#tool-output')).toHaveText(
    '2026-07-28 urgent file receipts +admin-work @desk due:2026-07-31',
    { timeout: 15_000 },
  );
});
