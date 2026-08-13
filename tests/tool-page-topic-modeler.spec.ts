import { test, expect } from './fixtures';

const corpus = [
  'Butter flour sugar oven baking pastry crust',
  'Sugar butter dough oven baking recipe',
  '',
  'Compiler module function type returns value',
  'Module compiler type function signature error',
].join('\n');

test('topic-modeler reports topics and document mixtures', async ({ page }) => {
  await page.goto('/tools/topic-modeler/');
  await page.fill('#in-documents', corpus);
  await page.fill('#in-topics', '2');
  await page.fill('#in-words_per_topic', '5');
  await page.fill('#in-iterations', '100');
  await page.fill('#in-seed', '42');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('LDA topic model', { timeout: 20000 });
  await expect(output).toContainText('2 documents · 2 topics');
  await expect(output).toContainText('Topics (ranked by share of the corpus)');
  await expect(output).toContainText('Document mixtures');
});

test('topic-modeler deep link can emit CSV from line-separated documents', async ({ page }) => {
  const docs = [
    'solar battery grid inverter energy',
    'battery solar panel grid power',
    'invoice ledger budget payment revenue',
    'budget invoice payment ledger close',
  ].join('\n');
  const qs =
    '?documents=' + encodeURIComponent(docs) +
    '&separator=line' +
    '&topics=2' +
    '&words_per_topic=4' +
    '&iterations=100' +
    '&seed=7' +
    '&output=csv';

  await page.goto('/tools/topic-modeler/' + qs);
  await expect(page.locator('#in-separator')).toHaveValue('line', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#in-topics')).toHaveValue('2');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('topic,share,top_words', { timeout: 20000 });
  await expect(output).toContainText('document,words,dominant_topic,topic_1,topic_2');
  await expect(output).toContainText('battery');
  await expect(output).toContainText('invoice');
});
