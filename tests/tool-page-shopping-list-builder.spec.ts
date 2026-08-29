import { test, expect } from './fixtures';

const TWO_RECIPE_TEXT =
  'Dairy\n' +
  '- 4 eggs\n' +
  '- 2.5 cup milk\n' +
  '\n' +
  'Pantry\n' +
  '- 2 cup flour\n' +
  '- 1 tbsp sugar';

const METRIC_TEXT =
  'Produce\n' +
  '- 2 clove garlic — Chili\n' +
  '\n' +
  'Meat & seafood\n' +
  '- 680.39 g beef — Chili';

test('shopping-list-builder page merges two recipes exactly', async ({ page }) => {
  await page.goto('/tools/shopping-list-builder/');
  await page.fill(
    '#in-ingredients',
    '# Pancakes x2\n1 cup flour\n1 cup milk\n2 eggs\n---\n# Sauce\n1/2 cup milk\n1 tbsp sugar'
  );
  await page.selectOption('#in-format', 'text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('2.5 cup milk', { timeout: 15000 });
  expect(await out.textContent()).toBe(TWO_RECIPE_TEXT);
});

test('shopping-list-builder page supports metric units and source labels', async ({ page }) => {
  const params = new URLSearchParams({
    ingredients: '# Chili\n1 lb beef\n8 oz beef\n2 cloves garlic',
    unit_system: 'metric',
    show_sources: 'true',
    format: 'text',
  });
  await page.goto(`/tools/shopping-list-builder/?${params.toString()}`);

  await expect(page.locator('#in-unit_system')).toHaveValue('metric');
  await expect(page.locator('#in-show_sources')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('680.39 g beef — Chili', { timeout: 15000 });
  expect(await out.textContent()).toBe(METRIC_TEXT);
});

test('shopping-list-builder page honours deep-linked CSV pantry skip', async ({ page }) => {
  const params = new URLSearchParams({
    ingredients: '1 cup flour\n2 tbsp sugar\n1 tsp salt',
    group_by: 'none',
    exclude: 'salt',
    format: 'csv',
  });
  await page.goto(`/tools/shopping-list-builder/?${params.toString()}`);

  await expect(page.locator('#in-ingredients')).toHaveValue('1 cup flour\n2 tbsp sugar\n1 tsp salt');
  await expect(page.locator('#in-group_by')).toHaveValue('none');
  await expect(page.locator('#in-exclude')).toHaveValue('salt');
  await expect(page.locator('#in-format')).toHaveValue('csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Pantry,flour,1,cup', { timeout: 15000 });
  expect(await out.textContent()).toBe(
    'category,item,quantity,unit\nPantry,flour,1,cup\nPantry,sugar,2,tbsp'
  );
});
