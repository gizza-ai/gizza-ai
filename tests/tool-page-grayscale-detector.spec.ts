import { test, expect } from './fixtures';

const GRAY = 'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAGUlEQVR4nGNgYGD439DQ8J/hPxBwcnIyAABEMQeWR4OwxQAAAABJRU5ErkJggg==';
const COLOR = 'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAGklEQVR4nGPg4uL6L6ch95+BnYPzPwMDw38AJSoEl4YJu4AAAAAASUVORK5CYII=';
const TINT = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGPgYWD4DwABNAEMg0Z3sgAAAABJRU5ErkJggg==';

function hexOfBase64(b64: string) {
  return Buffer.from(b64, 'base64').toString('hex');
}

async function runWasm(
  page: any,
  input = COLOR,
  input_format = 'base64',
  tolerance = '2',
  metric = 'channel_delta',
  ignore_alpha = true,
  max_samples = '20',
  output = 'report',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/grayscale-detector/gizza_ai_grayscale_detector_web.js');
    await mod.default('/tools/grayscale-detector/gizza_ai_grayscale_detector_web_bg.wasm');
    return mod.run(
      args.input,
      args.input_format,
      args.tolerance,
      args.metric,
      args.ignore_alpha ? 'true' : 'false',
      args.max_samples,
      args.output,
    );
  }, { input, input_format, tolerance, metric, ignore_alpha, max_samples, output });
}

test('grayscale-detector wasm reports exact grayscale verdicts', async ({ page }) => {
  await page.goto('/tools/grayscale-detector/');
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, GRAY, 'base64', '0')).toContain('Status: effectively grayscale');
  const color = await runWasm(page, COLOR, 'base64', '2');
  expect(color).toContain('Status: contains color pixels');
  expect(color).toContain('Dimensions: 2×2 (4 pixels)');
  expect(color).toContain('Color pixels: 1 (25.0000%)');
  expect(color).toContain('Sample color pixels:');
});

test('grayscale-detector page computes real output from form controls', async ({ page }) => {
  await page.goto('/tools/grayscale-detector/');
  await page.fill('#in-input', COLOR);
  await page.selectOption('#in-input_format', 'base64');
  await page.fill('#in-tolerance', '2');
  await page.selectOption('#in-metric', 'channel_delta');
  await page.check('#in-ignore_alpha');
  await page.fill('#in-max_samples', '20');
  await page.selectOption('#in-output', 'report');

  await expect(page.locator('#tool-output')).toContainText('Status: contains color pixels', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Color pixels: 1 (25.0000%)');
});

test('grayscale-detector deep link wires params and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    input: TINT,
    input_format: 'base64',
    tolerance: '20',
    metric: 'saturation',
    ignore_alpha: 'false',
    max_samples: '1',
    output: 'json',
  });
  await page.goto(`/tools/grayscale-detector/?${params.toString()}`);

  await expect(page.locator('#in-metric')).toHaveValue('saturation', { timeout: 15_000 });
  await expect(page.locator('#in-ignore_alpha')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"metric": "saturation"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"effective_grayscale": false');
});

test('grayscale-detector advertised values and CLI example stay wired', async ({ page }) => {
  await page.goto('/tools/grayscale-detector/');
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, hexOfBase64(GRAY), 'hex', '0', 'channel_delta', true, '20', 'report'))
    .toContain('Status: effectively grayscale');
  expect(await runWasm(page, COLOR, 'base64', '255', 'channel_delta', true, '20', 'report'))
    .toContain('Status: effectively grayscale');
  expect(await runWasm(page, TINT, 'base64', '20', 'saturation', true, '1', 'json'))
    .toContain('"metric": "saturation"');
  await expect(runWasm(page, COLOR, 'base64', '0', 'channel_delta', true, '201', 'report'))
    .rejects.toThrow(/max_samples must be 0-200/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool grayscale-detector');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
