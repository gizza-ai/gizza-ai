import { test, expect } from './fixtures';

test('krutidev-unicode-converter bidirectional conversion', async ({ page }) => {
  await page.goto('/tools/krutidev-unicode-converter/');

  // 1. Convert Kruti Dev to Unicode
  const leftTextarea = page.locator('#kd-input-area');
  const rightTextarea = page.locator('#kd-output-area');

  await leftTextarea.fill('esjk uke usgy gSA');
  await expect(rightTextarea).toHaveValue('मेरा नाम नेहल है।', { timeout: 15000 });

  // 2. Clear inputs
  const clearBtn = page.locator('.kd-btn-clear').first();
  await clearBtn.click();
  await expect(leftTextarea).toHaveValue('');
  await expect(rightTextarea).toHaveValue('');

  // 3. Convert Unicode to Kruti Dev (with chhoti-i reordering)
  await rightTextarea.fill('स्थिर');
  await expect(leftTextarea).toHaveValue('fLFkj', { timeout: 15000 });

  // 4. Test reph reordering
  await clearBtn.click();
  await rightTextarea.fill('धर्म');
  await expect(leftTextarea).toHaveValue('/keZ', { timeout: 15000 });
});
