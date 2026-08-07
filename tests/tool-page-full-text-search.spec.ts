import { test, expect } from './fixtures';

// Three policy docs separated by --- rules; each doc's first line is its title.
const CORPUS = [
  'Refund policy',
  'Refunds take five business days.',
  '---',
  'Shipping guide',
  'Orders ship within two days.',
  '---',
  'Return labels',
  'Print a return label before requesting a refund.',
].join('\n');

// Exact output: BM25 ranks the title-matching refund doc above the doc that only
// mentions "refund" in its body, and every matched word is wrapped in «…».
test('full-text-search page ranks BM25 hits with highlighted snippets', async ({ page }) => {
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', CORPUS);
  await page.fill('#in-query', 'refund');
  await page.fill('#in-snippet_words', '12');
  await expect(page.locator('#tool-output')).toContainText('BM25', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 2 of 3 documents match "refund"\n\n' +
      '#1  doc 1  score 0.76  Refund policy\n' +
      '    «Refund» policy «Refunds» take five business days\n\n' +
      '#2  doc 3  score 0.43  Return labels\n' +
      '    Return labels Print a return label before requesting a «refund»'
  );
  // The unrelated shipping doc is ranked out, not merely pushed down.
  expect(out).not.toContain('Shipping guide');
});

// Deep-link: params prefill the corpus/query fields and the tool auto-computes.
test('full-text-search deep-link pre-fills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/full-text-search/?' +
      new URLSearchParams({ corpus: CORPUS, query: 'refund', snippet_words: '12' }).toString()
  );
  await expect(page.locator('#in-corpus')).toHaveValue(CORPUS, { timeout: 15000 });
  await expect(page.locator('#in-query')).toHaveValue('refund');
  await expect(page.locator('#tool-output')).toContainText('BM25', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 2 of 3 documents match "refund"\n\n' +
      '#1  doc 1  score 0.76  Refund policy\n' +
      '    «Refund» policy «Refunds» take five business days\n\n' +
      '#2  doc 3  score 0.43  Return labels\n' +
      '    Return labels Print a return label before requesting a «refund»'
  );
});

// Enum "algorithm": tfidf swaps the header label and the classic log-TF x IDF
// scores in for BM25's saturated ones.
test('full-text-search algorithm=tfidf scores with classic TF-IDF', async ({ page }) => {
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', CORPUS);
  await page.fill('#in-query', 'refund');
  await page.fill('#in-snippet_words', '12');
  await page.selectOption('#in-algorithm', 'tfidf');
  await expect(page.locator('#tool-output')).toContainText('TF-IDF', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'TF-IDF · 2 of 3 documents match "refund"\n\n' +
      '#1  doc 1  score 1.02  Refund policy\n' +
      '    «Refund» policy «Refunds» take five business days\n\n' +
      '#2  doc 3  score 0.41  Return labels\n' +
      '    Return labels Print a return label before requesting a «refund»'
  );
});

// Enum "separator": blank-line splits on empty lines, so a corpus with no ---
// rules still becomes two documents with their own titles.
test('full-text-search separator=blank-line splits on empty lines', async ({ page }) => {
  await page.goto('/tools/full-text-search/');
  await page.fill(
    '#in-corpus',
    'Refund policy\nRefunds take five business days.\n\nShipping guide\nOrders ship within two days.'
  );
  await page.fill('#in-query', 'refund');
  await page.fill('#in-snippet_words', '12');
  await page.selectOption('#in-separator', 'blank-line');
  await expect(page.locator('#tool-output')).toContainText('BM25', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 1 of 2 documents match "refund"\n\n' +
      '#1  doc 1  score 1.09  Refund policy\n' +
      '    «Refund» policy «Refunds» take five business days'
  );
});

// Enum "separator": form-feed splits on \f, the page break extractors emit.
test('full-text-search separator=form-feed splits on \\f page breaks', async ({ page }) => {
  await page.goto('/tools/full-text-search/');
  await page.fill(
    '#in-corpus',
    'Refund policy\nRefunds take five business days.\fShipping guide\nOrders ship within two days.'
  );
  await page.fill('#in-query', 'refund');
  await page.fill('#in-snippet_words', '12');
  await page.selectOption('#in-separator', 'form-feed');
  await expect(page.locator('#tool-output')).toContainText('BM25', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 1 of 2 documents match "refund"\n\n' +
      '#1  doc 1  score 1.09  Refund policy\n' +
      '    «Refund» policy «Refunds» take five business days'
  );
});

// Enum "match": all (AND) keeps only the one document holding every query term,
// where the any (OR) default would have returned two.
test('full-text-search match=all requires every term', async ({ page }) => {
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', CORPUS);
  await page.fill('#in-query', 'refund label');
  await page.fill('#in-snippet_words', '12');
  await expect(page.locator('#tool-output')).toContainText('2 of 3 documents', { timeout: 15000 });
  await page.selectOption('#in-match', 'all');
  await expect(page.locator('#tool-output')).toContainText('1 of 3 documents', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 1 of 3 documents match "refund label"\n\n' +
      '#1  doc 3  score 1.89  Return labels\n' +
      '    Return «labels» Print a return «label» before requesting a «refund»'
  );
});

