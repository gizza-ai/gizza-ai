// Declarative widget chrome (tools/generator + its runtime tool.js): example chips,
// meta-declared defaults, Reset, and Copy-result — rendered on every page from
// meta.toml with zero per-tool JS. age-calculator is the first migrated tool
// (kind="date" pickers replace its old slug-specific setup in tool.js).
import { test, expect } from './fixtures';

const localToday = () => {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
};

test('example chip prefills its params and runs', async ({ page }) => {
  await page.goto('/tools/age-calculator/');
  await page.click('.tool-example-chip:has-text("Born 15 Apr 1990")');
  await expect(page.locator('#in-birthdate')).toHaveValue('1990-04-15');
  await expect(page.locator('#tool-output .age-result-hero-text')).toContainText('old', { timeout: 15000 });
});

test('kind="date" renders native pickers and default="today" prefills', async ({ page }) => {
  await page.goto('/tools/age-calculator/');
  await expect(page.locator('#in-birthdate')).toHaveAttribute('type', 'date');
  await expect(page.locator('#in-as_of')).toHaveValue(localToday());
});

test('reset restores server defaults and meta defaults', async ({ page }) => {
  await page.goto('/tools/age-calculator/');
  await page.fill('#in-birthdate', '1990-01-15');
  await page.fill('#in-as_of', '2026-06-22');
  await page.click('#tool-reset');
  await expect(page.locator('#in-birthdate')).toHaveValue('');
  await expect(page.locator('#in-as_of')).toHaveValue(localToday());
});

test('copy-result copies the output text', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/tools/url-encode/');
  await page.fill('#in-text', 'a b');
  await expect(page.locator('#tool-output')).toHaveText('a%20b', { timeout: 15000 });
  await page.click('#tool-copy-output');
  await expect(page.locator('#tool-copy-output')).toHaveClass(/copied/);
  const clip = await page.evaluate(() => navigator.clipboard.readText());
  expect(clip).toBe('a%20b');
});
