import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  ratio = '16:9',
  width = 1920,
  height = 0,
  rounding = 'nearest',
  outputFormat = 'summary',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/aspect-ratio-calc/gizza_ai_aspect_ratio_calc_web.js');
    await mod.default('/tools/aspect-ratio-calc/gizza_ai_aspect_ratio_calc_web_bg.wasm');
    return mod.run(args.ratio, args.width, args.height, args.rounding, args.outputFormat);
  }, { ratio, width, height, rounding, outputFormat });
}

test('aspect-ratio-calc page solves a missing height from the defaults', async ({ page }) => {
  await page.goto('/tools/aspect-ratio-calc/');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Height: 1080 px', { timeout: 15_000 });
  await expect(out).toContainText('Dimensions: 1920 x 1080 px');
  await expect(out).toContainText('Aspect ratio: 16:9 (1.7778:1)');
  await expect(out).toContainText('Nearest standard: 16:9 — Widescreen HD video (exact match)');
  await expect(out).toContainText('CSS: aspect-ratio: 16 / 9; (legacy padding-top: 56.25%)');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool aspect-ratio-calc');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('aspect-ratio-calc deep-link simplifies a resolution to a ratio', async ({ page }) => {
  const params = new URLSearchParams({
    ratio: '',
    width: '2560',
    height: '1600',
    output_format: 'ratio',
  });
  await page.goto(`/tools/aspect-ratio-calc/?${params.toString()}`);

  await expect(page.locator('#in-ratio')).toHaveValue('', { timeout: 15_000 });
  await expect(page.locator('#in-width')).toHaveValue('2560');
  await expect(page.locator('#in-height')).toHaveValue('1600');
  await expect(page.locator('#in-output_format')).toHaveValue('ratio');
  await expect(page.locator('#tool-output')).toHaveText('8:5', { timeout: 15_000 });
});

test('aspect-ratio-calc wasm covers formats, rounding, bounds and errors', async ({ page }) => {
  await page.goto('/tools/aspect-ratio-calc/');
  await page.waitForSelector('#in-ratio');

  await expect(runWasm(page, '16:9', 1920, 0, 'nearest', 'dimensions')).resolves.toBe('1920x1080');
  await expect(runWasm(page, '9:16', 1080, 0, 'nearest', 'dimensions')).resolves.toBe('1080x1920');
  await expect(runWasm(page, '', 1366, 768, 'nearest', 'ratio')).resolves.toBe('683:384');
  await expect(runWasm(page, '1.85:1', 1920, 0, 'down', 'dimensions')).resolves.toBe('1920x1037');
  await expect(runWasm(page, '4:5', 0, 0, 'nearest', 'css')).resolves.toContain('aspect-ratio: 4 / 5;');
  await expect(runWasm(page, '16:9', 1920, 0, 'nearest', 'json')).resolves.toContain('"mode": "solve_height"');

  await expect(runWasm(page, '', 1920, 0)).rejects.toThrow(/give a ratio/);
  await expect(runWasm(page, 'banana', 1920, 0)).rejects.toThrow(/use a form like 16:9/);
  await expect(runWasm(page, '16:9', 1_000_001, 0)).rejects.toThrow(/at most 1000000/);
});
