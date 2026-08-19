import { test, expect } from './fixtures';

async function outputText(page: any): Promise<string> {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('-----BEGIN AGE ENCRYPTED FILE-----', { timeout: 20000 });
  return ((await out.textContent()) ?? '').trim();
}

test('age-encrypt page creates armored passphrase ciphertext', async ({ page }) => {
  await page.goto('/tools/age-encrypt/');
  await page.fill('#in-text', 'Deploy key rotates at 17:00 UTC.');
  await page.fill('#in-passphrase', 'correct horse battery staple');
  await page.fill('#in-work_factor', '10');
  await page.selectOption('#in-mode', 'passphrase');

  const ciphertext = await outputText(page);
  expect(ciphertext).toContain('-----END AGE ENCRYPTED FILE-----');
  expect(ciphertext).not.toContain('Deploy key rotates');
  expect(ciphertext.length).toBeGreaterThan(180);
});

test('age-encrypt page honors passphrase deep link and low work-factor boundary', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'deep link secret',
    mode: 'passphrase',
    passphrase: 'demo passphrase',
    work_factor: '10',
  });
  await page.goto(`/tools/age-encrypt/?${params.toString()}`);

  const ciphertext = await outputText(page);
  expect(ciphertext).not.toContain('deep link secret');
  expect(ciphertext).toContain('YWdlLWVuY3J5cHRpb24ub3JnL3Yx');
  await expect(page.locator('#in-work_factor')).toHaveValue('10');
});

test('age-encrypt page reports missing recipient keys clearly', async ({ page }) => {
  await page.goto('/tools/age-encrypt/');
  await page.fill('#in-text', 'hello');
  await page.selectOption('#in-mode', 'recipients');

  await expect(page.locator('#tool-output')).toContainText('no recipients', { timeout: 20000 });
});
