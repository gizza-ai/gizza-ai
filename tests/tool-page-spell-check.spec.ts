import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('spell-check page reports misspellings and corrected text exactly', async ({ page }) => {
  await page.goto('/tools/spell-check/');
  await page.fill('#in-text', 'I recieve teh enviroment.');
  await page.fill('#in-max_suggestions', '1');
  await expect(page.locator('#tool-output')).toContainText('3 possible misspellings found (3 words checked):', { timeout: 15000 });
  expect(await outputText(page)).toBe([
    '3 possible misspellings found (3 words checked):',
    '',
    'recieve → receive',
    'teh → the',
    'enviroment → environment',
    '',
    'Corrected text:',
    'I receive the environment.',
  ].join('\n'));
});

test('spell-check page honors custom words and non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/spell-check/');
  await page.fill('#in-text', 'NASA ZZZQ gizza');
  await page.fill('#in-custom_words', 'gizza');
  await page.uncheck('#in-ignore_uppercase');
  await expect(page.locator('#tool-output')).toContainText('ZZZQ →', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('ZZZQ →');
  expect(text).not.toContain('NASA →');
  expect(text).not.toContain('gizza →');
});

test('spell-check deep-link pre-fills and auto-runs', async ({ page }) => {
  const text = encodeURIComponent('acheive definately');
  await page.goto(`/tools/spell-check/?text=${text}&max_suggestions=1&ignore_uppercase=true&ignore_capitalized=false&custom_words=`);
  await expect(page.locator('#in-text')).toHaveValue('acheive definately', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('2 possible misspellings found (2 words checked):', { timeout: 15000 });
  expect(await outputText(page)).toBe([
    '2 possible misspellings found (2 words checked):',
    '',
    'acheive → achieve',
    'definately → definitely',
    '',
    'Corrected text:',
    'achieve definitely',
  ].join('\n'));
});
