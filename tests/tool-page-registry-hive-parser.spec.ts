import { test, expect } from './fixtures';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const HIVE_HEX = readFileSync(resolve(__dirname, 'fixtures/registry-hive-parser.hex'), 'utf8').trim();

test('registry-hive-parser page browses a Run key from hex input', async ({ page }) => {
  await page.goto('/tools/registry-hive-parser/');
  await page.fill('#in-data', HIVE_HEX);
  await page.selectOption('#in-mode', 'path');
  await page.fill('#in-path', 'Software\\Microsoft\\Windows\\CurrentVersion\\Run');
  await page.fill('#in-max_entries', '10');
  await expect(page.locator('#tool-output')).toContainText('Key: Software\\Microsoft\\Windows\\CurrentVersion\\Run', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('OneDrive.exe /background');
});

test('registry-hive-parser page supports runkeys deep-link', async ({ page }) => {
  await page.goto(`/tools/registry-hive-parser/?data=${encodeURIComponent(HIVE_HEX)}&mode=runkeys&max_entries=10`);
  await expect(page.locator('#tool-output')).toContainText('autostart sweep', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('OneDrive');
});

test('registry-hive-parser page rejects non-hive bytes exactly', async ({ page }) => {
  await page.goto('/tools/registry-hive-parser/?data=504b0304140000000800&mode=summary');
  await expect(page.locator('#tool-output')).toContainText('not a registry hive', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('must start with the ASCII signature "regf"');
});
