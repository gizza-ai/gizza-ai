import { test, expect } from './fixtures';

const LIST = '# Launch\n- [x] write code\n- [x] write tests\n- [ ] update docs\n- [ ] ship it';

test('task-list-summarizer page summary counts done/pending', async ({ page }) => {
  await page.goto('/tools/task-list-summarizer/');
  await page.fill('#in-text', LIST);
  await expect(page.locator('#tool-output')).toHaveText(
    '4 tasks: 2 done, 2 pending (50% complete).',
    { timeout: 15000 },
  );
});

test('task-list-summarizer pending mode lists remaining tasks', async ({ page }) => {
  await page.goto('/tools/task-list-summarizer/');
  await page.fill('#in-text', LIST);
  await page.selectOption('#in-mode', 'pending');
  await expect(page.locator('#tool-output')).toHaveText('update docs\nship it', {
    timeout: 15000,
  });
});

test('task-list-summarizer json mode is machine-readable', async ({ page }) => {
  await page.goto('/tools/task-list-summarizer/');
  await page.fill('#in-text', '- [x] a\n- [ ] b');
  await page.selectOption('#in-mode', 'json');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"total":2,"done":1,"pending":1,"percent":50,"done_items":["a"],"pending_items":["b"]}',
    { timeout: 15000 },
  );
});

test('task-list-summarizer query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/task-list-summarizer/?text=' +
      encodeURIComponent('- [x] one\n- [ ] two\n- [ ] three') +
      '&mode=done',
  );
  await expect(page.locator('#in-mode')).toHaveValue('done', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('one', { timeout: 15000 });
});
