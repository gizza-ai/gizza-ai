import { test, expect } from './fixtures';

// /tools/find-replace/ runs find & replace in-browser (pure wasm, recomputes on
// input). Literal mode escapes regex metacharacters; regex mode supports $1/$2.
test('find-replace page does a literal replace (metachars escaped)', async ({ page }) => {
  await page.goto('/tools/find-replace/');
  await page.fill('#in-text', 'a.b.c');
  await page.fill('#in-find', '.');
  await page.fill('#in-replace', '-');
  // Default literal mode: only the real dots match → a-b-c.
  await expect(page.locator('#tool-output')).toHaveText('a-b-c', { timeout: 15000 });
});

test('find-replace page supports regex capture groups via deep-link', async ({ page }) => {
  const qs =
    '?text=' + encodeURIComponent('John Smith') +
    '&find=' + encodeURIComponent('(\\w+) (\\w+)') +
    '&replace=' + encodeURIComponent('$2 $1') +
    '&regex=true';
  await page.goto('/tools/find-replace/' + qs);
  await expect(page.locator('#in-text')).toHaveValue('John Smith', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('Smith John', { timeout: 15000 });
});
