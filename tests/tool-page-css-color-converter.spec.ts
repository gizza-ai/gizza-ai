import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  input = '#3498db',
  syntax = 'legacy',
  precision = '3',
  uppercaseHex = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/css-color-converter/gizza_ai_css_color_converter_web.js');
    await mod.default('/tools/css-color-converter/gizza_ai_css_color_converter_web_bg.wasm');
    return mod.run(args.input, args.syntax, args.precision, args.uppercaseHex);
  }, { input, syntax, precision, uppercaseHex });
}

test('css-color-converter page converts a pasted hex color', async ({ page }) => {
  await page.goto('/tools/css-color-converter/');
  await page.fill('#in-input', '#3498db');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('HEX                 #3498db', { timeout: 20_000 });
  await expect(output).toContainText('RGB                 rgb(52, 152, 219)');
  await expect(output).toContainText('OKLCH               oklch(65.309% 0.135 242.687)');
  await expect(output).toContainText('Flutter / Dart      Color(0xff3498db)');
  await expect(output).toContainText('On white            3.15:1 (AA large text only)');
});

test('css-color-converter deep link covers modern syntax and uppercase hex', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'rgba(52, 152, 219, 0.5)',
    syntax: 'modern',
    precision: '3',
    uppercase_hex: 'true',
  });
  await page.goto(`/tools/css-color-converter/?${params.toString()}`);

  await expect(page.locator('#in-syntax')).toHaveValue('modern', { timeout: 15_000 });
  await expect(page.locator('#in-uppercase_hex')).toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('HEX + alpha         #3498DB80', { timeout: 20_000 });
  await expect(output).toContainText('RGB                 rgb(52 152 219 / 0.5)');
  await expect(output).toContainText('Flutter / Dart      Color(0x803498DB)');
});

test('css-color-converter wasm covers advertised input forms and errors', async ({ page }) => {
  await page.goto('/tools/css-color-converter/');

  await expect(runWasm(page, '#f00', 'legacy', '0')).resolves.toContain('HSL                 hsl(0, 100%, 50%)');
  await expect(runWasm(page, '#3498db', 'legacy', '3', 'true')).resolves.toContain('HEX                 #3498DB');
  await expect(runWasm(page, '0xFF6750A4')).resolves.toContain('Flutter / Dart      Color(0xff6750a4)');
  await expect(runWasm(page, 'oklch(65.309% 0.135 242.687)', 'modern', '8')).resolves.toContain('HEX                 #3498db');
  await expect(runWasm(page, 'rebeccapurple')).resolves.toContain('CSS name            rebeccapurple');
  await expect(runWasm(page, 'not a color')).rejects.toThrow(/could not parse/);
});

test('css-color-converter ships a clean runnable CLI example', async ({ page }) => {
  await page.goto('/tools/css-color-converter/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe('gizza tool css-color-converter "#3498db"');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
