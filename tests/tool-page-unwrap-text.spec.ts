import { test, expect } from './fixtures';

// /tools/unwrap-text/ rejoins hard-wrapped lines in-browser (pure wasm).
// text is a multiline <textarea>; keep_list_breaks is a checkbox (default on).
test('unwrap-text page rejoins a wrapped paragraph', async ({ page }) => {
  await page.goto('/tools/unwrap-text/');
  await page.fill('#in-text', 'This is a paragraph that was\nhard wrapped across\nseveral lines.');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('This is a paragraph that was hard wrapped across several lines.', {
    timeout: 15000,
  });
});

test('unwrap-text page keeps list items separate (default)', async ({ page }) => {
  await page.goto('/tools/unwrap-text/');
  await page.fill('#in-text', 'Shopping:\n- milk\n- eggs');
  await expect(page.locator('#tool-output')).toContainText('- milk\n- eggs', { timeout: 15000 });
});