// Non-default checkbox: prefix on — "moto" starts to match "motorcycle" and
// "motor", where the default off state finds nothing at all.
test('full-text-search prefix checkbox matches word beginnings', async ({ page }) => {
  const corpus = 'Motorcycle notes\nMotorcycle helmets and motor oil.\n---\nCar notes\nCar tires and oil changes.';
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', corpus);
  await page.fill('#in-query', 'moto');
  await page.fill('#in-snippet_words', '10');
  await expect(page.locator('#tool-output')).toHaveText(
    'No documents match "moto" (searched 2 documents).',
    { timeout: 15000 }
  );
  await page.check('#in-prefix');
  await expect(page.locator('#tool-output')).toContainText('BM25', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 1 of 2 documents match "moto"\n\n' +
      '#1  doc 1  score 1.17  Motorcycle notes\n' +
      '    «Motorcycle» notes «Motorcycle» helmets and «motor» oil'
  );
});

// Non-default checkbox: stemming off — "run" stops reaching "running", so the
// doc that only has the inflected form drops out of the results.
test('full-text-search stemming checkbox off stops matching inflections', async ({ page }) => {
  const corpus = 'Running notes\nThe team is running daily experiments.\n---\nArchive\nA prior run finished yesterday.';
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', corpus);
  await page.fill('#in-query', 'run');
  await page.fill('#in-snippet_words', '12');
  // Stemming on (default): "running" and "run" both match.
  await expect(page.locator('#tool-output')).toContainText('2 of 2 documents', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('«Running»');
  await page.uncheck('#in-stemming');
  await expect(page.locator('#tool-output')).toContainText('1 of 2 documents', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 1 of 2 documents match "run"\n\n' +
      '#1  doc 2  score 0.74  Archive\n' +
      '    Archive A prior «run» finished yesterday'
  );
  expect(out).not.toContain('Running notes');
});

// Non-default checkbox: stopwords off — "the" is kept as a real query term, so a
// doc that shares only the stop word now matches and gets it highlighted.
test('full-text-search stopwords checkbox off keeps common words', async ({ page }) => {
  const corpus =
    'Refund policy\nThe refund takes five business days.\n---\nShipping guide\nThe orders ship within two days.';
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', corpus);
  await page.fill('#in-query', 'the refund');
  await page.fill('#in-snippet_words', '12');
  await expect(page.locator('#tool-output')).toContainText('1 of 2 documents', { timeout: 15000 });
  await page.uncheck('#in-stopwords');
  await expect(page.locator('#tool-output')).toContainText('2 of 2 documents', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    'BM25 · 2 of 2 documents match "the refund"\n\n' +
      '#1  doc 1  score 1.27  Refund policy\n' +
      '    «Refund» policy «The» «refund» takes five business days\n\n' +
      '#2  doc 2  score 0.18  Shipping guide\n' +
      '    Shipping guide «The» orders ship within two days'
  );
});

// Cap boundary: 60 matching documents, max_results at the cap (50) and one over
// (51, clamped to 50) both report the same "showing top 50" truncation.
test('full-text-search max_results caps at 50', async ({ page }) => {
  const many = Array.from({ length: 60 }, (_, i) => `Doc ${i + 1}\ncat number ${i + 1} here.`).join(
    '\n---\n'
  );
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', many);
  await page.fill('#in-query', 'cat');
  await page.fill('#in-snippet_words', '0');
  await page.fill('#in-max_results', '50');
  await expect(page.locator('#tool-output')).toContainText(
    'BM25 · 60 of 60 documents match "cat" (showing top 50)',
    { timeout: 15000 }
  );
  const out = await page.locator('#tool-output').textContent();
  // Exactly 50 ranked hits, #1 through #50 and no #51.
  expect(out.match(/^#\d+ {2}doc /gm)).toHaveLength(50);
  expect(out).toContain('#50  doc 50  score 0.01  Doc 50');
  expect(out).not.toContain('#51');
  // snippet_words=0 → ranked hits only, no indented snippet lines.
  expect(out).not.toContain('\n    ');
  // One over the cap is clamped, not an error.
  await page.fill('#in-max_results', '51');
  await expect(page.locator('#tool-output')).toContainText(
    'BM25 · 60 of 60 documents match "cat" (showing top 50)',
    { timeout: 15000 }
  );
});

// Cap boundary: snippet_words at the 120 maximum widens the keyword-in-context
// window to a full 120 words, elided on both sides.
test('full-text-search snippet_words widens context up to 120', async ({ page }) => {
  const alpha = Array.from({ length: 100 }, (_, i) => `alpha${i + 1}`).join(' ');
  const beta = Array.from({ length: 100 }, (_, i) => `beta${i + 101}`).join(' ');
  await page.goto('/tools/full-text-search/');
  await page.fill('#in-corpus', `Long doc\n${alpha} needle ${beta}`);
  await page.fill('#in-query', 'needle');
  await page.fill('#in-snippet_words', '8');
  await expect(page.locator('#tool-output')).toContainText('«needle»', { timeout: 15000 });
  const narrow = await page.locator('#tool-output').textContent();
  expect(narrow).toBe(
    'BM25 · 1 of 1 document match "needle"\n\n' +
      '#1  doc 1  score 0.29  Long doc\n' +
      '    …alpha97 alpha98 alpha99 alpha100 «needle» beta101 beta102 beta103…'
  );

  await page.fill('#in-snippet_words', '120');
  await expect(page.locator('#tool-output')).toContainText('alpha41', { timeout: 15000 });
  const wide = await page.locator('#tool-output').textContent();
  const snippet = wide.split('\n')[3].trim();
  expect(snippet.split(/\s+/)).toHaveLength(120);
  expect(snippet).toContain('«needle»');
  // Still truncated on both sides of the 201-word document.
  expect(snippet.startsWith('…alpha41')).toBe(true);
  expect(snippet.endsWith('…')).toBe(true);
});
