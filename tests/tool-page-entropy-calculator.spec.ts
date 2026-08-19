import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  text = 'password',
  basis = 'characters',
  unit = 'bits',
  scope = 'whole',
  ignoreCase = 'false',
  ignoreWhitespace = 'false',
  precision = '4',
  showFrequencies = 'true',
  topSymbols = '12',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/entropy-calculator/gizza_ai_entropy_calculator_web.js');
    await mod.default('/tools/entropy-calculator/gizza_ai_entropy_calculator_web_bg.wasm');
    return mod.run(args.text, args.basis, args.unit, args.scope, args.ignoreCase, args.ignoreWhitespace, args.precision, args.showFrequencies, args.topSymbols);
  }, { text, basis, unit, scope, ignoreCase, ignoreWhitespace, precision, showFrequencies, topSymbols });
}

test('entropy-calculator page computes a real entropy report from the form', async ({ page }) => {
  await page.goto('/tools/entropy-calculator/');
  await page.fill('#in-text', 'password');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Shannon entropy: 2.7500 bits per character', { timeout: 15_000 });
  await expect(out).toContainText('Total information: 22.0000 bits over 8 characters');
  await expect(out).toContainText('Distinct characters: 7');
  await expect(out).toContainText('Perplexity: 6.7272');
});

test('entropy-calculator deep link covers line scope and non-default checkboxes', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'Aa\naa',
    basis: 'characters',
    unit: 'bits',
    scope: 'line',
    ignore_case: 'true',
    ignore_whitespace: 'true',
    precision: '2',
    show_frequencies: 'false',
    top_symbols: '0',
  });
  await page.goto(`/tools/entropy-calculator/?${params.toString()}`);
  await expect(page.locator('#in-scope')).toHaveValue('line', { timeout: 15_000 });
  await expect(page.locator('#in-ignore_case')).toBeChecked();
  await expect(page.locator('#in-ignore_whitespace')).toBeChecked();
  await expect(page.locator('#in-show_frequencies')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('Line 1: 0.00 bits per character', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Combined:');
  await expect(page.locator('#tool-output')).not.toContainText('Symbol frequencies');
});

test('entropy-calculator wasm covers bases, units, scopes, filters and boundaries', async ({ page }) => {
  await page.goto('/tools/entropy-calculator/');
  await page.waitForSelector('#in-text');

  const password = await runWasm(page);
  expect(password).toContain('Shannon entropy: 2.7500 bits per character');
  expect(password).toContain("'s'  2  25.00%  ####################");

  expect(await runWasm(page, 'abcd')).toContain('Shannon entropy: 2.0000 bits per character');
  expect(await runWasm(page, 'abcd', 'characters', 'nats')).toContain('Shannon entropy: 1.3863 nats per character');
  expect(await runWasm(page, 'abcd', 'characters', 'dits')).toContain('Shannon entropy: 0.6021 dits per character');
  expect(await runWasm(page, 'abcd', 'characters', 'trits')).toContain('Shannon entropy: 1.2619 trits per character');

  const bytes = await runWasm(page, 'é', 'bytes', 'bits', 'whole', 'false', 'false', '4', 'true', '4');
  expect(bytes).toContain('2 bytes');
  expect(bytes).toContain('0xc3');
  expect(bytes).toContain('0xa9');

  const words = await runWasm(page, 'one one two', 'words');
  expect(words).toContain('Shannon entropy: 0.9183 bits per word');
  expect(words).toContain('Distinct words: 2');

  const folded = await runWasm(page, 'Aa', 'characters', 'bits', 'whole', 'true');
  expect(folded).toContain('Shannon entropy: 0.0000 bits per character');

  const noWhitespace = await runWasm(page, 'a a', 'characters', 'bits', 'whole', 'false', 'true');
  expect(noWhitespace).toContain('Total information: 0.0000 bits over 2 characters');

  await expect(runWasm(page, 'abcd', 'characters', 'bits', 'whole', 'false', 'false', '10')).resolves.toContain('2.0000000000');
  await expect(runWasm(page, 'abcd', 'characters', 'bits', 'whole', 'false', 'false', '11')).rejects.toThrow(/precision 11 is out of range/);
  await expect(runWasm(page, 'abcd', 'characters', 'bits', 'whole', 'false', 'false', '4', 'true', '64')).resolves.toContain('Symbol frequencies');
  await expect(runWasm(page, 'abcd', 'characters', 'bits', 'whole', 'false', 'false', '4', 'true', '65')).rejects.toThrow(/top_symbols 65 is out of range/);
  await expect(runWasm(page, '', 'characters')).rejects.toThrow(/text is empty/);
});

test('entropy-calculator generated CLI example is generic and runnable-looking', async ({ page }) => {
  await page.goto('/tools/entropy-calculator/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool entropy-calculator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
