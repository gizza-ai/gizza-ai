import { test, expect } from './fixtures';

const DEMO_ADDRESS = '0xAB43b6E5474F4Fab93aCe085aFa69f1742F03215';
const DEMO_LOWER = '0xab43b6e5474f4fab93ace085afa69f1742f03215';

test('eth-vanity-address page returns a deterministic prefix match', async ({ page }) => {
  await page.goto('/tools/eth-vanity-address/');
  await page.fill('#in-prefix', 'ab');
  await page.fill('#in-max_attempts', '10000');
  await page.fill('#in-seed', 'gizza-demo');
  await page.selectOption('#in-output_format', 'address');

  await expect(page.locator('#tool-output')).toHaveText(DEMO_ADDRESS, { timeout: 20_000 });
});

test('eth-vanity-address page estimates difficulty without generating a key', async ({ page }) => {
  await page.goto('/tools/eth-vanity-address/');
  await page.fill('#in-prefix', 'dead');
  await page.fill('#in-max_attempts', '100000');
  await page.selectOption('#in-output_format', 'estimate');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Pattern:         0xdead…, case-insensitive', { timeout: 20_000 });
  await expect(out).toContainText('Difficulty:      1 in 65,536');
  await expect(out).toContainText('Chance within 100,000: 78.26%');
  await expect(out).toContainText('(Estimate only — no keys were generated.)');
});

test('eth-vanity-address deep link prefills case-sensitive JSON params', async ({ page }) => {
  const params = new URLSearchParams({
    prefix: 'AB',
    suffix: '',
    match_case: 'true',
    max_attempts: '200000',
    seed: 'gizza-demo',
    output_format: 'json',
  });
  await page.goto(`/tools/eth-vanity-address/?${params.toString()}`);

  await expect(page.locator('#in-prefix')).toHaveValue('AB', { timeout: 20_000 });
  await expect(page.locator('#in-match_case')).toBeChecked();
  await expect(page.locator('#in-max_attempts')).toHaveValue('200000');
  await expect(page.locator('#in-seed')).toHaveValue('gizza-demo');
  await expect(page.locator('#in-output_format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText(`"address": "${DEMO_ADDRESS}"`, { timeout: 20_000 });
  await expect(out).toContainText(`"address_lowercase": "${DEMO_LOWER}"`);
  await expect(out).toContainText('"match_case": true');
  await expect(out).toContainText('"attempts": 72');
});
