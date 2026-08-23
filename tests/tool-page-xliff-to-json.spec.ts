import { test, expect } from './fixtures';

const xliff12 = `<xliff version="1.2">
  <file original="app.ts" source-language="en" target-language="de">
    <body>
      <trans-unit id="home.title"><source>Welcome</source><target>Willkommen</target></trans-unit>
      <trans-unit id="home.empty"><source>Empty cart</source><target></target></trans-unit>
    </body>
  </file>
</xliff>`;

const xliff20 = `<xliff version="2.0" xmlns="urn:oasis:names:tc:xliff:document:2.0">
  <file id="f1">
    <unit id="cart.title" name="CartTitle">
      <segment><source>Your cart</source><target>Votre panier</target></segment>
    </unit>
  </file>
</xliff>`;

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  xliff: string,
  output = 'pairs',
  key = 'id',
  inlineTags = 'placeholder',
  includeEmptyTargets = 'true',
  fallbackToSource = 'false',
  nested = 'false',
  separator = '.',
  includeMetadata = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/xliff-to-json/gizza_ai_xliff_to_json_web.js');
    await mod.default('/tools/xliff-to-json/gizza_ai_xliff_to_json_web_bg.wasm');
    return mod.run(
      args.xliff,
      args.output,
      args.key,
      args.inlineTags,
      args.includeEmptyTargets,
      args.fallbackToSource,
      args.nested,
      args.separator,
      args.includeMetadata,
    );
  }, { xliff, output, key, inlineTags, includeEmptyTargets, fallbackToSource, nested, separator, includeMetadata });
}

test('xliff-to-json page extracts XLIFF 1.2 pairs', async ({ page }) => {
  await page.goto('/tools/xliff-to-json/');
  await setTextarea(page, '#in-xliff', xliff12);

  await expect(page.locator('#tool-output')).toContainText('"home.title"', { timeout: 15_000 });
  const parsed = JSON.parse((await page.locator('#tool-output').textContent())!);
  expect(parsed['home.title']).toEqual({ source: 'Welcome', target: 'Willkommen' });
  expect(parsed['home.empty']).toEqual({ source: 'Empty cart', target: '' });
});

test('xliff-to-json deep-link prefills and nests target output', async ({ page }) => {
  const params = new URLSearchParams({
    xliff: xliff12,
    output: 'target',
    key: 'id',
    inline_tags: 'placeholder',
    include_empty_targets: 'true',
    fallback_to_source: 'false',
    nested: 'true',
    separator: '.',
    include_metadata: 'false',
  });
  await page.goto(`/tools/xliff-to-json/?${params.toString()}`);

  await expect(page.locator('#in-xliff')).toHaveValue(xliff12, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('target');
  await expect(page.locator('#in-nested')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"home"', { timeout: 15_000 });
  const parsed = JSON.parse((await page.locator('#tool-output').textContent())!);
  expect(parsed.home.title).toBe('Willkommen');
});

test('xliff-to-json wasm covers enums and checkbox defaults', async ({ page }) => {
  await page.goto('/tools/xliff-to-json/');
  await page.waitForSelector('#in-xliff');

  const arrayOut = JSON.parse(await runWasm(page, xliff20, 'array', 'resname', 'placeholder', 'true', 'false', 'false', '.', 'true'));
  expect(arrayOut[0].id).toBe('cart.title');
  expect(arrayOut[0].resname).toBe('CartTitle');
  expect(arrayOut[0].target).toBe('Votre panier');

  const noEmpty = JSON.parse(await runWasm(page, xliff12, 'target', 'id', 'placeholder', 'false'));
  expect(noEmpty).toEqual({ 'home.title': 'Willkommen' });

  const fallback = JSON.parse(await runWasm(page, xliff12, 'target', 'id', 'placeholder', 'true', 'true'));
  expect(fallback['home.empty']).toBe('Empty cart');
});
