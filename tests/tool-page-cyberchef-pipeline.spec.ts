import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('cyberchef-pipeline decodes a base64 payload', async ({ page }) => {
  await page.goto('/tools/cyberchef-pipeline/');
  await page.fill('#in-input', 'SGVsbG8sIHdvcmxkIQ==');
  await page.fill('#in-recipe', 'from-base64');
  await page.selectOption('#in-output_format', 'auto');
  await expect(page.locator('#tool-output')).toHaveText('Hello, world!', { timeout: 15000 });
});

test('cyberchef-pipeline chains url-decode, base64 and rot13', async ({ page }) => {
  await page.goto('/tools/cyberchef-pipeline/');
  await page.fill('#in-input', 'VXJ5eWI%3D');
  await page.fill('#in-recipe', 'url-decode\nfrom-base64\nrot13');
  await expect(page.locator('#tool-output')).toHaveText('Hello', { timeout: 15000 });
});

test('cyberchef-pipeline deep-link pre-fills and auto-runs', async ({ page }) => {
  await page.goto('/tools/cyberchef-pipeline/?input=48656c6c6f&recipe=from-hex&output_format=auto');
  await expect(page.locator('#in-input')).toHaveValue('48656c6c6f', { timeout: 15000 });
  await expect(page.locator('#in-recipe')).toHaveValue('from-hex');
  await expect(page.locator('#in-output_format')).toHaveValue('auto');
  await expect(page.locator('#tool-output')).toHaveText('Hello', { timeout: 15000 });
});

test('cyberchef-pipeline output format matrix covers hex and base64', async ({ page }) => {
  await page.goto('/tools/cyberchef-pipeline/');
  await page.fill('#in-input', 'Hi');
  await page.fill('#in-recipe', 'reverse');
  await page.selectOption('#in-output_format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('6948', { timeout: 15000 });

  await page.selectOption('#in-output_format', 'base64');
  await expect(page.locator('#tool-output')).toHaveText('aUg=', { timeout: 15000 });
});

test('cyberchef-pipeline reports recipe errors with the line number', async ({ page }) => {
  await page.goto('/tools/cyberchef-pipeline/');
  await page.fill('#in-input', 'abc');
  await page.fill('#in-recipe', 'from-hex');
  await expect(page.locator('#tool-output')).toContainText('recipe line 1', { timeout: 15000 });
  expect(await outputText(page)).toContain('odd number of hex digits');
});
