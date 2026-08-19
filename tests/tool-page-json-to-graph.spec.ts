import { test, expect } from './fixtures';

async function setValue(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('json-to-graph renders mermaid source exactly', async ({ page }) => {
  await page.goto('/tools/json-to-graph/');
  await setValue(page, '#in-json', '{"a":[1]}');

  await expect(page.locator('#tool-output')).toHaveText(
    'flowchart TD\n    n0["root"]\n    n1[["a"]]\n    n2("[0]: 1")\n    n0 --> n1\n    n1 --> n2',
    { timeout: 15_000 },
  );
});

test('json-to-graph deep-links DOT with caps and non-default checkboxes', async ({ page }) => {
  const json = '{"orders":[{"id":101,"items":[{"sku":"A1","qty":2},{"sku":"B2","qty":1}]}],"next":null}';
  const qs = new URLSearchParams({
    json,
    format: 'dot',
    direction: 'LR',
    max_depth: '3',
    max_nodes: '20',
    max_array_items: '1',
    include_values: 'false',
    value_max_len: '20',
    show_types: 'true',
  });
  await page.goto(`/tools/json-to-graph/?${qs.toString()}`);

  await expect(page.locator('#in-json')).toHaveValue(json, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('dot');
  await expect(page.locator('#in-direction')).toHaveValue('LR');
  await expect(page.locator('#in-max_array_items')).toHaveValue('1');
  await expect(page.locator('#in-include_values')).not.toBeChecked();
  await expect(page.locator('#in-show_types')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('digraph json {', { timeout: 15_000 });
  await expect(out).toContainText('rankdir="LR";');
  await expect(out).toContainText('label="orders [1]", shape=box3d');
  await expect(out).toContainText('… 2 items hidden');
  await expect(out).toContainText('label="next: null", shape=ellipse');
});

test('json-to-graph covers directions, depth cap and errors', async ({ page }) => {
  await page.goto('/tools/json-to-graph/');
  await setValue(page, '#in-json', '{"a":{"b":{"c":1}}}');
  await page.selectOption('#in-format', 'mermaid');
  await page.selectOption('#in-direction', 'RL');
  await setValue(page, '#in-max_depth', '1');
  await setValue(page, '#in-max_nodes', '1');
  await page.locator('#in-include_values').check();
  await page.locator('#in-show_types').uncheck();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('flowchart RL', { timeout: 15_000 });
  await expect(out).toContainText('truncated at the 1-node limit');

  await setValue(page, '#in-json', '{oops');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15_000 });
});
