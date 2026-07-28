import { test, expect } from './fixtures';

// Regression guard for tool.js's showResult(): for format="number" tools the
// visible text goes through formatNumber(value) (float-noise trimming), but
// "Copy result" must copy exactly what's on screen — not the raw pre-format
// float. 0.1 + 0.2 displays "0.3"; the raw JS float is
// "0.30000000000000004". Before this branch's copy-handler change, Copy
// matched the display; a regression made it copy the raw float again.

test('calculator copies the displayed result, not the raw float', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], {
    origin: 'http://localhost:8001',
  });
  await page.goto('/tools/calculator/');
  await page.fill('#in-expr', '0.1+0.2');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('0.3', { timeout: 15000 });

  await page.click('#tool-copy-output');
  const clipboardText = await page.evaluate(() => navigator.clipboard.readText());

  expect(clipboardText).toBe('0.3');
  expect(clipboardText).toBe(await out.textContent());
});
