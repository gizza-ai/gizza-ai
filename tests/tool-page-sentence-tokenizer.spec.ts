import { test, expect } from './fixtures';

async function outputText(page) {
  return await page.locator('#tool-output').textContent({ timeout: 20000 });
}

async function outputJson(page) {
  const text = await outputText(page);
  return JSON.parse(text ?? '');
}

test('sentence-tokenizer returns sentence and token spans as JSON', async ({ page }) => {
  await page.goto('/tools/sentence-tokenizer/');
  await page.fill('#in-text', 'Dr. Green paid $99.99. It works.');
  await page.selectOption('#in-format', 'json');

  const report = await outputJson(page);
  expect(report.counts.sentences).toBe(2);
  expect(report.counts.tokens).toBe(9);
  expect(report.sentences[0].text).toBe('Dr. Green paid $99.99.');
  expect(report.sentences[0].start).toBe(0);
  expect(report.sentences[0].end).toBe(22);
  expect(report.sentences[0].tokens.map((t) => [t.text, t.type, t.start, t.end])).toEqual([
    ['Dr.', 'word', 0, 3],
    ['Green', 'word', 4, 9],
    ['paid', 'word', 10, 14],
    ['$', 'symbol', 15, 16],
    ['99.99', 'number', 16, 21],
    ['.', 'punct', 21, 22],
  ]);
});

test('sentence-tokenizer deep link applies format, newline mode and checkbox flags', async ({ page }) => {
  const qs =
    '?text=' + encodeURIComponent('Hello, World!\nBye.') +
    '&format=lines' +
    '&newlines=always' +
    '&split_contractions=true' +
    '&split_hyphenated=false' +
    '&lowercase=true' +
    '&drop_punctuation=true';

  await page.goto('/tools/sentence-tokenizer/' + qs);
  await expect(page.locator('#in-format')).toHaveValue('lines', { timeout: 15000 });
  await expect(page.locator('#in-newlines')).toHaveValue('always');
  await expect(page.locator('#in-lowercase')).toBeChecked();
  await expect(page.locator('#in-drop_punctuation')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('hello\nworld\nbye', { timeout: 20000 });
});

test('sentence-tokenizer supports TSV, sentence output and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/sentence-tokenizer/');
  await page.fill('#in-text', "They don't like state-of-the-art gear.");
  await page.selectOption('#in-format', 'table');
  await page.check('#in-split_hyphenated');

  await expect(page.locator('#tool-output')).toContainText('sentence\ttoken\tstart\tend\ttype\ttext', { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText("1\t2\t5\t7\tword\tdo");
  await expect(page.locator('#tool-output')).toContainText('1\t6\t21\t22\tpunct\t-');

  await page.selectOption('#in-format', 'sentences');
  await expect(page.locator('#tool-output')).toHaveText("They don't like state-of-the-art gear.", { timeout: 20000 });
});

test('sentence-tokenizer honours extra abbreviations and reports invalid input', async ({ page }) => {
  await page.goto('/tools/sentence-tokenizer/');
  await page.fill('#in-text', 'Ship to Acme Blarg. Then invoice.');
  await page.selectOption('#in-format', 'sentences');
  await page.fill('#in-extra_abbreviations', 'Blarg.');

  await expect(page.locator('#tool-output')).toHaveText('Ship to Acme Blarg. Then invoice.', { timeout: 20000 });

  await page.fill('#in-text', '   ');
  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('text is empty');
});
