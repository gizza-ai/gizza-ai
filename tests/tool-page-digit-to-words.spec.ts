import { test, expect } from './fixtures';

test('digit-to-words page spells default cardinals exactly', async ({ page }) => {
  await page.goto('/tools/digit-to-words/');
  await page.fill('#in-input', '1234');

  await expect(page.locator('#tool-output')).toHaveText('one thousand two hundred thirty-four', { timeout: 20_000 });
});

test('digit-to-words deep link renders check wording with non-default options', async ({ page }) => {
  const params = new URLSearchParams({
    input: '14,273.38',
    style: 'check',
    scale: 'short',
    letter_case: 'sentence',
    currency: 'USD',
    use_and: 'false',
    hyphenate: 'true',
    decimals: 'point',
    only_suffix: 'true',
  });

  await page.goto(`/tools/digit-to-words/?${params.toString()}`);
  await expect(page.locator('#in-style')).toHaveValue('check', { timeout: 15_000 });
  await expect(page.locator('#in-letter_case')).toHaveValue('sentence');
  await expect(page.locator('#in-only_suffix')).toBeChecked();

  await expect(page.locator('#tool-output')).toHaveText(
    'Fourteen thousand two hundred seventy-three and 38/100 dollars only',
    { timeout: 20_000 },
  );
});

test('digit-to-words page covers Indian scale, ordinal suffixes, and British and', async ({ page }) => {
  await page.goto('/tools/digit-to-words/');

  await page.fill('#in-input', '125000');
  await page.selectOption('#in-style', 'currency');
  await page.selectOption('#in-scale', 'indian');
  await page.selectOption('#in-letter_case', 'title');
  await page.selectOption('#in-currency', 'INR');
  await page.check('#in-only_suffix');
  await expect(page.locator('#tool-output')).toHaveText('One Lakh Twenty-Five Thousand Rupees Only', { timeout: 20_000 });

  await page.fill('#in-input', '1\n2\n103');
  await page.selectOption('#in-style', 'ordinal_num');
  await page.selectOption('#in-scale', 'short');
  await page.selectOption('#in-letter_case', 'lower');
  await page.selectOption('#in-currency', 'USD');
  await page.uncheck('#in-only_suffix');
  await expect(page.locator('#tool-output')).toHaveText('1st\n2nd\n103rd', { timeout: 20_000 });

  await page.fill('#in-input', '1023');
  await page.selectOption('#in-style', 'cardinal');
  await page.check('#in-use_and');
  await expect(page.locator('#tool-output')).toHaveText('one thousand and twenty-three', { timeout: 20_000 });
});
