import { test, expect } from './fixtures';

const HAR = JSON.stringify({
  log: {
    entries: [
      {
        request: {
          method: 'GET',
          url: 'https://api.example.com/users/42?active=true&page=2',
          queryString: [
            { name: 'active', value: 'true' },
            { name: 'page', value: '2' },
          ],
        },
        response: {
          status: 200,
          content: {
            mimeType: 'application/json; charset=utf-8',
            text: '{"id":42,"name":"Ada","admin":false}',
          },
        },
      },
      {
        request: {
          method: 'POST',
          url: 'https://api.example.com/users',
          postData: {
            mimeType: 'application/json',
            text: '{"name":"Grace","age":30}',
          },
        },
        response: {
          status: 201,
          content: { mimeType: 'application/json', text: '{"id":43}' },
        },
      },
    ],
  },
});

test('har-to-openapi infers YAML paths and schemas from a HAR', async ({ page }) => {
  await page.goto('/tools/har-to-openapi/');
  await page.fill('#in-har', HAR);

  const out = page.locator('#tool-output');
  await expect(out).toContainText("openapi: '3.0.3'", { timeout: 15_000 });
  await expect(out).toContainText('/users/{user}:');
  await expect(out).toContainText('name: active');
  await expect(out).toContainText('type: boolean');
  await expect(out).toContainText('requestBody:');
  await expect(out).toContainText('/users:');
});

test('har-to-openapi deep-link emits JSON 3.1 without examples', async ({ page }) => {
  await page.goto('/tools/har-to-openapi/?format=json&openapi_version=3.1.0&include_examples=false&title=Captured%20API');
  await page.fill('#in-har', HAR);

  const text = await page.locator('#tool-output').textContent({ timeout: 15_000 });
  expect(text).toContain('"openapi": "3.1.0"');
  expect(text).toContain('"title": "Captured API"');
  expect(text).not.toContain('"example"');
});

test('har-to-openapi can keep literal paths', async ({ page }) => {
  await page.goto('/tools/har-to-openapi/?parameterize_paths=false');
  await page.fill('#in-har', HAR);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('/users/42:', { timeout: 15_000 });
  await expect(out).not.toContainText('/users/{user}:');
});

test('har-to-openapi reports invalid HAR JSON clearly', async ({ page }) => {
  await page.goto('/tools/har-to-openapi/');
  await page.fill('#in-har', 'not json');
  await expect(page.locator('#tool-output')).toContainText('invalid HAR', { timeout: 15_000 });
});
