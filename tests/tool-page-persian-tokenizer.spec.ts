import { test, expect } from './fixtures';

const sample = 'ما کتاب می‌خوانیم. حال شما چطور است؟ قیمت ۱٬۲۵۰ تومان است.';

test('persian-tokenizer page tokenizes words with ZWNJ and punctuation', async ({ page }) => {
  await page.goto('/tools/persian-tokenizer/');
  await page.fill('#in-text', sample);
  await expect(page.locator('#tool-output')).toContainText('می‌خوانیم');
  const out = (await page.locator('#tool-output').textContent())!;
  expect(out).toContain('ما\nکتاب\nمی‌خوانیم\n.');
  expect(out).toContain('؟');
  expect(out).toContain('۱٬۲۵۰');
});

test('persian-tokenizer deep-link splits ZWNJ compounds and removes punctuation', async ({ page }) => {
  const text = encodeURIComponent('ما کتاب می‌خوانیم و کتاب‌ها را نمی‌بندیم.');
  await page.goto(`/tools/persian-tokenizer/?text=${text}&mode=words&format=space-separated&punctuation=remove&split_zwnj=true`);
  await expect(page.locator('#in-split_zwnj')).toBeChecked();
  await expect(page.locator('#in-punctuation')).toHaveValue('remove');
  await expect(page.locator('#in-format')).toHaveValue('space-separated');
  await expect(page.locator('#tool-output')).toHaveText('ما کتاب می خوانیم و کتاب ها را نمی بندیم');
});

test('persian-tokenizer emits sentence JSON with counts', async ({ page }) => {
  await page.goto('/tools/persian-tokenizer/');
  await page.fill('#in-text', 'سلام دنیا. خوبی؟');
  await page.selectOption('#in-mode', 'both');
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('"sentence_count":2');
  const json = JSON.parse((await page.locator('#tool-output').textContent())!);
  expect(json.mode).toBe('both');
  expect(json.sentence_count).toBe(2);
  expect(json.token_count).toBe(5);
  expect(json.sentences[1].text).toBe('خوبی؟');
});

test('persian-tokenizer advertised options stay wired', async ({ page }) => {
  await page.goto('/tools/persian-tokenizer/');
  await page.fill('#in-text', 'كتابي\u{064E} خوب است. info@example.com');
  await page.selectOption('#in-mode', 'words');
  await page.selectOption('#in-format', 'lines');
  await page.selectOption('#in-punctuation', 'remove');
  await page.uncheck('#in-normalize');
  await page.uncheck('#in-keep_entities');
  await expect(page.locator('#in-normalize')).not.toBeChecked();
  await expect(page.locator('#in-keep_entities')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('كتابيَ');
  const out = (await page.locator('#tool-output').textContent())!;
  expect(out).toContain('info\nexample\ncom');
});

test('persian-tokenizer preset and generated CLI example are generic', async ({ page }) => {
  await page.goto('/tools/persian-tokenizer/');
  await page.getByRole('button', { name: 'Sentences', exact: true }).click();
  await expect(page.locator('#in-mode')).toHaveValue('sentences');
  await expect(page.locator('#in-format')).toHaveValue('numbered');
  await expect(page.locator('#tool-output')).toContainText('1. حال شما چطور است؟');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe('gizza tool persian-tokenizer "ما کتاب می‌خوانیم. یادگیری خوب است."');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
