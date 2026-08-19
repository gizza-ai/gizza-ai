import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('chunk-list page splits newline items into labelled plain batches', async ({ page }) => {
  await page.goto('/tools/chunk-list/');
  await page.fill('#in-items', 'alpha\nbeta\ngamma\ndelta\nepsilon');
  await page.fill('#in-chunk_size', '2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Chunk 1 (2 items)', { timeout: 15_000 });
  expect(await output(page)).toBe('Chunk 1 (2 items)\nalpha, beta\n\nChunk 2 (2 items)\ngamma, delta\n\nChunk 3 (1 item)\nepsilon');
});

test('chunk-list deep link pre-fills comma input, JSON output, and unlabelled checkbox', async ({ page }) => {
  const qs = new URLSearchParams({
    items: '1001,1002,1003,1004,1005',
    input_separator: 'comma',
    chunk_size: '3',
    output: 'json',
    label_chunks: 'false',
  });
  await page.goto(`/tools/chunk-list/?${qs.toString()}`);

  await expect(page.locator('#in-input_separator')).toHaveValue('comma', { timeout: 15_000 });
  await expect(page.locator('#in-chunk_size')).toHaveValue('3');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-label_chunks')).not.toBeChecked();
  expect(JSON.parse(await output(page))).toEqual([
    ['1001', '1002', '1003'],
    ['1004', '1005'],
  ]);
});

test('chunk-list page supports custom separator and CSV output', async ({ page }) => {
  await page.goto('/tools/chunk-list/');
  await page.fill('#in-items', 'red :: blue, green :: yellow');
  await page.selectOption('#in-input_separator', 'custom');
  await page.fill('#in-custom_separator', '::');
  await page.fill('#in-chunk_size', '1');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('"blue, green"', { timeout: 15_000 });
  expect(await output(page)).toBe('1,red\n2,"blue, green"\n3,yellow');
});

test('chunk-list page supports markdown output and pipe separator', async ({ page }) => {
  await page.goto('/tools/chunk-list/');
  await page.fill('#in-items', 'task A|task B|task C');
  await page.selectOption('#in-input_separator', 'pipe');
  await page.fill('#in-chunk_size', '2');
  await page.selectOption('#in-output', 'markdown');

  const text = await output(page);
  expect(text).toContain('### Chunk 1 (2 items)');
  expect(text).toContain('- task A\n- task B');
  expect(text).toContain('### Chunk 2 (1 item)');
});

test('chunk-list page reports invalid chunk sizes', async ({ page }) => {
  await page.goto('/tools/chunk-list/');
  await page.fill('#in-items', 'a,b');
  await page.fill('#in-chunk_size', '0');

  await expect(page.locator('#tool-output')).toContainText('chunk_size must be at least 1', { timeout: 15_000 });
});
