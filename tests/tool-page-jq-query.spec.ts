import { test, expect } from './fixtures';

// /tools/jq-query/ runs a jq filter over JSON in-browser (pure-Rust jaq).
test('jq-query page applies a filter to JSON', async ({ page }) => {
  await page.goto('/tools/jq-query/');
  await page.fill('#in-query', '.users | map(.name) | join(", ")');
  await page.fill('#in-json', '{"users":[{"name":"Ada"},{"name":"Bo"}]}');
  await expect(page.locator('#tool-output')).toContainText('Ada, Bo', { timeout: 15000 });
});

test('jq-query page deep-link with array iteration', async ({ page }) => {
  const qs = '?query=' + encodeURIComponent('.[] | . * 2') + '&json=' + encodeURIComponent('[1,2,3]');
  await page.goto('/tools/jq-query/' + qs);
  await expect(page.locator('#in-query')).toHaveValue('.[] | . * 2', { timeout: 15000 });
  // three outputs, one per line
  await expect(page.locator('#tool-output')).toContainText('2', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('6');
});
