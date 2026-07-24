import { test, expect } from './fixtures';

const IAM_URL = 'https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08';
const ACCESS_KEY = 'AKIDEXAMPLE';
const SECRET_KEY = 'wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY';
const DOC_SIGNATURE =
  '5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7';
const CONTENT_TYPE = 'content-type: application/x-www-form-urlencoded; charset=utf-8';

async function fillIamDocVector(page) {
  await page.fill('#in-url', IAM_URL);
  await page.fill('#in-region', 'us-east-1');
  await page.fill('#in-service', 'iam');
  await page.fill('#in-access_key', ACCESS_KEY);
  await page.fill('#in-secret_key', SECRET_KEY);
  await page.selectOption('#in-method', 'GET');
  await page.fill('#in-headers', CONTENT_TYPE);
  await page.fill('#in-amz_date', '20150830T123600Z');
}

test('aws-sigv4-signer page computes the IAM documentation signature', async ({
  page,
}) => {
  await page.goto('/tools/aws-sigv4-signer/');
  await fillIamDocVector(page);
  await page.selectOption('#in-output', 'signature');
  await expect(page.locator('#tool-output')).toHaveText(DOC_SIGNATURE, {
    timeout: 15000,
  });
});

test('aws-sigv4-signer page emits an Authorization header', async ({ page }) => {
  await page.goto('/tools/aws-sigv4-signer/');
  await fillIamDocVector(page);
  await page.selectOption('#in-output', 'authorization');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('AWS4-HMAC-SHA256', { timeout: 15000 });
  await expect(out).toContainText('Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request');
  await expect(out).toContainText('SignedHeaders=content-type;host;x-amz-date');
  await expect(out).toContainText(`Signature=${DOC_SIGNATURE}`);
});

test('aws-sigv4-signer deep-link prefills and computes signature', async ({
  page,
}) => {
  const params = new URLSearchParams({
    url: IAM_URL,
    region: 'us-east-1',
    service: 'iam',
    access_key: ACCESS_KEY,
    secret_key: SECRET_KEY,
    method: 'GET',
    headers: CONTENT_TYPE,
    amz_date: '20150830T123600Z',
    output: 'signature',
  });
  await page.goto(`/tools/aws-sigv4-signer/?${params.toString()}`);
  await expect(page.locator('#in-url')).toHaveValue(IAM_URL, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('signature');
  await expect(page.locator('#tool-output')).toHaveText(DOC_SIGNATURE, {
    timeout: 15000,
  });
});

test('aws-sigv4-signer S3 checkbox signs x-amz-content-sha256', async ({
  page,
}) => {
  await page.goto('/tools/aws-sigv4-signer/');
  await page.fill('#in-url', 'https://examplebucket.s3.amazonaws.com/test.txt');
  await page.fill('#in-region', 'us-east-1');
  await page.fill('#in-service', 's3');
  await page.fill('#in-access_key', ACCESS_KEY);
  await page.fill('#in-secret_key', SECRET_KEY);
  await page.fill('#in-amz_date', '20130524T000000Z');
  await page.fill('#in-headers', 'range: bytes=0-9');
  await page.check('#in-sign_content_sha256');
  await page.selectOption('#in-output', 'headers');
  await expect(page.locator('#tool-output')).toContainText(
    'x-amz-content-sha256: e3b0c44298fc1c149afbf4c8996fb924',
    { timeout: 15000 },
  );
});
