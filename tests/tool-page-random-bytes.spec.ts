import { test, expect } from './fixtures';

const tool = '/tools/random-bytes/';
const seed = '00112233445566778899aabbccddeeff';
const expectedHex =
  '7ec9d9574b137d2d\n' +
  'd8f2174cb89a86f8\n' +
  '1b9f730bb019bf3f\n\n' +
  '3 values · 8 bytes (64 bits) each · hex · derived from seed_hex (reproducible, NOT secret unless the seed is) · equivalent: openssl rand -hex 8';
const expectedUpperColon =
  '7E:C9:D9:57\n\n' +
  '1 value · 4 bytes (32 bits) each · hex · colon-separated · uppercase · derived from seed_hex (reproducible, NOT secret unless the seed is) · equivalent: openssl rand -hex 4';

async function runWasm(
  page,
  bytes = '8',
  count = '3',
  encoding = 'hex',
  separator = 'auto',
  uppercase = 'false',
  output = 'text',
  seedHex = seed,
): Promise<string> {
  return await page.evaluate(
    async ({ bytes, count, encoding, separator, uppercase, output, seedHex }) => {
      const mod = await import('/tools/random-bytes/gizza_ai_random_bytes_web.js');
      await mod.default('/tools/random-bytes/gizza_ai_random_bytes_web_bg.wasm');
      return mod.run(bytes, count, encoding, separator, uppercase, output, seedHex);
    },
    { bytes, count, encoding, separator, uppercase, output, seedHex },
  );
}

test('random-bytes page renders deterministic hex bytes exactly', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-bytes', '8');
  await page.fill('#in-count', '3');
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-separator', 'auto');
  await page.selectOption('#in-output', 'text');
  await page.fill('#in-seed_hex', seed);

  await expect(page.locator('#tool-output')).toHaveText(expectedHex, { timeout: 15_000 });
});

test('random-bytes deep link pre-fills and runs uppercase colon hex', async ({ page }) => {
  const qs = new URLSearchParams({
    bytes: '4',
    count: '1',
    encoding: 'hex',
    separator: 'colon',
    uppercase: 'true',
    output: 'text',
    seed_hex: seed,
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-bytes')).toHaveValue('4', { timeout: 15_000 });
  await expect(page.locator('#in-separator')).toHaveValue('colon');
  await expect(page.locator('#in-uppercase')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(expectedUpperColon, { timeout: 15_000 });
});

test('random-bytes wasm covers encodings, separators, checkbox and caps', async ({ page }) => {
  await page.goto(tool);

  const base64 = await runWasm(page, '8', '3', 'base64');
  expect(base64).toContain('fsnZV0sTfS0=');
  expect(base64).toContain('equivalent: openssl rand -base64 8');

  const base64Url = await runWasm(page, '2', '1', 'base64url');
  expect(base64Url.split('\n')[0]).toBe('fsk');
  expect(base64Url).not.toContain('=');

  const binary = await runWasm(page, '2', '1', 'binary', 'none');
  expect(binary.split('\n')[0]).toBe('0111111011001001');

  const decimal = await runWasm(page, '2', '2', 'decimal', 'comma', 'false', 'json');
  expect(JSON.parse(decimal)).toMatchObject({
    count: 2,
    bytes: 2,
    bits: 16,
    encoding: 'decimal',
    deterministic: true,
    values: ['126, 201', '217, 87'],
  });

  const cArray = await runWasm(page, '3', '1', 'c-array', 'auto', 'true');
  expect(cArray.split('\n')[0]).toBe('{ 0x7E, 0xC9, 0xD9 }');

  const pythonBytes = await runWasm(page, '3', '1', 'python-bytes');
  expect(pythonBytes.split('\n')[0]).toBe("b'\\x7e\\xc9\\xd9'");

  const maxBoundary = await runWasm(page, '81', '100', 'hex', 'auto', 'false', 'json');
  const parsed = JSON.parse(maxBoundary);
  expect(parsed.count).toBe(100);
  expect(parsed.bytes).toBe(81);
  expect(parsed.values).toHaveLength(100);

  await expect(runWasm(page, '82', '100')).rejects.toThrow(/at most 8192 random bytes per run/);
  await expect(runWasm(page, '4097', '1')).rejects.toThrow(/bytes must be between 1 and 4096/);
  await expect(runWasm(page, '1', '101')).rejects.toThrow(/count must be between 1 and 100/);
  await expect(runWasm(page, '4', '1', 'base58')).rejects.toThrow(/unknown encoding/);
  await expect(runWasm(page, '4', '1', 'hex', 'pipe')).rejects.toThrow(/unknown separator/);
  await expect(runWasm(page, '4', '1', 'hex', 'auto', 'false', 'text', 'abc')).rejects.toThrow(/even number/);
});

test('random-bytes ships security-size example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(8);
  await expect(page.locator('.tool-example-chip')).toContainText([
    '32 bytes hex (AES-256 key)',
    '32 bytes Base64 (JWT secret)',
    '24 bytes Base64URL token',
    '16 bytes hex (AES-128 IV)',
    '6 bytes colon-separated (MAC style)',
    '16 bytes as a C array',
    '10 x 32-byte keys as JSON',
    'Reproducible batch from a seed',
  ]);
});
