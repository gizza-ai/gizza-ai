import { test, expect } from './fixtures';

// /tools/xml-formatter/ formats XML in-browser (pure wasm). xml is a multiline
// <textarea>; mode is a <select>; indent is a field.
test('xml-formatter page pretty-prints XML', async ({ page }) => {
  await page.goto('/tools/xml-formatter/');
  await page.fill('#in-xml', '<a><b>hi</b><c/></a>');
  await page.selectOption('#in-mode', 'pretty');
  await page.fill('#in-indent', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<b>hi</b>', { timeout: 15000 });
  const txt = (await out.textContent())!;
  expect(txt).toContain('\n  <b>hi</b>'); // indented
});

test('xml-formatter page reports malformed XML', async ({ page }) => {
  await page.goto('/tools/xml-formatter/');
  await page.fill('#in-xml', '<a><b></a>');
  await page.selectOption('#in-mode', 'pretty');
  await page.fill('#in-indent', '2');
  await expect(page.locator('#tool-output')).toContainText('not well-formed', { timeout: 15000 });
});
