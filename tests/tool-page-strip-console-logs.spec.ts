import { test, expect } from './fixtures';

async function setBigTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('strip-console-logs removes default debug statements without touching errors', async ({ page }) => {
  await page.goto('/tools/strip-console-logs/');
  await setBigTextarea(
    page,
    '#in-code',
    'function checkout(cart) {\n  console.log("cart", cart);\n  console.debug(`total ${cart.length}`);\n  console.error("payment failed");\n  return cart.length;\n}'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('function checkout(cart) {', { timeout: 15000 });
  await expect(out).toContainText('console.error("payment failed");');
  await expect(out).toContainText('return cart.length;');
  await expect(out).not.toContainText('console.log("cart", cart);');
  await expect(out).not.toContainText('console.debug');
});

test('strip-console-logs deep link keeps selected methods and reports debugger removal', async ({ page }) => {
  await page.goto('/tools/strip-console-logs/?methods=all&keep=error%2Cwarn&action=remove&remove_debugger=true&output=report');
  await expect(page.locator('#in-action')).toHaveValue('remove', { timeout: 15000 });
  await expect(page.locator('#in-remove_debugger')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#in-methods')).toHaveValue('all');
  await expect(page.locator('#in-keep')).toHaveValue('error,warn');
  await setBigTextarea(
    page,
    '#in-code',
    'console.log("boot");\nconsole.error("request failed");\ndebugger;\nconst id = console.log("used as a value");\nconsole.warn("careful");'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Removed: 2', { timeout: 15000 });
  await expect(out).toContainText('debugger: 1');
  await expect(out).toContainText('log: 1');
  await expect(out).toContainText('Kept in expression position: 1');
  await expect(out).toContainText('console.log("used as a value")');
  await expect(out).not.toContainText('request failed');
});

test('strip-console-logs comments out statements and leaves debugger when checkbox is off', async ({ page }) => {
  await page.goto('/tools/strip-console-logs/');
  await page.selectOption('#in-action', 'comment');
  await page.uncheck('#in-remove_debugger');
  await setBigTextarea(
    page,
    '#in-code',
    'function save(row) {\n  console.log("saving", row);\n  debugger;\n  return db.put(row);\n}'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('  // console.log("saving", row);', { timeout: 15000 });
  await expect(out).toContainText('debugger;');
  await expect(out).toContainText('return db.put(row);');
});
