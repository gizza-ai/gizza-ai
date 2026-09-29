import { test, expect } from './fixtures';

test('json-to-messagepack renders default hex output', async ({ page }) => {
  await page.goto('/tools/json-to-messagepack/');
  await page.fill('#in-input', '{"a":1,"b":2}');
  await page.selectOption('#in-output', 'hex');
  await page.selectOption('#in-key_order', 'input');
  await page.selectOption('#in-spec', 'new');
  await page.fill('#in-group', '0');

  await expect(page.locator('#tool-output')).toContainText('82a16101a16202', { timeout: 15000 });
});

test('json-to-messagepack honors deep-linked sorted summary and compact floats', async ({ page }) => {
  const params = new URLSearchParams({
    input: '{"b":2,"a":1.5}',
    output: 'summary',
    key_order: 'sorted',
    compact_floats: 'true',
    spec: 'new',
    group: '2',
  });
  await page.goto(`/tools/json-to-messagepack/?${params.toString()}`);

  await expect(page.locator('#in-compact_floats')).toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('MessagePack bytes:', { timeout: 15000 });
  await expect(out).toContainText('Hex: 82a1 61ca 3fc0 0000 a162 02');
});

test('json-to-messagepack exercises advertised output and spec modes', async ({ page }) => {
  await page.goto('/tools/json-to-messagepack/');
  await page.fill('#in-input', '[1,true,null,"x"]');
  await page.selectOption('#in-output', 'base64');
  await expect(page.locator('#tool-output')).toContainText('lAHDwKF4', { timeout: 15000 });

  await page.selectOption('#in-output', 'bytes');
  await expect(page.locator('#tool-output')).toContainText('[148, 1, 195, 192, 161, 120]', { timeout: 15000 });

  await page.selectOption('#in-output', 'annotated');
  await expect(page.locator('#tool-output')).toContainText('fixarray', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('positive fixint');

  const long = 'x'.repeat(40);
  await page.fill('#in-input', JSON.stringify(long));
  await page.selectOption('#in-output', 'hex');
  await page.selectOption('#in-spec', 'old');
  await expect(page.locator('#tool-output')).toContainText('da0028', { timeout: 15000 });

  await page.selectOption('#in-output', 'json');
  await expect(page.locator('#tool-output')).toContainText('"encoding":"messagepack"', { timeout: 15000 });
});
