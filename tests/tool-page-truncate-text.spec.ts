import { test, expect } from './fixtures';

async function fillText(page: any, selector: string, value: string) {
  await page.$eval(
    selector,
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('truncate-text page truncates by characters without splitting words', async ({ page }) => {
  await page.goto('/tools/truncate-text/');
  await fillText(page, '#in-text', 'the quick brown fox');
  await page.fill('#in-length', '12');
  await expect(page.locator('#tool-output')).toHaveText('the quick…', { timeout: 15000 });
});

test('truncate-text page supports word mode and custom suffix', async ({ page }) => {
  await page.goto('/tools/truncate-text/');
  await page.selectOption('#in-unit', 'words');
  await page.fill('#in-length', '3');
  await page.fill('#in-ellipsis', ' [more]');
  await fillText(page, '#in-text', 'the quick brown fox jumps');
  await expect(page.locator('#tool-output')).toHaveText('the quick brown [more]', {
    timeout: 15000,
  });
});

test('truncate-text page can hard-cut inside a word', async ({ page }) => {
  await page.goto('/tools/truncate-text/');
  await page.fill('#in-length', '10');
  await page.check('#in-break_words');
  await fillText(page, '#in-text', 'supercalifragilisticexpialidocious');
  await expect(page.locator('#tool-output')).toHaveText('supercali…', { timeout: 15000 });
});

test('truncate-text query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/truncate-text/?text=' +
      encodeURIComponent('one two three four') +
      '&length=2&unit=words&ellipsis=' +
      encodeURIComponent('...') +
      '&count_ellipsis=true&break_words=false',
  );
  await expect(page.locator('#in-text')).toHaveValue('one two three four', { timeout: 15000 });
  await expect(page.locator('#in-unit')).toHaveValue('words');
  await expect(page.locator('#in-length')).toHaveValue('2');
  await expect(page.locator('#tool-output')).toHaveText('one two...', { timeout: 15000 });
});
