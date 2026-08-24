import { test, expect } from './fixtures';

const SAMPLE = '{"id":1,"title":"Hello world","published_at":"2026-01-02T03:04:05Z","views":42,"rating":4.5}';

async function outputJson(page) {
  const text = (await page.locator('#tool-output').textContent())?.trim() ?? '';
  return JSON.parse(text);
}

test('elasticsearch-mapping-generator infers a default mapping from JSON', async ({ page }) => {
  await page.goto('/tools/elasticsearch-mapping-generator/');
  await page.fill('#in-json', SAMPLE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"mappings"', { timeout: 15000 });
  const mapping = await outputJson(page);
  const props = mapping.mappings.properties;
  expect(props.id.type).toBe('long');
  expect(props.title.type).toBe('text');
  expect(props.title.fields.keyword).toEqual({ type: 'keyword', ignore_above: 256 });
  expect(props.published_at.type).toBe('date');
  expect(props.views.type).toBe('long');
  expect(props.rating.type).toBe('float');
});

test('elasticsearch-mapping-generator supports create-index output and option controls', async ({ page }) => {
  await page.goto('/tools/elasticsearch-mapping-generator/');
  await page.fill('#in-json', '{"client_ip":"203.0.113.7","status":"200","loc":{"lat":48.8566,"lon":2.3522},"lines":[{"sku":"A","qty":2}]}');
  await page.selectOption('#in-output', 'create-index');
  await page.selectOption('#in-text_fields', 'keyword');
  await page.check('#in-numeric_detection');
  await page.check('#in-detect_ip');
  await page.check('#in-detect_geo_point');
  await page.selectOption('#in-array_objects', 'nested');
  await page.selectOption('#in-dynamic', 'strict');
  await page.fill('#in-shards', '3');
  await page.fill('#in-replicas', '0');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"number_of_shards": 3', { timeout: 15000 });
  const body = await outputJson(page);
  expect(body.settings.index.number_of_shards).toBe(3);
  expect(body.settings.index.number_of_replicas).toBe(0);
  expect(body.mappings.dynamic).toBe('strict');
  expect(body.mappings.properties.client_ip.type).toBe('ip');
  expect(body.mappings.properties.status.type).toBe('long');
  expect(body.mappings.properties.loc.type).toBe('geo_point');
  expect(body.mappings.properties.lines.type).toBe('nested');
});

test('elasticsearch-mapping-generator deep-link pre-fills and runs properties output', async ({ page }) => {
  const params = new URLSearchParams({
    json: '{"headline":"Running shoes reviewed","score":"4.5"}',
    output: 'properties',
    text_fields: 'text',
    analyzer: 'english',
    numeric_detection: 'true',
  });
  await page.goto(`/tools/elasticsearch-mapping-generator/?${params.toString()}`);
  await expect(page.locator('#tool-output')).toContainText('"properties"', { timeout: 15000 });
  const body = await outputJson(page);
  expect(body.properties.headline).toEqual({ type: 'text', analyzer: 'english' });
  expect(body.properties.score.type).toBe('float');
});

test('elasticsearch-mapping-generator reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/elasticsearch-mapping-generator/');
  await page.fill('#in-json', '{bad}');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});
