import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const TRIAGE = `billing,invoice charge refund
billing,subscription payment failed
support,password reset login issue
support,account locked cannot sign in
sales,pricing quote enterprise plan
sales,demo request for buying team`;

test('few-shot-text-classifier page predicts support with exact report text', async ({ page }) => {
  await page.goto('/tools/few-shot-text-classifier/');
  await setField(page, '#in-examples', TRIAGE);
  await setField(page, '#in-text', 'I cannot sign in after resetting my password.');
  await page.selectOption('#in-separator', 'comma');

  await expect(page.locator('#tool-output')).toContainText('Prediction:  support', { timeout: 15_000 });
  const out = await outputText(page);
  expect(out).toContain('Confidence:  100.0%');
  expect(out).toContain('Similarity:  0.6667 (cosine)');
  expect(out).toContain('support      0.6667      100.0%');
  expect(out).toContain('[support] account locked cannot sign in');
});

test('few-shot-text-classifier deep-link batch labels lines as CSV', async ({ page }) => {
  const params = new URLSearchParams({
    examples: `bug|crashes when I open the settings panel
bug|export button throws an error
praise|love the new dashboard and charts
praise|the app feels faster after the update
feature|please add dark mode to reports
feature|need an API endpoint for bulk import`,
    text: `The import job failed with an error.
Can you add a dark theme?
The dashboard loads very quickly now.`,
    separator: 'pipe',
    input_mode: 'lines',
    method: 'knn',
    k: '3',
    similarity: 'cosine',
    weighting: 'tfidf',
    analyzer: 'word',
    ngram_max: '1',
    lowercase: 'true',
    strip_accents: 'false',
    remove_stopwords: 'false',
    sublinear_tf: 'false',
    min_df: '1',
    min_confidence: '0',
    top_k: '3',
    explain: 'true',
    output: 'csv',
  });
  await page.goto(`/tools/few-shot-text-classifier/?${params.toString()}`);

  await expect(page.locator('#in-separator')).toHaveValue('pipe', { timeout: 15_000 });
  await expect(page.locator('#in-input_mode')).toHaveValue('lines');
  await expect(page.locator('#in-method')).toHaveValue('knn');
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('"feature",1.0000', { timeout: 15_000 });
  expect(await outputText(page)).toBe(
    'text,prediction,confidence,similarity,top_terms\n"The import job failed with an error.","bug",0.4333,0.4358,"error an"\n"Can you add a dark theme?","feature",1.0000,0.5774,"add dark"\n"The dashboard loads very quickly now.","praise",0.8411,0.5196,"dashboard the"',
  );
});

test('few-shot-text-classifier supports non-default preprocessing and boundary values', async ({ page }) => {
  await page.goto('/tools/few-shot-text-classifier/');
  await setField(page, '#in-examples', 'fruit: apple banana orange pear\nfruit: berry mango melon grape\nanimal: kitten puppy hamster rabbit\nanimal: tiger zebra elephant horse');
  await setField(page, '#in-text', 'bananna and oragne smoothie');
  await page.selectOption('#in-separator', 'colon');
  await page.selectOption('#in-method', 'best-match');
  await page.selectOption('#in-similarity', 'jaccard');
  await page.selectOption('#in-weighting', 'binary');
  await page.selectOption('#in-analyzer', 'char');
  await setField(page, '#in-ngram_max', '3');
  await page.check('#in-strip_accents');
  await page.uncheck('#in-explain');
  await setField(page, '#in-top_k', '50');

  await expect(page.locator('#tool-output')).toContainText('Prediction:  fruit', { timeout: 15_000 });
  const out = await outputText(page);
  expect(out).toContain('jaccard');
  expect(out).toContain('analyzer=char  ngram_max=3');
  expect(out).not.toContain('Top matching terms');
});

test('few-shot-text-classifier shows a runnable generated CLI example', async ({ page }) => {
  await page.goto('/tools/few-shot-text-classifier/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool few-shot-text-classifier');
  expect(cli).toContain('billing,invoice charge refund');
  expect(cli).toContain('text=');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
