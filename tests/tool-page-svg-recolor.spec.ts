import { test, expect } from './fixtures';

// /tools/svg-recolor/ remaps SVG colors in-browser (pure wasm).
test('svg-recolor swaps mapped fill/stroke colors', async ({ page }) => {
  await page.goto('/tools/svg-recolor/');
  await page.fill(
    '#in-svg',
    '<svg xmlns="http://www.w3.org/2000/svg"><path fill="#000000" stroke="red" d="M0 0"/></svg>',
  );
  await page.fill('#in-mapping', '#000000 => #ffffff\nred => #00ff00');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('fill="#ffffff"', { timeout: 15000 });
  await expect(out).toContainText('stroke="#00ff00"');
  // path geometry untouched
  await expect(out).toContainText('d="M0 0"');
});

test('svg-recolor matches a source color in any syntax', async ({ page }) => {
  await page.goto('/tools/svg-recolor/');
  await page.fill(
    '#in-svg',
    '<svg xmlns="http://www.w3.org/2000/svg"><rect fill="#FFF"/><rect fill="rgb(255, 255, 255)"/></svg>',
  );
  await page.fill('#in-mapping', '#ffffff => #123456');
  const out = page.locator('#tool-output');
  // both occurrences should be rewritten to the canonical hex
  await expect(out).toContainText('#123456', { timeout: 15000 });
  await expect(async () => {
    const txt = await out.textContent();
    expect((txt ?? '').match(/#123456/g)?.length ?? 0).toBe(2);
  }).toPass({ timeout: 15000 });
});

test('svg-recolor monochrome recolors every color and keeps none', async ({ page }) => {
  await page.goto('/tools/svg-recolor/');
  await page.fill(
    '#in-svg',
    '<svg xmlns="http://www.w3.org/2000/svg"><path fill="#000"/><path stroke="blue"/><path fill="none"/></svg>',
  );
  await page.fill('#in-to', '#ff0000');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('fill="#ff0000"', { timeout: 15000 });
  await expect(out).toContainText('stroke="#ff0000"');
  await expect(out).toContainText('fill="none"');
});
