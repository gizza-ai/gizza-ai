import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('hashtag-extractor ranks prose into paste-ready hashtags', async ({ page }) => {
  await page.goto('/tools/hashtag-extractor/');
  await setField(
    page,
    '#in-text',
    'Remote work is changing how teams build software. Async communication and clear documentation keep remote teams shipping fast.',
  );

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '#remote #teams #work #changing #build #software #async #communication #clear #documentation\n\n10 hashtags · 91 characters · 13 candidates found',
    { timeout: 15_000 },
  );
});

test('hashtag-extractor honors deep-linked platform and style options', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'Remote work is changing how teams build software. Async communication and clear documentation keep remote teams shipping fast.',
    max_tags: '0',
    platform: 'x',
    style: 'camel',
    phrase_words: '2',
    min_word_length: '4',
    include_existing: 'true',
    separator: 'comma',
  });
  await page.goto(`/tools/hashtag-extractor/?${params.toString()}`);

  await expect(page.locator('#in-platform')).toHaveValue('x');
  await expect(page.locator('#in-style')).toHaveValue('camel');
  await expect(page.locator('#in-separator')).toHaveValue('comma');
  await expect(page.locator('#tool-output')).toHaveText(
    '#remote, #remoteWork\n\n2 hashtags · 20 characters · 13 candidates found',
    { timeout: 15_000 },
  );
});

test('hashtag-extractor covers authored tags, checkbox, boundaries and newline separator', async ({ page }) => {
  await page.goto('/tools/hashtag-extractor/');
  await setField(page, '#in-text', 'Shipping the new release today. #DevLog #BuildInPublic');
  await setField(page, '#in-max_tags', '100');
  await page.selectOption('#in-platform', 'none');
  await page.selectOption('#in-style', 'lowercase');
  await setField(page, '#in-phrase_words', '1');
  await setField(page, '#in-min_word_length', '3');
  await page.uncheck('#in-include_existing');
  await page.selectOption('#in-separator', 'newline');

  await expect(page.locator('#tool-output')).toHaveText(
    '#shipping\n#new\n#release\n#today\n#devlog\n#buildinpublic\n\n6 hashtags · 53 characters',
    { timeout: 15_000 },
  );
});
