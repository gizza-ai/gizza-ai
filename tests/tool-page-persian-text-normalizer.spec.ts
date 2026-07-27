import { test, expect } from './fixtures';

const messy = 'كتاب123 را می خواهم ,خوب';

// /tools/persian-text-normalizer/ normalizes Persian/Farsi text in-browser.
test('persian-text-normalizer renders exact default cleanup output', async ({ page }) => {
  await page.goto('/tools/persian-text-normalizer/');
  await page.fill('#in-text', messy);

  await expect(page.locator('#tool-output')).toHaveText('کتاب۱۲۳ را می‌خواهم, خوب', { timeout: 15000 });
});

test('persian-text-normalizer deep-link applies non-default punctuation and diacritic options', async ({ page }) => {
  const text = 'سلامٌ, حالت خوبه? مُحَمَّد';
  await page.goto(
    '/tools/persian-text-normalizer/?text=' +
      encodeURIComponent(text) +
      '&characters=true&digits=persian&half_space=true&punctuation_spacing=true&persian_punctuation=true&remove_diacritics=true&whitespace=true',
  );

  await expect(page.locator('#in-text')).toHaveValue(text, { timeout: 15000 });
  await expect(page.locator('#in-persian_punctuation')).toBeChecked();
  await expect(page.locator('#in-remove_diacritics')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('سلام، حالت خوبه؟ محمد', { timeout: 15000 });
});

test('persian-text-normalizer supports each digits mode', async ({ page }) => {
  await page.goto('/tools/persian-text-normalizer/');
  await page.fill('#in-text', 'شماره 123 و ٤٥٦ و ۷۸۹');

  await page.selectOption('#in-digits', 'persian');
  await expect(page.locator('#tool-output')).toHaveText('شماره ۱۲۳ و ۴۵۶ و ۷۸۹', { timeout: 15000 });

  await page.selectOption('#in-digits', 'english');
  await expect(page.locator('#tool-output')).toHaveText('شماره 123 و 456 و 789', { timeout: 15000 });

  await page.selectOption('#in-digits', 'keep');
  await expect(page.locator('#tool-output')).toHaveText('شماره 123 و ٤٥٦ و ۷۸۹', { timeout: 15000 });
});

test('persian-text-normalizer can disable default half-space and character folding', async ({ page }) => {
  await page.goto('/tools/persian-text-normalizer/');
  await page.fill('#in-text', 'كتاب ها می روم');
  await page.uncheck('#in-characters');
  await page.uncheck('#in-half_space');

  await expect(page.locator('#tool-output')).toHaveText('كتاب ها می روم', { timeout: 15000 });
});
