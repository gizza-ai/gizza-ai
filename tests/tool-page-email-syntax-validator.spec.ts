import { test, expect } from './fixtures';

test('email-syntax-validator page flags disposable role address', async ({
  page,
}) => {
  await page.goto('/tools/email-syntax-validator/');

  await page.fill('#in-email', 'admin@mailinator.com');
  await page.selectOption('#in-format', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText(
    'fail: admin@mailinator.com (syntax: valid, disposable: yes, role_based: yes, risk: high)',
    { timeout: 15000 },
  );
});

test('email-syntax-validator page supports deep links and disabled checks', async ({
  page,
}) => {
  await page.goto(
    '/tools/email-syntax-validator/?email=ada%40example.com&check_disposable=false&check_role=false&format=summary',
  );

  const out = page.locator('#tool-output');
  await expect(out).toContainText(
    'pass: ada@example.com (syntax: valid, disposable: no, role_based: no, risk: low)',
    { timeout: 15000 },
  );
  await expect(page.locator('#in-check_disposable')).not.toBeChecked();
  await expect(page.locator('#in-check_role')).not.toBeChecked();
});
