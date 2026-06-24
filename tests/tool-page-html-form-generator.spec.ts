import { test, expect } from './fixtures';

test('html-form-generator page builds accessible markup', async ({ page }) => {
  await page.goto('/tools/html-form-generator/');

  // A small form: required text + email, a select, and an opt-in checkbox.
  await page.fill(
    '#in-fields',
    'Full Name*\nEmail* | email\nTopic | select | Sales, Support\nSubscribe | checkbox',
  );
  await page.fill('#in-submit_label', 'Send');
  // Turn off the <style> block so the output is bare, predictable markup.
  await page.uncheck('#in-styled');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<form method="post">', { timeout: 15000 });
  // Bound label + slugified name/id + required attribute.
  await expect(out).toContainText('<label for="full-name">Full Name');
  await expect(out).toContainText('<input type="email" id="email" name="email" required>');
  // Select renders its options.
  await expect(out).toContainText('<option value="Sales">Sales</option>');
  // Checkbox uses value="yes" with its label after the box.
  await expect(out).toContainText('<input type="checkbox" id="subscribe" name="subscribe" value="yes">');
  // Custom submit label.
  await expect(out).toContainText('<button type="submit">Send</button>');
});

test('html-form-generator page method switch', async ({ page }) => {
  await page.goto('/tools/html-form-generator/');

  await page.fill('#in-fields', 'Name');
  await page.selectOption('#in-method', 'get');
  await expect(page.locator('#tool-output')).toContainText('method="get"', { timeout: 15000 });
});
