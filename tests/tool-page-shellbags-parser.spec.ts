import { test, expect } from './fixtures';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const HIVE_B64 = readFileSync(resolve(__dirname, 'fixtures/shellbags-parser.b64'), 'utf8').trim();

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('shellbags-parser page reconstructs the sample shellbag tree from Base64', async ({ page }) => {
  await page.goto('/tools/shellbags-parser/');
  await page.fill('#in-data', HIVE_B64);
  await page.selectOption('#in-input_encoding', 'base64');
  await page.selectOption('#in-mode', 'tree');
  await page.fill('#in-max_entries', '200');
  await page.fill('#in-max_depth', '32');
  await expect(page.locator('#tool-output')).toContainText('Shellbag root: Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('This PC  (Root folder)');
  await expect(page.locator('#tool-output')).toContainText('C:\\  (Volume)');
  await expect(page.locator('#tool-output')).toContainText('Secret Plans  (Directory)');
  expect(await output(page)).toContain('6 entr(ies) reconstructed');
});

test('shellbags-parser page supports deep links and non-default checkbox state', async ({ page }) => {
  await page.goto(
    '/tools/shellbags-parser/?data=' +
      encodeURIComponent(HIVE_B64) +
      '&input_encoding=base64&mode=list&bag_root=auto&max_entries=200&max_depth=32&resolve_guids=false',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('{20d04fe0-3aea-1069-a2d8-08002b30309d}', { timeout: 15000 });
  await expect(out).toContainText('C:\\Users\\alice\\Secret Plans');
  await expect(out).not.toContainText('This PC');
});

test('shellbags-parser page rejects truncated hive bytes exactly', async ({ page }) => {
  await page.goto('/tools/shellbags-parser/?data=72656766&input_encoding=hex&mode=tree');
  await expect(page.locator('#tool-output')).toContainText('input is only 4 byte(s)', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('4096-byte base block');
});
