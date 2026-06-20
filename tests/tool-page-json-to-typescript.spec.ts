import { test, expect } from './fixtures';

// /tools/json-to-typescript/ infers TS interfaces from JSON in-browser (pure wasm).
test('json-to-typescript page infers an interface', async ({ page }) => {
  await page.goto('/tools/json-to-typescript/');
  await page.fill('#in-json', '{"name":"Ada","age":30}');
  await page.fill('#in-root_name', 'User');
  await expect(page.locator('#tool-output')).toContainText('interface User', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('name: string;');
  await expect(page.locator('#tool-output')).toContainText('age: number;');
});

test('json-to-typescript page deep-link infers optional field', async ({ page }) => {
  const qs = '?json=' + encodeURIComponent('{"items":[{"a":1},{"a":2,"b":"x"}]}') + '&root_name=Root';
  await page.goto('/tools/json-to-typescript/' + qs);
  await expect(page.locator('#tool-output')).toContainText('b?: string;', { timeout: 15000 });
});
