import { test, expect } from './fixtures';

const OUTLINE = `Launch plan
  Product
    Pricing
    Onboarding
  Marketing
    Landing page
    Email list`;

test('outline-to-mindmap renders a rightward SVG mind map', async ({ page }) => {
  await page.goto('/tools/outline-to-mindmap/');
  await page.fill('#in-outline', OUTLINE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15000 });
  await expect(out).toContainText('Launch plan');
  await expect(out).toContainText('Pricing');
  await expect(out).toContainText('<path');
});

test('outline-to-mindmap supports down layout and dark mode via query params', async ({ page }) => {
  await page.goto('/tools/outline-to-mindmap/?direction=down&dark_mode=true');
  await page.fill('#in-outline', OUTLINE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15000 });
  await expect(out).toContainText('Launch plan');
  await expect(out).toContainText('#0f172a');
});

test('outline-to-mindmap groups multiple top-level roots under the title', async ({ page }) => {
  await page.goto('/tools/outline-to-mindmap/?title=Roadmap');
  await page.fill('#in-outline', 'Alpha\n  A1\nBeta\n  B1');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Roadmap', { timeout: 15000 });
  await expect(out).toContainText('Alpha');
  await expect(out).toContainText('Beta');
});

test('outline-to-mindmap query-param deep-link prefills controls', async ({ page }) => {
  await page.goto('/tools/outline-to-mindmap/?direction=down&title=Plan&colorful=false&dark_mode=true');
  await expect(page.locator('#in-direction')).toHaveValue('down', { timeout: 15000 });
  await expect(page.locator('#in-title')).toHaveValue('Plan');
  await expect(page.locator('#in-colorful')).not.toBeChecked();
  await expect(page.locator('#in-dark_mode')).toBeChecked();
});
