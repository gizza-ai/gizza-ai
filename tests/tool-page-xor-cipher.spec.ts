import { test, expect } from './fixtures';

test('xor-cipher page matches Hello/K vector', async ({ page }) => {
  await page.goto('/tools/xor-cipher/');
  await page.fill('#in-data', 'Hello');
  await page.selectOption('#in-input', 'text');
  await page.fill('#in-key', 'K');
  await page.selectOption('#in-key_format', 'text');
  await page.selectOption('#in-output', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('032e272724', { timeout: 15000 });
});

test('xor-cipher page round-trips hex ciphertext to utf8', async ({ page }) => {
  await page.goto('/tools/xor-cipher/');
  await page.fill('#in-data', 'attack');
  await page.selectOption('#in-input', 'text');
  await page.fill('#in-key', 'lemon');
  await page.selectOption('#in-key_format', 'text');
  await page.selectOption('#in-output', 'hex');

  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const ciphertext = (await out.textContent())!.trim();
  expect(ciphertext).toMatch(/^[0-9a-f]+$/);
  expect(ciphertext).not.toBe('attack');

  await page.fill('#in-data', ciphertext);
  await page.selectOption('#in-input', 'hex');
  await page.selectOption('#in-output', 'utf8');
  await expect(out).toHaveText('attack', { timeout: 15000 });
});

test('xor-cipher page rejects an empty key', async ({ page }) => {
  await page.goto('/tools/xor-cipher/');
  await page.fill('#in-data', 'hello');
  await page.fill('#in-key', '');
  await expect(page.locator('#tool-output')).toContainText('key must not be empty', { timeout: 15000 });
});

test('xor-cipher page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/xor-cipher/?data=Hello&input=text&key=K&key_format=text&output=hex');
  await expect(page.locator('#in-data')).toHaveValue('Hello');
  await expect(page.locator('#in-key')).toHaveValue('K');
  await expect(page.locator('#tool-output')).toHaveText('032e272724', { timeout: 15000 });
});
