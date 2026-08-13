import { test, expect } from './fixtures';

const corpus = ['the cat sat', 'the dog sat', 'the cat chased the dog'].join('\n');

test('document-term-matrix builds a real CSV matrix', async ({ page }) => {
  await page.goto('/tools/document-term-matrix/');
  await page.fill('#in-documents', corpus);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('document,the,cat,dog,sat,chased,__total_terms', { timeout: 20000 });
  await expect(output).toContainText('doc_1,1,1,0,1,0,3');
  await expect(output).toContainText('doc_3,2,1,1,0,1,5');
});

test('document-term-matrix deep link supports binary bigrams', async ({ page }) => {
  const qs =
    '?documents=' + encodeURIComponent('quick red fox\nquick blue fox') +
    '&input_format=lines' +
    '&weighting=binary' +
    '&ngram_min=1' +
    '&ngram_max=2' +
    '&output=csv' +
    '&include_totals=true';

  await page.goto('/tools/document-term-matrix/' + qs);
  await expect(page.locator('#in-weighting')).toHaveValue('binary', { timeout: 15000 });
  await expect(page.locator('#in-ngram_max')).toHaveValue('2');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('document,fox,quick,blue,blue fox,quick blue,quick red,red,red fox,__total_terms', { timeout: 20000 });
  await expect(output).toContainText('doc_1,1,1,0,0,0,1,1,1,5');
  await expect(output).toContainText('doc_2,1,1,1,1,1,0,0,0,5');
});

test('document-term-matrix renders structured JSON from JSON-array input', async ({ page }) => {
  const qs =
    '?documents=' + encodeURIComponent('["Alpha beta beta", "Beta gamma"]') +
    '&input_format=json' +
    '&output=json' +
    '&include_totals=true';

  await page.goto('/tools/document-term-matrix/' + qs);
  await expect(page.locator('#in-input_format')).toHaveValue('json', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('json');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"terms": [', { timeout: 20000 });
  await expect(output).toContainText('"beta"');
  await expect(output).toContainText('"matrix": [');
  await expect(output).toContainText('2');
});
