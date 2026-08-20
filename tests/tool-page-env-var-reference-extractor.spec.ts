import { test, expect } from './fixtures';

const shellSample = [
  'PORT=${PORT:-8080}',
  'curl "$API_URL/health"',
  'echo "$PORT" # $IGNORED_COMMENT',
  '',
].join('\n');

test('env-var-reference-extractor page lists shell references with defaults and status', async ({ page }) => {
  await page.goto('/tools/env-var-reference-extractor/');
  await page.fill('#in-text', shellSample);
  await page.selectOption('#in-syntax', 'shell');
  await page.selectOption('#in-output', 'table');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('API_URL', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`VARIABLE  USES  LINES  DEFAULT  STATUS
API_URL   1     2               undefined
PORT      2     1, 3   8080     defined`);
});

test('env-var-reference-extractor page deep-link generates dotenv template', async ({ page }) => {
  const text = 'DB_URL=${DATABASE_URL:-postgres://localhost/dev}\nSTRIPE=$STRIPE_KEY\n';
  const qs =
    '?text=' + encodeURIComponent(text) +
    '&syntax=shell' +
    '&output=env-template' +
    '&include_defined_in_source=false' +
    '&skip_comments=true' +
    '&only_undefined=false' +
    '&sort=name';
  await page.goto('/tools/env-var-reference-extractor/' + qs);

  await expect(page.locator('#in-output')).toHaveValue('env-template', { timeout: 15_000 });
  await expect(page.locator('#in-include_defined_in_source')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('DATABASE_URL=postgres://localhost/dev', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`# 1 use on line 1
DATABASE_URL=postgres://localhost/dev

# 1 use on line 2
STRIPE_KEY=`);
});

test('env-var-reference-extractor page scans code accessors as JSON', async ({ page }) => {
  await page.goto('/tools/env-var-reference-extractor/');
  await page.fill('#in-text', 'const port = process.env.PORT || 3000;\nconst key = process.env["STRIPE_KEY"];\n');
  await page.selectOption('#in-syntax', 'code');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "PORT"', { timeout: 15_000 });
  await expect(out).toContainText('"name": "STRIPE_KEY"');
});
