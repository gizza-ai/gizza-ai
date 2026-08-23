import { test, expect } from './fixtures';

const LEFT = '<catalog><book id="1"><title>Rust</title></book></catalog>';
const RIGHT = '<catalog><book id="2"><title>Rust 2</title></book></catalog>';

// Multi-line / very large values are set directly instead of typed: page.fill()
// on a megabyte-sized textarea is needlessly slow, and the page recomputes off
// the `input` event either way.
async function setField(
  page: import('@playwright/test').Page,
  id: string,
  value: string,
) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('xml-diff reports attribute and text changes with XPath paths', async ({ page }) => {
  await page.goto('/tools/xml-diff/');
  await setField(page, '#in-left', LEFT);
  await setField(page, '#in-right', RIGHT);
  await page.selectOption('#in-format', 'text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 differences', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '2 differences: 0 added, 0 removed, 2 changed\n' +
      '~ /catalog/book/@id  1 -> 2\n' +
      '~ /catalog/book/title/text()  Rust -> Rust 2',
  );
});

test('xml-diff minifies the JSON report at indent 0 and reports an LCS insertion', async ({
  page,
}) => {
  await page.goto('/tools/xml-diff/');
  await setField(page, '#in-left', '<r><i>1</i><i>3</i></r>');
  await setField(page, '#in-right', '<r><i>1</i><i>2</i><i>3</i></r>');
  await page.locator('#in-indent').fill('0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"added":1', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '{"equal":false,"added":1,"removed":0,"changed":0,' +
      '"changes":[{"path":"/r/i[2]","kind":"added","new":"<i>2</i>"}]}',
  );
});

test('xml-diff deep-link pre-fills the unordered strategy and auto-runs', async ({ page }) => {
  const qs = new URLSearchParams({
    left: '<r><a>1</a><b>2</b></r>',
    right: '<r><b>2</b><a>1</a></r>',
    strategy: 'unordered',
    format: 'text',
  });
  await page.goto(`/tools/xml-diff/?${qs.toString()}`);

  await expect(page.locator('#in-left')).toHaveValue('<r><a>1</a><b>2</b></r>');
  await expect(page.locator('#in-right')).toHaveValue('<r><b>2</b><a>1</a></r>');
  await expect(page.locator('#in-strategy')).toHaveValue('unordered');
  await expect(page.locator('#in-format')).toHaveValue('text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('No differences', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'No differences: the two XML documents are equivalent.',
  );

  // The same pair under the default (lcs) strategy is NOT equal — sibling order
  // matters unless it is explicitly ignored.
  await page.selectOption('#in-strategy', 'lcs');
  await expect(out).toContainText('2 differences', { timeout: 15_000 });
});

test('xml-diff index strategy pairs siblings position by position', async ({ page }) => {
  await page.goto('/tools/xml-diff/');
  await setField(page, '#in-left', '<r><i>1</i><i>3</i></r>');
  await setField(page, '#in-right', '<r><i>1</i><i>2</i><i>3</i></r>');
  await page.selectOption('#in-strategy', 'index');
  await page.selectOption('#in-format', 'text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 differences', { timeout: 15_000 });
  // Positional pairing shifts everything after the insertion, unlike lcs.
  expect(await out.textContent()).toBe(
    '2 differences: 1 added, 0 removed, 1 changed\n' +
      '~ /r/i[2]/text()  3 -> 2\n' +
      '+ /r/i[3]  <i>3</i>',
  );
});

test('xml-diff compares comments once "ignore comments" is switched off', async ({ page }) => {
  await page.goto('/tools/xml-diff/');
  await setField(page, '#in-left', '<a><!-- one --><b/></a>');
  await setField(page, '#in-right', '<a><!-- two --><b/></a>');
  await page.selectOption('#in-format', 'text');

  const out = page.locator('#tool-output');
  // Default: comments are dropped before comparing.
  await expect(out).toContainText('No differences', { timeout: 15_000 });

  await page.uncheck('#in-ignore_comments');
  await expect(out).toContainText('1 difference', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '1 difference: 0 added, 0 removed, 1 changed\n~ /a/comment()/text()  one -> two',
  );
});

test('xml-diff reports whitespace once "ignore whitespace" is switched off', async ({ page }) => {
  await page.goto('/tools/xml-diff/');
  await setField(page, '#in-left', '<a><b>hi</b></a>');
  await setField(page, '#in-right', '<a><b>  hi  </b></a>');
  await page.selectOption('#in-format', 'text');
  await page.uncheck('#in-ignore_whitespace');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('1 difference', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '1 difference: 0 added, 0 removed, 1 changed\n~ /a/b/text()  hi ->   hi',
  );
});

test('xml-diff numeric comparison makes 1.0 equal 1 only when enabled', async ({ page }) => {
  await page.goto('/tools/xml-diff/');
  await setField(page, '#in-left', '<a v="1.0">2.50</a>');
  await setField(page, '#in-right', '<a v="1">2.5</a>');
  await page.selectOption('#in-format', 'text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 differences', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '2 differences: 0 added, 0 removed, 2 changed\n' +
      '~ /a/@v  1.0 -> 1\n' +
      '~ /a/text()  2.50 -> 2.5',
  );

  await page.check('#in-numeric_text');
  await expect(out).toHaveText('No differences: the two XML documents are equivalent.', {
    timeout: 15_000,
  });
});

test('xml-diff clamps the JSON indent to its maximum of 8 spaces', async ({ page }) => {
  await page.goto('/tools/xml-diff/');
  await setField(page, '#in-left', '<a v="1"/>');
  await setField(page, '#in-right', '<a v="2"/>');

  const out = page.locator('#tool-output');
  await page.locator('#in-indent').fill('8');
  await expect(out).toContainText('        "equal": false', { timeout: 15_000 });

  const at_max =
    '{\n' +
    '        "equal": false,\n' +
    '        "added": 0,\n' +
    '        "removed": 0,\n' +
    '        "changed": 1,\n' +
    '        "changes": [\n' +
    '                {\n' +
    '                        "path": "/a/@v",\n' +
    '                        "kind": "changed",\n' +
    '                        "old": "1",\n' +
    '                        "new": "2"\n' +
    '                }\n' +
    '        ]\n' +
    '}';
  expect(await out.textContent()).toBe(at_max);

  // One past the cap renders identically — indent is clamped, not rejected.
  await setField(page, '#in-indent', '9');
  await expect(out).toHaveText(at_max, { timeout: 15_000 });
});

test('xml-diff accepts a document of exactly 1 MB and rejects one byte more', async ({ page }) => {
  await page.goto('/tools/xml-diff/');
  await page.selectOption('#in-format', 'text');

  const at_cap = await page.evaluate(() => `<a>${'x'.repeat(1_000_000 - 7)}</a>`);
  expect(at_cap.length).toBe(1_000_000);

  await setField(page, '#in-left', at_cap);
  await setField(page, '#in-right', at_cap);
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('No differences: the two XML documents are equivalent.', {
    timeout: 15_000,
  });

  await setField(page, '#in-left', `${at_cap}y`);
  await expect(out).toHaveText(
    'the first (left) XML is too large (1000001 bytes); the limit is 1000000 bytes (1 MB) per document',
    { timeout: 15_000 },
  );
});
