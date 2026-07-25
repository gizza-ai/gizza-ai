import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const SAMPLE = 'Hello 👋 World 🚀 family 👨‍👩‍👧‍👦 flag 🇬🇧 thumb 👍🏽 key 1️⃣';

test('emoji-remover page: removes emoji clusters with default mode', async ({ page }) => {
  await page.goto('/tools/emoji-remover/');
  await page.fill('#in-text', SAMPLE);
  await expect(page.locator('#tool-output')).toContainText('Hello', { timeout: 15000 });
  expect(await outputText(page)).toBe('Hello  World  family  flag  thumb  key');
});

test('emoji-remover page: placeholder mode and text-symbol preservation', async ({ page }) => {
  await page.goto('/tools/emoji-remover/');
  await page.fill('#in-text', 'Great 👏 © ❤ ❤️');
  await page.selectOption('#in-mode', 'placeholder');
  await page.fill('#in-placeholder', '[emoji]');
  await page.check('#in-keep_text_symbols');
  await expect(page.locator('#tool-output')).toContainText('[emoji]', { timeout: 15000 });
  expect(await outputText(page)).toBe('Great [emoji] © ❤ [emoji]');
});

test('emoji-remover page: collapse whitespace checkbox tidies deletion gaps', async ({ page }) => {
  await page.goto('/tools/emoji-remover/');
  await page.fill('#in-text', '🚀 Hello 👋 World 🚀');
  await page.check('#in-collapse_whitespace');
  await expect(page.locator('#tool-output')).toContainText('Hello World', { timeout: 15000 });
  expect(await outputText(page)).toBe('Hello World');
});

test('emoji-remover page: query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/emoji-remover/?text=' +
      encodeURIComponent('Great work 👏 everyone 🎉') +
      '&mode=placeholder&placeholder=%5Bemoji%5D&collapse_whitespace=false&keep_text_symbols=false',
  );
  await expect(page.locator('#in-text')).toHaveValue('Great work 👏 everyone 🎉', { timeout: 15000 });
  await expect(page.locator('#in-mode')).toHaveValue('placeholder');
  await expect(page.locator('#tool-output')).toContainText('Great work [emoji] everyone [emoji]', { timeout: 15000 });
});
