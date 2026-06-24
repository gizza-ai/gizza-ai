import { test, expect } from './fixtures';

test('hash-all page computes every digest', async ({ page }) => {
  await page.goto('/tools/hash-all/');
  await page.fill('#in-text', 'abc');
  const out = page.locator('#tool-output');
  // Wait for the SHA-256("abc") vector, then assert several others are present.
  await expect(out).toContainText(
    'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    { timeout: 15000 },
  );
  const text = await out.innerText();
  expect(text).toContain('MD5');
  expect(text).toContain('900150983cd24fb0d6963f7d28e17f72');
  expect(text).toContain('RIPEMD-160');
  expect(text).toContain('8eb208f7e05d987a9b044a8e98c6b087f15a0bfc');
  expect(text).toContain('Whirlpool');
  expect(text).toContain('BLAKE3');
  expect(text).toContain('6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85');
});

test('hash-all page honours uppercase hex', async ({ page }) => {
  await page.goto('/tools/hash-all/');
  await page.fill('#in-text', 'abc');
  await page.check('#in-uppercase');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('900150983CD24FB0D6963F7D28E17F72', { timeout: 15000 });
});
