import { test, expect } from './fixtures';

const tool = '/tools/recovery-code-generator/';
const seed = '00112233445566778899aabbccddeeff';
const expectedNumbered =
  '1. 9850-5721\n' +
  ' 2. 1909-0601\n' +
  ' 3. 7191-8782\n\n' +
  '3 codes · 8 characters each (0-9, 10-character alphabet) · 26.6 bits per code · 79.7 bits total · derived from seed_hex (reproducible, NOT secret unless the seed is)';

async function runWasm(
  page,
  count = '3',
  blocks = '2',
  charsPerBlock = '4',
  charset = 'numeric',
  separator = '-',
  output = 'numbered',
  hash = 'none',
  seedHex = seed,
): Promise<string> {
  return await page.evaluate(
    async ({ count, blocks, charsPerBlock, charset, separator, output, hash, seedHex }) => {
      const mod = await import('/tools/recovery-code-generator/gizza_ai_recovery_code_generator_web.js');
      await mod.default('/tools/recovery-code-generator/gizza_ai_recovery_code_generator_web_bg.wasm');
      return mod.run(count, blocks, charsPerBlock, charset, separator, output, hash, seedHex);
    },
    { count, blocks, charsPerBlock, charset, separator, output, hash, seedHex },
  );
}

test('recovery-code-generator page renders a deterministic numbered sheet', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-count', '3');
  await page.fill('#in-blocks', '2');
  await page.fill('#in-chars_per_block', '4');
  await page.selectOption('#in-charset', 'numeric');
  await page.fill('#in-separator', '-');
  await page.selectOption('#in-output', 'numbered');
  await page.selectOption('#in-hash', 'none');
  await page.fill('#in-seed_hex', seed);
  await expect(page.locator('#tool-output')).toHaveText(expectedNumbered, { timeout: 15_000 });
});

test('recovery-code-generator deep link pre-fills and runs', async ({ page }) => {
  const qs = new URLSearchParams({
    count: '3',
    blocks: '2',
    chars_per_block: '4',
    charset: 'numeric',
    separator: '-',
    output: 'numbered',
    hash: 'none',
    seed_hex: seed,
  });
  await page.goto(`${tool}?${qs.toString()}`);
  await expect(page.locator('#in-seed_hex')).toHaveValue(seed, { timeout: 15_000 });
  await expect(page.locator('#in-charset')).toHaveValue('numeric');
  await expect(page.locator('#tool-output')).toHaveText(expectedNumbered, { timeout: 15_000 });
});

test('recovery-code-generator wasm covers advertised choices and boundaries', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-count');

  const lowercase = await runWasm(page, '1', '1', '8', 'lowercase', '-', 'plain');
  expect(lowercase).toContain('1 code · 8 characters each (a-z0-9, 36-character alphabet)');

  const alpha = await runWasm(page, '1', '1', '8', 'alphanumeric', '-', 'plain');
  expect(alpha).toContain('A-Za-z0-9, 62-character alphabet');

  const upper = await runWasm(page, '1', '1', '8', 'uppercase', '-', 'plain');
  expect(upper).toContain('A-Z0-9, 36-character alphabet');

  const unambiguous = await runWasm(page, '1', '1', '8', 'unambiguous', '-', 'plain');
  expect(unambiguous).toContain('without look-alikes, 30-character alphabet');

  const hex = await runWasm(page, '1', '1', '8', 'hex', '-', 'plain');
  expect(hex).toContain('0-9a-f, 16-character alphabet');

  const csv = await runWasm(page, '2', '2', '4', 'numeric', '-', 'csv', 'sha256');
  expect(csv).toMatch(/^index,code,sha256\n1,\d{4}-\d{4},[0-9a-f]{64}\n2,\d{4}-\d{4},[0-9a-f]{64}$/);

  const json = JSON.parse(await runWasm(page, '1', '1', '2', 'hex', '', 'json'));
  expect(json).toMatchObject({ count: 1, code_length: 2, alphabet_size: 16, deterministic: true });
  expect(json.codes[0]).toMatch(/^[0-9a-f]{2}$/);

  const salted = await runWasm(page, '1', '1', '8', 'lowercase', '-', 'plain', 'sha256-salted');
  expect(salted).toMatch(/[0-9a-f]{32}:[0-9a-f]{64}/);

  const maxBoundary = await runWasm(page, '50', '6', '16', 'hex', '', 'json');
  const parsed = JSON.parse(maxBoundary);
  expect(parsed.count).toBe(50);
  expect(parsed.code_length).toBe(96);

  await expect(runWasm(page, '51')).rejects.toThrow(/count must be between 1 and 50/);
  await expect(runWasm(page, '1', '2', '4', 'numeric', '0')).rejects.toThrow(/separator character '0' is also in the numeric alphabet/);
  await expect(runWasm(page, '1', '1', '8', 'numeric', '-', 'plain', 'none', 'abc')).rejects.toThrow(/even number of hex digits/);
});

test('recovery-code-generator ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(6);
  await expect(page.locator('.tool-example-chip')).toContainText([
    '10 codes, 5+5 lowercase',
    '8 digits, no separator',
    '4+4 numeric, dashed',
    'Print-safe, no look-alikes',
    'CSV + hashes to store server-side',
    'Reproducible sheet from a seed',
  ]);
});
