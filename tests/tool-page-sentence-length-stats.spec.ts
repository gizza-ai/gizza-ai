import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('sentence-length-stats reports sentence count and averages', async ({ page }) => {
  await page.goto('/tools/sentence-length-stats/');
  await setField(
    page,
    '#in-text',
    'Short sentences land fast. Longer sentences can carry nuance, context, and rhythm when they are placed carefully. Mix both.',
  );

  await expect(page.locator('#tool-output')).toContainText('Sentences: 3', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Average length: 6.3 words', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('Distribution (words per sentence)', {
    timeout: 15_000,
  });
});

test('sentence-length-stats deep-link uses line mode and lists longest sentence', async ({ page }) => {
  const qs = new URLSearchParams({
    text: 'First line\nSecond caption line with more words\nThird',
    newlines: 'always',
    long_threshold: '5',
    list_longest: '1',
    extra_abbreviations: '',
  });
  await page.goto(`/tools/sentence-length-stats/?${qs.toString()}`);

  await expect(page.locator('#in-newlines')).toHaveValue('always');
  await expect(page.locator('#tool-output')).toContainText('Sentences: 3', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Long sentences (5+ words): 1 of 3', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('1. 6 words (sentence 2):', {
    timeout: 15_000,
  });
});

test('sentence-length-stats validates numeric boundaries', async ({ page }) => {
  await page.goto('/tools/sentence-length-stats/');
  await setField(page, '#in-text', 'One sentence. Two sentence.');
  await setField(page, '#in-long_threshold', '0');
  await expect(page.locator('#tool-output')).toContainText('long_threshold is 0', {
    timeout: 15_000,
  });

  await setField(page, '#in-long_threshold', '25');
  await setField(page, '#in-list_longest', '51');
  await expect(page.locator('#tool-output')).toContainText('list_longest is 51', {
    timeout: 15_000,
  });
});
