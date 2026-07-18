import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const LOG = '2024-01-01 INFO started\n2024-01-01 ERROR disk full\n2024-01-02 ERROR disk full\n2024-01-02 WARN low memory\n2024-01-03 ERROR timeout';

test('text-pipeline-playground chains grep replace sort unique', async ({ page }) => {
  await page.goto('/tools/text-pipeline-playground/');
  await page.fill('#in-text', LOG);
  await page.fill('#in-pipeline', 'grep ERROR\nreplace /^\\S+ ERROR /!! /\nsort\nunique');
  await expect(page.locator('#tool-output')).toContainText('!! disk full', { timeout: 15000 });
  expect(await output(page)).toBe('!! disk full\n!! timeout');
});

test('text-pipeline-playground supports regex and case-insensitive filter', async ({ page }) => {
  await page.goto('/tools/text-pipeline-playground/');
  await page.fill('#in-text', 'TASK: refactor\nnote: ship it\ntask: write tests\nDone: deploy');
  await page.fill('#in-pipeline', 'grep ^task\ntrim\nupper');
  await page.check('#in-regex_mode');
  await page.check('#in-case_insensitive');
  await expect(page.locator('#tool-output')).toContainText('TASK: WRITE TESTS', { timeout: 15000 });
  expect(await output(page)).toBe('TASK: REFACTOR\nTASK: WRITE TESTS');
});

test('text-pipeline-playground honors limit and skip-on-error enum', async ({ page }) => {
  await page.goto('/tools/text-pipeline-playground/');
  await page.fill('#in-text', 'b\na\nc\nd');
  await page.fill('#in-pipeline', 'sort\nbogus op\nhead 3');
  await page.selectOption('#in-on_error', 'skip');
  await page.fill('#in-limit', '2');
  await expect(page.locator('#tool-output')).toHaveText('a\nb', { timeout: 15000 });
});

test('text-pipeline-playground deep-link pre-fills params and runs on load', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'apple\nBANANA\napricot\ncherry',
    pipeline: 'grep ^a\nsort -r',
    regex_mode: 'true',
    case_insensitive: 'true',
    limit: '10',
    on_error: 'stop',
  });
  await page.goto(`/tools/text-pipeline-playground/?${params.toString()}`);
  await expect(page.locator('#in-text')).toHaveValue('apple\nBANANA\napricot\ncherry', { timeout: 15000 });
  await expect(page.locator('#in-regex_mode')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('apricot', { timeout: 15000 });
  expect(await output(page)).toBe('apricot\napple');
});
