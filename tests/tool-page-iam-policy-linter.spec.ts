import { test, expect } from './fixtures';

const ADMIN = JSON.stringify({
  Version: '2012-10-17',
  Statement: [{ Effect: 'Allow', Action: '*', Resource: '*' }],
});

const PUBLIC_BUCKET = JSON.stringify({
  Version: '2012-10-17',
  Statement: [
    {
      Effect: 'Allow',
      Principal: '*',
      Action: 's3:GetObject',
      Resource: 'arn:aws:s3:::public-bucket/*',
    },
  ],
});

const TRUST = JSON.stringify({
  Version: '2012-10-17',
  Statement: [
    {
      Effect: 'Allow',
      Principal: { Service: 'ec2.amazonaws.com' },
      Action: 'sts:AssumeRole',
    },
  ],
});

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

async function setPolicy(page, value: string) {
  await page.locator('#in-policy').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function fillBase(page) {
  await setPolicy(page, ADMIN);
  await page.selectOption('#in-policy_type', 'identity');
  await page.selectOption('#in-format', 'text');
  await page.selectOption('#in-min_severity', 'low');
  await page.fill('#in-ignore', '');
}

test('iam-policy-linter renders an exact admin wildcard report', async ({ page }) => {
  await page.goto('/tools/iam-policy-linter/');
  await fillBase(page);

  await expect(page.locator('#tool-output')).toContainText('ADMIN-STAR', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'UNSAFE — 1 finding (1 high, 0 medium, 0 low)',
      'identity policy · 1 statement · 85 characters (managed-policy limit 6144)',
      '',
      '[high] ADMIN-STAR — $.Statement[0].Action (line 1)',
      '  Allow with Action "*" on an unconstrained Resource grants full administrator access — every action, on every resource, in the account.',
    ].join('\n'),
  );
});

test('iam-policy-linter covers output format and policy type choices', async ({ page }) => {
  await page.goto('/tools/iam-policy-linter/');
  await fillBase(page);

  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('"verdict": "unsafe"', { timeout: 15000 });
  const json = JSON.parse(await outputText(page));
  expect(json.findings[0].code).toBe('ADMIN-STAR');
  expect(json.summary.total).toBe(1);

  await page.selectOption('#in-format', 'csv');
  await expect(page.locator('#tool-output')).toContainText('severity,code,path,line,message', {
    timeout: 15000,
  });
  expect(await outputText(page)).toContain('high,ADMIN-STAR,$.Statement[0].Action,1');

  await setPolicy(page, PUBLIC_BUCKET);
  await page.selectOption('#in-policy_type', 'resource');
  await page.selectOption('#in-format', 'text');
  await expect(page.locator('#tool-output')).toContainText('PRINCIPAL-STAR', { timeout: 15000 });

  await setPolicy(page, TRUST);
  await page.selectOption('#in-policy_type', 'trust');
  await expect(page.locator('#tool-output')).toContainText('CLEAN — no findings', { timeout: 15000 });
});

test('iam-policy-linter supports severity filtering, ignore suppression and deep links', async ({
  page,
}) => {
  await page.goto('/tools/iam-policy-linter/');
  await fillBase(page);

  await page.selectOption('#in-min_severity', 'high');
  await expect(page.locator('#tool-output')).toContainText('ADMIN-STAR', { timeout: 15000 });

  await page.fill('#in-ignore', 'ADMIN-STAR');
  await expect(page.locator('#tool-output')).toContainText('CLEAN — no findings', { timeout: 15000 });

  const policy = encodeURIComponent(PUBLIC_BUCKET);
  await page.goto(
    `/tools/iam-policy-linter/?policy=${policy}&policy_type=resource&format=json&min_severity=low&ignore=`,
  );
  await expect(page.locator('#in-policy_type')).toHaveValue('resource');
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('PRINCIPAL-STAR', { timeout: 15000 });
  const linked = JSON.parse(await outputText(page));
  expect(linked.policy_type).toBe('resource');
  expect(linked.findings.some((f) => f.code === 'PRINCIPAL-STAR')).toBe(true);
});
