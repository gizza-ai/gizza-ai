import { test, expect } from './fixtures';

test('html-to-markdown page converts HTML', async ({ page }) => {
  await page.goto('/tools/html-to-markdown/');
  await page.fill('#in-html', '<h1>Title</h1><p>Hello <b>world</b></p>');
  await expect(page.locator('#tool-output')).toHaveText(/# Title[\s\S]*\*\*world\*\*/, {
    timeout: 15000,
  });
});

test('html-to-markdown page preserves links via query-param deep-link', async ({ page }) => {
  await page.goto(
    '/tools/html-to-markdown/?html=' +
      encodeURIComponent('<a href="https://x.test">link</a>'),
  );
  await expect(page.locator('#in-html')).toHaveValue('<a href="https://x.test">link</a>', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toHaveText('[link](https://x.test)', {
    timeout: 15000,
  });
});
