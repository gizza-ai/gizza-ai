import { test, expect } from './fixtures';

async function fillOtp(page, opts: {
  mode?: string;
  cipher?: string;
  message?: string;
  pad?: string;
  encoding?: string;
  length?: string;
  group?: string;
}) {
  if (opts.mode) await page.selectOption('#in-mode', opts.mode);
  if (opts.cipher) await page.selectOption('#in-cipher', opts.cipher);
  if (opts.message !== undefined) await page.fill('#in-message', opts.message);
  if (opts.pad !== undefined) await page.fill('#in-pad', opts.pad);
  if (opts.encoding) await page.selectOption('#in-encoding', opts.encoding);
  if (opts.length !== undefined) await page.fill('#in-length', opts.length);
  if (opts.group !== undefined) await page.fill('#in-group', opts.group);
}

test('one-time-pad encrypts the canonical HELLO/XMCKA vector exactly', async ({ page }) => {
  await page.goto('/tools/one-time-pad/');
  await fillOtp(page, { mode: 'encrypt', cipher: 'letters', message: 'HELLO', pad: 'XMCKA' });

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"ciphertext": "EQNVO"', { timeout: 20000 });
  await expect(output).toContainText('"pad": "XMCKA"');
  await expect(output).toContainText('"pad_generated": false');
});

test('one-time-pad decrypts the canonical vector from a deep link', async ({ page }) => {
  const qs =
    '?mode=decrypt' +
    '&cipher=letters' +
    '&message=EQNVO' +
    '&pad=XMCKA' +
    '&encoding=hex' +
    '&length=0' +
    '&group=0';

  await page.goto('/tools/one-time-pad/' + qs);
  await expect(page.locator('#in-mode')).toHaveValue('decrypt', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"plaintext": "HELLO"', { timeout: 20000 });
});

test('one-time-pad supports digit OTP and grouping', async ({ page }) => {
  await page.goto('/tools/one-time-pad/');
  await fillOtp(page, {
    mode: 'encrypt',
    cipher: 'digits',
    message: '1234',
    pad: '9876',
    group: '2',
  });

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"ciphertext": "00 00"', { timeout: 20000 });
  await expect(output).toContainText('"pad": "98 76"');
  await expect(output).toContainText('"pad_length": 4');
});

test('one-time-pad supports XOR with base64 encoding', async ({ page }) => {
  await page.goto('/tools/one-time-pad/');
  await fillOtp(page, {
    mode: 'encrypt',
    cipher: 'xor',
    message: 'Hi',
    pad: 'AAA=',
    encoding: 'base64',
  });

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"encoding": "base64"', { timeout: 20000 });
  await expect(output).toContainText('"ciphertext": "SGk="');
});

test('one-time-pad auto-generates a pad while encrypting', async ({ page }) => {
  await page.goto('/tools/one-time-pad/');
  await fillOtp(page, { mode: 'encrypt', cipher: 'letters', message: 'MEET', pad: '' });

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"pad_generated": true', { timeout: 20000 });
  await expect(output).toContainText('"pad_length": 4');
  await expect(output).toContainText('"pad": "');
});

test('one-time-pad reports a short pad instead of repeating it', async ({ page }) => {
  await page.goto('/tools/one-time-pad/');
  await fillOtp(page, { mode: 'encrypt', cipher: 'letters', message: 'ATTACK AT DAWN', pad: 'XMCKA' });

  const output = page.locator('#tool-output');
  await expect(output).toHaveClass(/error/, { timeout: 20000 });
  await expect(output).toContainText('pad too short: message needs 12 pad letters, pad has 5');
});
