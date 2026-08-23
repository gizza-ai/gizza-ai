import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('color-code-extractor deduplicates equivalent colors with counts', async ({ page }) => {
  await page.goto('/tools/color-code-extractor/');
  await setField(
    page,
    '#in-text',
    '.a{color:#f00}.b{color:#FF0000}.c{color:red}.d{color:rgb(255,0,0)}.e{background:hsl(210 50% 40%)}',
  );

  await expect(page.locator('#tool-output')).toHaveText('#ff0000  ×4\n#336699  ×1', {
    timeout: 15_000,
  });
});

test('color-code-extractor deep-link can disable named colors and uppercase hex', async ({
  page,
}) => {
  const qs = new URLSearchParams({
    text: 'The orange card uses #ff8800; plum is prose.',
    output_format: 'list',
    color_format: 'hex',
    sort: 'first_seen',
    include_counts: 'true',
    include_named: 'false',
    exclude_grey: 'false',
    exclude_monochrome: 'false',
    uppercase: 'true',
    limit: '0',
    var_prefix: 'color',
  });
  await page.goto(`/tools/color-code-extractor/?${qs.toString()}`);

  await expect(page.locator('#in-include_named')).not.toBeChecked();
  await expect(page.locator('#in-uppercase')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('#FF8800  ×1', { timeout: 15_000 });
});

test('color-code-extractor covers output format and color notation enums', async ({ page }) => {
  await page.goto('/tools/color-code-extractor/');
  await setField(page, '#in-text', '#336699 rgba(255,0,0,.5)');
  await page.selectOption('#in-output_format', 'csv');
  await page.selectOption('#in-color_format', 'rgb');
  await expect(page.locator('#tool-output')).toContainText(
    '"rgb(51, 102, 153)",#336699,"rgb(51, 102, 153)"',
    { timeout: 15_000 },
  );

  await page.selectOption('#in-output_format', 'css_vars');
  await setField(page, '#in-var_prefix', 'brand');
  await page.uncheck('#in-include_counts');
  await expect(page.locator('#tool-output')).toHaveText(
    ':root {\n  --brand-1: rgb(51, 102, 153);\n  --brand-2: rgba(255, 0, 0, 0.5);\n}',
    { timeout: 15_000 },
  );
});

test('color-code-extractor sorts by frequency and applies limit boundary', async ({ page }) => {
  await page.goto('/tools/color-code-extractor/');
  await setField(page, '#in-text', '.a{color:#f00}.b{color:#f00}.c{color:#0f0}.d{color:#00f}');
  await page.selectOption('#in-sort', 'frequency');
  await setField(page, '#in-limit', '2');
  await expect(page.locator('#tool-output')).toHaveText('#ff0000  ×2\n#00ff00  ×1', {
    timeout: 15_000,
  });

  await setField(page, '#in-limit', '1001');
  await expect(page.locator('#tool-output')).toHaveText(
    'limit must be between 0 (no limit) and 1000, got 1001',
    { timeout: 15_000 },
  );
});

test('color-code-extractor filters neutrals with non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/color-code-extractor/');
  await setField(page, '#in-text', '#000 #fff #888 #ff0000');
  await page.check('#in-exclude_grey');
  await expect(page.locator('#tool-output')).toHaveText('#000000  ×1\n#ffffff  ×1\n#ff0000  ×1', {
    timeout: 15_000,
  });

  await page.check('#in-exclude_monochrome');
  await expect(page.locator('#tool-output')).toHaveText('#ff0000  ×1', { timeout: 15_000 });
});
