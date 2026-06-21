import { test, expect } from './fixtures';

// /tools/argon2-hash/ hashes/verifies passwords in-browser (pure wasm). password
// and number fields are inputs; mode is a <select>.
test('argon2-hash page hashes then verifies a password', async ({ page }) => {
  await page.goto('/tools/argon2-hash/');
  await page.fill('#in-password', 'hunter2');
  await page.selectOption('#in-mode', 'hash');
  await page.fill('#in-memory_kib', '4096'); // small for a fast test
  await page.fill('#in-iterations', '2');
  await page.fill('#in-parallelism', '1');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('$argon2id$', { timeout: 20000 });
  const phc = (await out.textContent())!.trim();

  // Verify the produced hash.
  await page.fill('#in-password', 'hunter2');
  await page.selectOption('#in-mode', 'verify');
  await page.fill('#in-hash', phc);
  await expect(out).toContainText('match', { timeout: 20000 });
  await expect(out).not.toContainText('no match');
});
