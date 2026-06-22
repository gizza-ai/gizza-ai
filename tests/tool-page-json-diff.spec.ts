import { test, expect } from './fixtures';

// /tools/json-diff/ structurally diffs two JSON documents in-browser (pure wasm).
test('json-diff reports changed and added (minified)', async ({ page }) => {
  await page.goto('/tools/json-diff/');
  await page.fill('#in-left', '{"a":1,"b":2}');
  await page.fill('#in-right', '{"a":1,"b":9,"c":3}');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"equal":false,"added":1,"removed":0,"changed":1,"changes":[{"path":"$.b","kind":"changed","old":2,"new":9},{"path":"$.c","kind":"added","new":3}]}',
    { timeout: 15000 }
  );
});

test('json-diff reports equal for identical documents', async ({ page }) => {
  await page.goto('/tools/json-diff/');
  await page.fill('#in-left', '{"x":[1,2,3]}');
  await page.fill('#in-right', '{"x":[1,2,3]}');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"equal":true,"added":0,"removed":0,"changed":0,"changes":[]}',
    { timeout: 15000 }
  );
});
