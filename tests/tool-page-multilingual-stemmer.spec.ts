import { test, expect } from './fixtures';

const english = 'The runners were running quickly. Studies studied studying.';

async function fillStemmer(page, opts: {
  input?: string;
  language?: string;
  output?: string;
  minLength?: string;
  lowercase?: boolean;
}) {
  if (opts.input !== undefined) await page.fill('#in-input', opts.input);
  if (opts.language !== undefined) await page.selectOption('#in-language', opts.language);
  if (opts.output !== undefined) await page.selectOption('#in-output', opts.output);
  if (opts.minLength !== undefined) await page.fill('#in-min_length', opts.minLength);
  if (opts.lowercase !== undefined) opts.lowercase ? await page.check('#in-lowercase') : await page.uncheck('#in-lowercase');
}

test('multilingual-stemmer stems English text while preserving layout', async ({ page }) => {
  await page.goto('/tools/multilingual-stemmer/');
  await fillStemmer(page, { input: english, language: 'english', output: 'text' });

  await expect(page.locator('#tool-output')).toHaveText('the runner were run quick. studi studi studi.', { timeout: 20000 });
});

test('multilingual-stemmer deep link applies language, output, slider and checkbox', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('Häuser Häusern Haus') +
    '&language=german' +
    '&output=mapping' +
    '&min_length=1' +
    '&lowercase=true';

  await page.goto('/tools/multilingual-stemmer/' + qs);
  await expect(page.locator('#in-language')).toHaveValue('german', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('mapping');
  await expect(page.locator('#in-lowercase')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('häuser -> haus\nhäusern -> haus\nhaus -> haus', { timeout: 20000 });
});

test('multilingual-stemmer supports table and JSON outputs', async ({ page }) => {
  await page.goto('/tools/multilingual-stemmer/');
  await fillStemmer(page, { input: 'cats cat dogs', language: 'english', output: 'table' });
  await expect(page.locator('#tool-output')).toContainText('STEM\tCOUNT\tFORMS', { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('cat\t2\tcats, cat');

  await page.selectOption('#in-output', 'json');
  const raw = await page.locator('#tool-output').textContent({ timeout: 20000 });
  const data = JSON.parse(raw ?? '');
  expect(data.language).toBe('english');
  expect(data.unique_stems).toBe(2);
  expect(data.stems[0].stem).toBe('cat');
});

test('multilingual-stemmer honors non-default min length and lowercase off', async ({ page }) => {
  await page.goto('/tools/multilingual-stemmer/');
  await fillStemmer(page, {
    input: 'Running running APIs',
    language: 'english',
    output: 'text',
    minLength: '6',
    lowercase: false,
  });

  await expect(page.locator('#in-lowercase')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('Run run APIs', { timeout: 20000 });
});

test('multilingual-stemmer reports invalid input clearly', async ({ page }) => {
  await page.goto('/tools/multilingual-stemmer/');
  await fillStemmer(page, { input: 'running', minLength: '99' });

  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('min_length must be between 1 and 30');
});
