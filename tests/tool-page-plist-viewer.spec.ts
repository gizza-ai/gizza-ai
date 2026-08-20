import { test, expect } from './fixtures';

const XML = `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>Name</key><string>Example</string>
  <key>Count</key><integer>3</integer>
  <key>Enabled</key><true/>
</dict>
</plist>`;

test('plist-viewer converts XML plist to JSON', async ({ page }) => {
  await page.goto('/tools/plist-viewer/');
  await page.fill('#in-input', XML);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"Name": "Example"', { timeout: 15_000 });
  await expect(out).toContainText('"Count": 3');
  await expect(out).toContainText('"Enabled": true');
});

test('plist-viewer tree output and sorted keys', async ({ page }) => {
  await page.goto('/tools/plist-viewer/');
  await page.fill('#in-input', XML);
  await page.selectOption('#in-format', 'tree');
  await page.setChecked('#in-sort_keys', true);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"Count" => 3', { timeout: 15_000 });
  await expect(out).toContainText('"Name" => "Example"');
});
