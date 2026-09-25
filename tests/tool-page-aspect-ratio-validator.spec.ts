import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  width = '1920',
  height = '1080',
  target = '16:9',
  tolerancePercent = '1',
  orientationAgnostic = 'false',
  evenDimensions = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/aspect-ratio-validator/gizza_ai_aspect_ratio_validator_web.js');
    await mod.default('/tools/aspect-ratio-validator/gizza_ai_aspect_ratio_validator_web_bg.wasm');
    return mod.run(
      args.width,
      args.height,
      args.target,
      args.tolerancePercent,
      args.orientationAgnostic,
      args.evenDimensions,
    );
  }, { width, height, target, tolerancePercent, orientationAgnostic, evenDimensions });
}

test('aspect-ratio-validator page renders default 16:9 PASS report', async ({ page }) => {
  await page.goto('/tools/aspect-ratio-validator/');
  await expect(page.locator('#tool-output')).toContainText('"status": "PASS"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"ratio": "16:9"');
  await expect(page.locator('#tool-output')).toContainText('PASS — 1920x1080 is 16:9');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool aspect-ratio-validator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('aspect-ratio-validator deep-link prefills controls and reports crop/pad fix', async ({ page }) => {
  const params = new URLSearchParams({
    width: '1600',
    height: '1200',
    target: '16:9',
    tolerance_percent: '1',
    even_dimensions: 'true',
  });
  await page.goto(`/tools/aspect-ratio-validator/?${params.toString()}`);
  await expect(page.locator('#in-width')).toHaveValue('1600', { timeout: 15_000 });
  await expect(page.locator('#in-height')).toHaveValue('1200');
  await expect(page.locator('#in-target')).toHaveValue('16:9');
  await expect(page.locator('#in-even_dimensions')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"status": "FAIL"', { timeout: 15_000 });
  await expect(out).toContainText('"reason": "too_tall"');
  await expect(out).toContainText('Crop to 1600x900 or pad to 2134x1200');
});

test('aspect-ratio-validator wasm covers target forms, booleans, bounds and errors', async ({ page }) => {
  await page.goto('/tools/aspect-ratio-validator/');
  await page.waitForSelector('#in-width');

  expect(await runWasm(page, '1080', '1920', '16:9', '0', 'true', 'true')).toContain('"orientation_flipped": true');
  expect(await runWasm(page, '1080', '1350', '4/5', '0', 'false', 'true')).toContain('"ratio": "4:5"');
  expect(await runWasm(page, '3840', '1607', '2.39:1', '0.1', 'false', 'true')).toContain('"target_ratio": "2.39:1"');
  expect(await runWasm(page, '1600', '1200', '1.7778', '1', 'false', 'true')).toContain('"crop_height": 900');

  await expect(runWasm(page, '', '1080')).rejects.toThrow(/enter the width/);
  await expect(runWasm(page, '1920', '1080', 'square-ish')).rejects.toThrow(/use a form like 16:9/);
});
