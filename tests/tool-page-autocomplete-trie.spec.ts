import { test, expect } from './fixtures';

const tool = '/tools/autocomplete-trie/';
const list = 'apple,12\napplication,7\napply,3\napricot,5\nbanana,9\nband,4';

test('autocomplete-trie page returns weighted prefix suggestions', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-wordlist', list);
  await page.fill('#in-prefix', 'app');
  await expect(page.locator('#tool-output')).toContainText('Suggestions for "app": 3 of 3 matches', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('1. apple        weight 12');
  await expect(page.locator('#tool-output')).toContainText('2. application  weight 7');
  await expect(page.locator('#tool-output')).toContainText('3. apply        weight 3');
});

test('autocomplete-trie page supports typo tolerance', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-wordlist', list);
  await page.fill('#in-prefix', 'aple');
  await page.fill('#in-max_typos', '1');
  await expect(page.locator('#tool-output')).toContainText('apple', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('(1 typo)');
});

test('autocomplete-trie page renders trie drawing', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-wordlist', 'car,1\ncart,1\ncarton,1\ncardigan,1');
  await page.fill('#in-prefix', 'car');
  await page.selectOption('#in-output', 'trie');
  await expect(page.locator('#tool-output')).toContainText('Trie for "car": 4 terms below this prefix', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('car*  -> car (weight 1)');
  await expect(page.locator('#tool-output')).toContainText('* marks a node where a stored term ends.');
});

test('autocomplete-trie query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    tool +
      '?wordlist=' +
      encodeURIComponent(list) +
      '&prefix=' +
      encodeURIComponent('a') +
      '&rank=alphabetical&limit=2',
  );
  await expect(page.locator('#in-wordlist')).toHaveValue(list, { timeout: 15000 });
  await expect(page.locator('#in-prefix')).toHaveValue('a');
  await expect(page.locator('#in-rank')).toHaveValue('alphabetical');
  await expect(page.locator('#in-limit')).toHaveValue('2');
  await expect(page.locator('#tool-output')).toContainText('Suggestions for "a": 2 of 4 matches');
  await expect(page.locator('#tool-output')).toContainText('1. apple');
  await expect(page.locator('#tool-output')).toContainText('2. application');
});
