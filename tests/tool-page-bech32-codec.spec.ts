import { test, expect } from './fixtures';

test('bech32-codec page encodes hex data with a prefix (BIP 173)', async ({ page }) => {
  await page.goto('/tools/bech32-codec/');
  await page.fill('#in-input', '751e76e8199196d454941c45d1b3a323f1433bd6');
  await page.fill('#in-hrp', 'bc');
  await expect(page.locator('#tool-output')).toHaveText(
    'bc1w508d6qejxtdg4y5r3zarvary0c5xw7kj7gz7z',
    { timeout: 15000 },
  );
});

test('bech32-codec page decodes a Bech32 string back to its parts', async ({ page }) => {
  await page.goto('/tools/bech32-codec/');
  await page.fill('#in-input', 'bc1w508d6qejxtdg4y5r3zarvary0c5xw7kj7gz7z');
  await page.selectOption('#in-mode', 'decode');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('hrp: bc', { timeout: 15000 });
  await expect(out).toContainText('variant: bech32');
  await expect(out).toContainText('data: 751e76e8199196d454941c45d1b3a323f1433bd6');
});

test('bech32-codec page encodes UTF-8 text as Bech32m', async ({ page }) => {
  await page.goto('/tools/bech32-codec/');
  await page.fill('#in-input', 'hello');
  await page.fill('#in-hrp', 'test');
  await page.selectOption('#in-variant', 'bech32m');
  await page.selectOption('#in-format', 'text');
  await expect(page.locator('#tool-output')).toHaveText('test1dpjkcmr0scqr9j', { timeout: 15000 });
});

test('bech32-codec page reports a checksum error on a corrupted string', async ({ page }) => {
  await page.goto('/tools/bech32-codec/');
  await page.fill('#in-input', 'a12uel5m');
  await page.selectOption('#in-mode', 'decode');
  await expect(page.locator('#tool-output')).toContainText('checksum', { timeout: 15000 });
});

test('bech32-codec page honours a query-param deep link', async ({ page }) => {
  await page.goto('/tools/bech32-codec/?input=hello&mode=encode&hrp=test&variant=bech32m&format=text');
  await expect(page.locator('#in-input')).toHaveValue('hello');
  await expect(page.locator('#in-hrp')).toHaveValue('test');
  await expect(page.locator('#in-variant')).toHaveValue('bech32m');
  await expect(page.locator('#in-format')).toHaveValue('text');
  await expect(page.locator('#tool-output')).toHaveText('test1dpjkcmr0scqr9j', { timeout: 15000 });
});
