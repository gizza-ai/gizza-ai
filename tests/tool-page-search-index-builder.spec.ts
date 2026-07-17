import { test, expect } from './fixtures';

// The tool emits a serialized inverted-index JSON; every case parses #tool-output
// as JSON and asserts real structure (df/postings/store/boosts), never substrings.
async function outputJson(page) {
  const raw = await page.locator('#tool-output').textContent();
  return JSON.parse(raw);
}

test('search-index-builder default run: auto-detected fields, df and term frequencies', async ({
  page,
}) => {
  await page.goto('/tools/search-index-builder/');
  await page.fill(
    '#in-documents',
    '[{"id":"a","title":"Hello World","body":"hello there"}]'
  );
  await expect(page.locator('#tool-output')).toContainText('documentCount', { timeout: 15000 });
  const v = await outputJson(page);
  expect(v.documentCount).toBe(1);
  expect(v.ref).toBe('id');
  // Fields auto-detected from string values, id excluded, sorted.
  expect(v.fields).toEqual(['body', 'title']);
  // "hello" appears in both the title and the body of doc a.
  expect(v.index.hello.title.df).toBe(1);
  expect(v.index.hello.title.postings.a).toBe(1);
  expect(v.index.hello.body.postings.a).toBe(1);
  // Distinct tokens: hello, world, there.
  expect(v.tokenCount).toBe(3);
  // No boosts supplied → key omitted entirely.
  expect(v.boosts).toBeUndefined();
});

test('search-index-builder non-default options: chosen fields, store, boosts, stopwords, min_length', async ({
  page,
}) => {
  await page.goto('/tools/search-index-builder/');
  await page.fill(
    '#in-documents',
    '[{"id":"1","title":"The Fast API","body":"Build the index and API"}]'
  );
  await page.fill('#in-fields', 'title,body');
  await page.fill('#in-store_fields', 'title');
  await page.fill('#in-boosts', 'title:3,body:1');
  await page.check('#in-remove_stopwords');
  await page.fill('#in-min_length', '2');
  await expect(page.locator('#tool-output')).toContainText('boosts', { timeout: 15000 });
  const v = await outputJson(page);
  // Boosts recorded verbatim for the query-time ranker.
  expect(v.boosts.title).toBe(3);
  expect(v.boosts.body).toBe(1);
  // "api" indexed under both fields for doc 1.
  expect(v.index.api.title.postings['1']).toBe(1);
  expect(v.index.api.body.postings['1']).toBe(1);
  // Stop words ("the", "and") dropped from the index.
  expect(v.index.the).toBeUndefined();
  expect(v.index.and).toBeUndefined();
  // Store carries only the chosen display field.
  expect(v.documents['1'].title).toBe('The Fast API');
});

test('search-index-builder lowercase=false keeps case-sensitive tokens, pretty-prints', async ({
  page,
}) => {
  await page.goto('/tools/search-index-builder/');
  await page.fill('#in-documents', '[{"id":"x","body":"API api Api"}]');
  await page.fill('#in-fields', 'body');
  await page.uncheck('#in-lowercase');
  await page.check('#in-pretty');
  await expect(page.locator('#tool-output')).toContainText('documentCount', { timeout: 15000 });
  const raw = await page.locator('#tool-output').textContent();
  // pretty=true → indented multi-line JSON.
  expect(raw).toContain('\n');
  const v = JSON.parse(raw);
  // Case preserved: three distinct tokens rather than one folded "api".
  expect(v.index.API.body.postings.x).toBe(1);
  expect(v.index.api.body.postings.x).toBe(1);
  expect(v.index.Api.body.postings.x).toBe(1);
  expect(v.tokenCount).toBe(3);
});

test('search-index-builder deep-link pre-fills and auto-runs with min_length', async ({ page }) => {
  const documents = '[{"id":"x","body":"go golang goroutine"}]';
  await page.goto(
    '/tools/search-index-builder/?' +
      new URLSearchParams({ documents, fields: 'body', min_length: '3' }).toString()
  );
  await expect(page.locator('#in-documents')).toHaveValue(documents, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('documentCount', { timeout: 15000 });
  const v = await outputJson(page);
  // "go" is under the 3-char minimum and dropped; the longer tokens survive.
  expect(v.index.go).toBeUndefined();
  expect(v.index.golang.body.postings.x).toBe(1);
  expect(v.index.goroutine.body.postings.x).toBe(1);
  expect(v.tokenCount).toBe(2);
});
