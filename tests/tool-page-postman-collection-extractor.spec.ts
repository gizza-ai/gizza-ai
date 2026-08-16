import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

// A collection v2.1 export with: a root-level request, a nested folder,
// collection-level bearer auth (inherited by every request), a {{baseUrl}}
// variable, one disabled header, and a JSON body.
const COLLECTION = JSON.stringify({
  info: { name: 'Demo API' },
  variable: [{ key: 'baseUrl', value: 'https://api.example.com' }],
  auth: { type: 'bearer', bearer: [{ key: 'token', value: 'abc123' }] },
  item: [
    { name: 'List users', request: { method: 'GET', url: { raw: '{{baseUrl}}/users?page=1' } } },
    {
      name: 'Users',
      item: [
        {
          name: 'Create user',
          request: {
            method: 'POST',
            url: '{{baseUrl}}/users',
            header: [
              { key: 'Content-Type', value: 'application/json' },
              { key: 'X-Debug', value: '1', disabled: true },
            ],
            body: {
              mode: 'raw',
              raw: '{"name":"Ada"}',
              options: { raw: { language: 'json' } },
            },
          },
        },
        { name: 'Delete user', request: { method: 'DELETE', url: '{{baseUrl}}/users/1' } },
      ],
    },
  ],
});

function bigCollection(n: number): string {
  const items = [];
  for (let i = 0; i < n; i++) {
    items.push({ name: `r${i}`, request: { method: 'GET', url: `https://example.com/${i}` } });
  }
  return JSON.stringify({ info: { name: 'Big' }, item: items });
}

test('default list output is exact (folder, inherited auth, headers, body)', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', COLLECTION);
  await expect(page.locator('#tool-output')).toContainText('3 of 3 requests', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '3 of 3 requests · 1 folder',
      '',
      '1. List users',
      '   GET https://api.example.com/users?page=1',
      '   Auth: bearer',
      '   Headers: (none)',
      '   Body: (none)',
      '',
      '2. Create user',
      '   POST https://api.example.com/users',
      '   Folder: Users',
      '   Auth: bearer',
      '   Headers:',
      '     Content-Type: application/json',
      '   Body (json):',
      '     {"name":"Ada"}',
      '',
      '3. Delete user',
      '   DELETE https://api.example.com/users/1',
      '   Folder: Users',
      '   Auth: bearer',
      '   Headers: (none)',
      '   Body: (none)',
    ].join('\n'),
  );
});

test('format=table is aligned; the disabled header is not counted (enum choice)', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', COLLECTION);
  await page.selectOption('#in-format', 'table');
  await expect(page.locator('#tool-output')).toContainText('METHOD', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '3 of 3 requests · 1 folder',
      '',
      '#  METHOD  HDRS  BODY  NAME         URL',
      '1  GET     0     none  List users   https://api.example.com/users?page=1',
      '2  POST    1     json  Create user  https://api.example.com/users',
      '3  DELETE  0     none  Delete user  https://api.example.com/users/1',
    ].join('\n'),
  );
});

test('format=csv quotes the multi-field body and keeps every column', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', COLLECTION);
  await page.selectOption('#in-format', 'csv');
  await expect(page.locator('#tool-output')).toContainText('index,folder,name', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'index,folder,name,method,url,auth,headers,body_mode,body',
      '1,,List users,GET,https://api.example.com/users?page=1,bearer,,none,',
      '2,Users,Create user,POST,https://api.example.com/users,bearer,Content-Type: application/json,json,"{""name"":""Ada""}"',
      '3,Users,Delete user,DELETE,https://api.example.com/users/1,bearer,,none,',
    ].join('\n'),
  );
});

test('format=markdown renders a GFM table with an em dash for empty cells', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', COLLECTION);
  await page.selectOption('#in-format', 'markdown');
  await expect(page.locator('#tool-output')).toContainText('| # | Method |', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '| # | Method | Name | URL | Headers | Body |',
      '| --- | --- | --- | --- | --- | --- |',
      '| 1 | GET | List users | https://api.example.com/users?page=1 | — | — |',
      '| 2 | POST | Create user | https://api.example.com/users | Content-Type: application/json | {"name":"Ada"} |',
      '| 3 | DELETE | Delete user | https://api.example.com/users/1 | — | — |',
    ].join('\n'),
  );
});

