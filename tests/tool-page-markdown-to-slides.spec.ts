import { test, expect } from './fixtures';

test('markdown-to-slides page builds a deck', async ({ page }) => {
  await page.goto('/tools/markdown-to-slides/');
  await page.fill('#in-markdown', '# Hello\n\nIntro\n\n---\n\n## Second\n\n- a\n- b');
  await page.selectOption('#in-theme', 'dark');
  await page.fill('#in-title', 'My Talk');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<title>My Talk</title>', { timeout: 15000 });
  await expect(out).toContainText('--bg: #1e1e2e', { timeout: 15000 });
  await expect(out).toContainText('1</span> / 2', { timeout: 15000 });
  await expect(out).toContainText('<h1>Hello</h1>', { timeout: 15000 });
});

test('markdown-to-slides query-param deep-link', async ({ page }) => {
  await page.goto(
    '/tools/markdown-to-slides/?markdown=' + encodeURIComponent('# One\n\n---\n\n# Two')
  );
  await expect(page.locator('#in-markdown')).toHaveValue('# One\n\n---\n\n# Two', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('<h1>Two</h1>', { timeout: 15000 });
});
