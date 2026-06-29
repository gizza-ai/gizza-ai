import { test, expect } from './fixtures';

// /tools/strip-ansi-codes/ removes ANSI escape / color codes from terminal text
// in-browser (pure wasm). text is a multiline <textarea>; scope is a <select>
// ("all" default | "color"). ESC (\x1b) bytes are injected straight into the
// textarea value — page.fill can sanitize control chars — then an input event is
// dispatched to trigger the recompute.
async function setText(page: any, value: string) {
  await page.$eval(
    '#in-text',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

const ESC = '\x1b';

test('strip-ansi-codes page (all mode) strips SGR + OSC/control from multiline text', async ({ page }) => {
  await page.goto('/tools/strip-ansi-codes/');
  // Clear screen + home (control), an OSC 0 window title (BEL-terminated), bold
  // green + red SGR colors, across two lines. "all" is the default scope.
  await setText(
    page,
    `${ESC}[2J${ESC}[H${ESC}[1;32m✓ build passed${ESC}[0m\n${ESC}]0;my title${ESC}\x07${ESC}[31mERROR${ESC}[0m: not found`,
  );
  const out = page.locator('#tool-output');
  // Every escape sequence is gone — only the clean, Unicode-preserving text remains.
  await expect(out).toHaveText('✓ build passed\nERROR: not found', { timeout: 15000 });
});

test('strip-ansi-codes page (color mode) keeps cursor/control sequences but strips color', async ({ page }) => {
  await page.goto('/tools/strip-ansi-codes/');
  await page.selectOption('#in-scope', 'color');
  // \x1b[2J (erase) and \x1b[H (cursor home) must survive; the SGR colors go.
  await setText(page, `${ESC}[2J${ESC}[31mred${ESC}[0m${ESC}[H`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('red', { timeout: 15000 });
  // Non-SGR cursor/erase control is preserved verbatim.
  await expect(out).toContainText(`${ESC}[2J`);
  await expect(out).toContainText(`${ESC}[H`);
  // The color SGR codes are stripped.
  await expect(out).not.toContainText('[31m');
  await expect(out).not.toContainText('[0m');
});

test('strip-ansi-codes query-param deep-link pre-fills and computes', async ({ page }) => {
  const input = `${ESC}[2J${ESC}[33mwarn${ESC}[0m${ESC}[K`;
  await page.goto('/tools/strip-ansi-codes/?text=' + encodeURIComponent(input) + '&scope=color');
  // Deep link pre-fills both fields…
  await expect(page.locator('#in-text')).toHaveValue(input, { timeout: 15000 });
  await expect(page.locator('#in-scope')).toHaveValue('color');
  // …and computes: color mode drops the SGR color, keeps erase-line/erase-screen.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('warn', { timeout: 15000 });
  await expect(out).toContainText(`${ESC}[2J`);
  await expect(out).toContainText(`${ESC}[K`);
  await expect(out).not.toContainText('[33m');
});
