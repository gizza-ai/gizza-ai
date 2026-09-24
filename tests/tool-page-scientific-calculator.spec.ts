import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  expression = 'sin(90) + cos(0)',
  variables = '',
  angleUnit = 'degrees',
  precision = '12',
  notation = 'auto',
  complexForm = 'rectangular',
  groupDigits = 'false',
  outputFormat = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/scientific-calculator/gizza_ai_scientific_calculator_web.js');
    await mod.default('/tools/scientific-calculator/gizza_ai_scientific_calculator_web_bg.wasm');
    return mod.run(
      args.expression,
      args.variables,
      args.angleUnit,
      args.precision,
      args.notation,
      args.complexForm,
      args.groupDigits,
      args.outputFormat,
    );
  }, { expression, variables, angleUnit, precision, notation, complexForm, groupDigits, outputFormat });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('scientific-calculator wasm evaluates exact scientific and complex outputs', async ({ page }) => {
  await page.goto('/tools/scientific-calculator/');
  await page.waitForSelector('#in-expression');

  await expect(runWasm(page)).resolves.toBe('2');
  await expect(runWasm(page, 'sqrt(-1)', '', 'radians')).resolves.toBe('i');
  await expect(runWasm(page, 'x = 7\ny = x^2\ny + ans', '', 'radians')).resolves.toBe('x = 7 -> 7\ny = x^2 -> 49\n98');
});

test('scientific-calculator page renders output and honors non-default checkbox', async ({ page }) => {
  await page.goto('/tools/scientific-calculator/');
  await page.fill('#in-expression', '10000 + 5');
  await page.fill('#in-variables', '');
  await page.selectOption('#in-angle_unit', 'radians');
  await page.fill('#in-precision', '12');
  await page.selectOption('#in-notation', 'auto');
  await page.selectOption('#in-complex_form', 'rectangular');
  await page.check('#in-group_digits');
  await page.selectOption('#in-output_format', 'text');

  await expect(page.locator('#tool-output')).toHaveText('10,005', { timeout: 15_000 });
});

test('scientific-calculator deep-link pre-fills and returns JSON', async ({ page }) => {
  const params = new URLSearchParams({
    expression: 'sqrt(-1)',
    variables: '',
    angle_unit: 'radians',
    precision: '12',
    notation: 'auto',
    complex_form: 'rectangular',
    group_digits: 'false',
    output_format: 'json',
  });

  await page.goto(`/tools/scientific-calculator/?${params.toString()}`);
  await expect(page.locator('#in-expression')).toHaveValue('sqrt(-1)', { timeout: 15_000 });
  await expect(page.locator('#in-output_format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"result": "i"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"imaginary": 1');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool scientific-calculator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
  expect(await outputText(page)).toContain('"input": "sqrt(-1)"');
});
