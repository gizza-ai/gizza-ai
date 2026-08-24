import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  input: string,
  sort = 'key',
  dedupe = 'exact',
  encoding = 'normalize',
  space = 'percent',
  dropTracking = 'false',
  dropParams = '',
  keepParams = '',
  dropEmpty = 'false',
  output = 'urls',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/url-query-normalizer/gizza_ai_url_query_normalizer_web.js');
    await mod.default('/tools/url-query-normalizer/gizza_ai_url_query_normalizer_web_bg.wasm');
    return mod.run(
      args.input,
      args.sort,
      args.dedupe,
      args.encoding,
      args.space,
      args.dropTracking,
      args.dropParams,
      args.keepParams,
      args.dropEmpty,
      args.output,
    );
  }, { input, sort, dedupe, encoding, space, dropTracking, dropParams, keepParams, dropEmpty, output });
}

test('url-query-normalizer page renders exact canonical URLs', async ({ page }) => {
  await page.goto('/tools/url-query-normalizer/');
  await setTextarea(page, '#in-input', 'https://example.com/p?utm_source=news&b=hello+world&a=1&b=hello%20world\nb=2&a=1');

  await expect(page.locator('#tool-output')).toHaveText([
    'https://example.com/p?a=1&b=hello%20world&utm_source=news',
    'a=1&b=2',
  ].join('\n'), { timeout: 15_000 });
});

test('url-query-normalizer deep-link prefills controls and strips tracking', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'https://example.com/article?utm_source=newsletter&id=42&fbclid=abc',
    sort: 'key',
    dedupe: 'exact',
    encoding: 'normalize',
    space: 'percent',
    drop_tracking: 'true',
    drop_params: '',
    keep_params: '',
    drop_empty: 'false',
    output: 'urls',
  });

  await page.goto(`/tools/url-query-normalizer/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('https://example.com/article?utm_source=newsletter&id=42&fbclid=abc', { timeout: 15_000 });
  await expect(page.locator('#in-drop_tracking')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('urls');
  await expect(page.locator('#tool-output')).toHaveText('https://example.com/article?id=42', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool url-query-normalizer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('url-query-normalizer wasm covers enum values and filters', async ({ page }) => {
  await page.goto('/tools/url-query-normalizer/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, 'https://e.com?b=2&a=1', 'none')).resolves.toBe('https://e.com?b=2&a=1');
  await expect(runWasm(page, 'https://e.com?t=z&t=a&s=1', 'key-value')).resolves.toBe('https://e.com?s=1&t=a&t=z');

  await expect(runWasm(page, 'https://e.com?tag=a&tag=b&tag=a', 'key', 'none')).resolves.toBe('https://e.com?tag=a&tag=b&tag=a');
  await expect(runWasm(page, 'https://e.com?b=2&a=1&b=3', 'key', 'first')).resolves.toBe('https://e.com?a=1&b=2');
  await expect(runWasm(page, 'https://e.com?b=2&a=1&b=3', 'key', 'last')).resolves.toBe('https://e.com?a=1&b=3');

  await expect(runWasm(page, 'https://e.com?z=a%2db&a=x+y', 'key', 'exact', 'preserve')).resolves.toBe('https://e.com?a=x+y&z=a%2db');
  await expect(runWasm(page, 'https://e.com?q=hello%20world', 'key', 'exact', 'normalize', 'plus')).resolves.toBe('https://e.com?q=hello+world');

  await expect(runWasm(page, 'https://e.com?sid=1&x_a=2&x_b=3&keep=4', 'key', 'exact', 'normalize', 'percent', 'false', 'sid,x_*')).resolves.toBe('https://e.com?keep=4');
  await expect(runWasm(page, 'https://e.com?page=2&sort=asc&junk=1&session=abc', 'key', 'exact', 'normalize', 'percent', 'false', '', 'page,sort')).resolves.toBe('https://e.com?page=2&sort=asc');
});

test('url-query-normalizer controls cover non-default checkboxes, reports, and exact cap', async ({ page }) => {
  await page.goto('/tools/url-query-normalizer/');
  await setTextarea(page, '#in-input', 'https://e.com?b=&flag&a=1&utm_source=x');
  await page.check('#in-drop_tracking');
  await page.check('#in-drop_empty');
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toContainText('params_removed,3', { timeout: 15_000 });

  await expect(runWasm(page, 'https://e.com?b=1&a=2', 'key', 'exact', 'normalize', 'percent', 'false', '', '', 'false', 'changed'))
    .resolves.toBe('https://e.com?a=2&b=1');
  await expect(runWasm(page, 'https://e.com?b=1&a=2&a=2', 'key', 'exact', 'normalize', 'percent', 'false', '', '', 'false', 'report'))
    .resolves.toContain('line,original,normalized,params_in,params_out,changed');

  const atCap = 'a'.repeat(1_000_000);
  await expect(runWasm(page, atCap)).resolves.toBe(atCap);
  await expect(runWasm(page, `${atCap}x`)).rejects.toThrow(/limit is 1000000 bytes/);
});