test('method + folder filters narrow the listing (index keeps collection order)', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', COLLECTION);
  await page.selectOption('#in-format', 'table');
  await page.fill('#in-method', 'delete');
  await expect(page.locator('#tool-output')).toContainText('1 of 3 requests', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '1 of 3 requests · 1 folder',
      '',
      '#  METHOD  HDRS  BODY  NAME         URL',
      '3  DELETE  0     none  Delete user  https://api.example.com/users/1',
    ].join('\n'),
  );

  await page.fill('#in-method', '');
  await page.fill('#in-folder', 'users');
  await expect(page.locator('#tool-output')).toContainText('2 of 3 requests', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '2 of 3 requests · 1 folder',
      '',
      '#  METHOD  HDRS  BODY  NAME         URL',
      '2  POST    1     json  Create user  https://api.example.com/users',
      '3  DELETE  0     none  Delete user  https://api.example.com/users/1',
    ].join('\n'),
  );
});

test('format=urls dedupes; the variables field overrides the collection variable', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', COLLECTION);
  await page.selectOption('#in-format', 'urls');
  await expect(page.locator('#tool-output')).toContainText('api.example.com', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'https://api.example.com/users?page=1',
      'https://api.example.com/users',
      'https://api.example.com/users/1',
    ].join('\n'),
  );

  await page.fill('#in-variables', 'baseUrl=https://staging.example.com');
  await expect(page.locator('#tool-output')).toContainText('staging.example.com', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'https://staging.example.com/users?page=1',
      'https://staging.example.com/users',
      'https://staging.example.com/users/1',
    ].join('\n'),
  );
});

test('unchecking "Resolve {{variables}}" keeps placeholders verbatim (non-default checkbox)', async ({
  page,
}) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', COLLECTION);
  await page.selectOption('#in-format', 'urls');
  await expect(page.locator('#in-resolve_variables')).toBeChecked();
  await page.uncheck('#in-resolve_variables');
  await expect(page.locator('#tool-output')).toContainText('{{baseUrl}}', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    ['{{baseUrl}}/users?page=1', '{{baseUrl}}/users', '{{baseUrl}}/users/1'].join('\n'),
  );
});

test('request cap: 500 lists, 501 is rejected with the exact count', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.selectOption('#in-format', 'urls');
  await page.fill('#in-collection', bigCollection(500));
  await expect(page.locator('#tool-output')).toContainText('https://example.com/499', {
    timeout: 30000,
  });
  await page.fill('#in-collection', bigCollection(501));
  await expect(page.locator('#tool-output')).toContainText(
    'collection contains 501 requests; the limit is 500',
    { timeout: 30000 },
  );
});

test('deep-link pre-fills and auto-runs (?collection=&format=urls)', async ({ page }) => {
  const url = `/tools/postman-collection-extractor/?collection=${encodeURIComponent(
    COLLECTION,
  )}&format=urls`;
  await page.goto(url);
  await expect(page.locator('#in-collection')).toHaveValue(COLLECTION, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('api.example.com', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'https://api.example.com/users?page=1',
      'https://api.example.com/users',
      'https://api.example.com/users/1',
    ].join('\n'),
  );
});

test('errors are specific: not JSON, not a collection, and no filter matches', async ({ page }) => {
  await page.goto('/tools/postman-collection-extractor/');
  await page.fill('#in-collection', 'not json');
  await expect(page.locator('#tool-output')).toContainText('collection is not valid JSON', {
    timeout: 15000,
  });

  await page.fill('#in-collection', '{"info":{"name":"x"}}');
  await expect(page.locator('#tool-output')).toContainText(
    'expected a Postman Collection v2.0/v2.1 export',
    { timeout: 15000 },
  );

  await page.fill('#in-collection', COLLECTION);
  await page.fill('#in-url_contains', 'nothing-matches-this');
  await expect(page.locator('#tool-output')).toContainText('no requests match the filters', {
    timeout: 15000,
  });
});
