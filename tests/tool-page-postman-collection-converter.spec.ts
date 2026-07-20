import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

// A Postman collection with a folder, bearer auth, and a JSON body.
const POSTMAN_POST = JSON.stringify({
  info: { name: 'Demo API' },
  item: [
    {
      name: 'Users',
      item: [
        {
          name: 'Create user',
          request: {
            method: 'POST',
            url: 'https://api.example.com/users',
            auth: { type: 'bearer', bearer: [{ key: 'token', value: 'abc123' }] },
            body: {
              mode: 'raw',
              raw: '{"name":"Ada"}',
              options: { raw: { language: 'json' } },
            },
          },
        },
      ],
    },
  ],
});

// A minimal Postman GET with one header.
const POSTMAN_GET = JSON.stringify({
  info: { name: 'Demo API' },
  item: [
    {
      name: 'Get user',
      request: {
        method: 'GET',
        header: [{ key: 'Accept', value: 'application/json' }],
        url: { raw: 'https://api.example.com/users/1' },
      },
    },
  ],
});

// An Insomnia export (format 4): environment + folder + bearer request.
const INSOMNIA = JSON.stringify({
  _type: 'export',
  __export_format: 4,
  resources: [
    { _id: 'env_1', _type: 'environment', data: { base_url: 'https://api.example.com' } },
    { _id: 'fld_1', _type: 'request_group', name: 'Admin', parentId: 'wrk_1' },
    {
      _id: 'req_1',
      _type: 'request',
      parentId: 'fld_1',
      name: 'List users',
      method: 'GET',
      url: '{{ _.base_url }}/users',
      headers: [{ name: 'Accept', value: 'application/json' }],
      authentication: { type: 'bearer', token: 't-9' },
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

test('postman → curl is exact (folder label, auth, JSON body, multi-line default)', async ({ page }) => {
  await page.goto('/tools/postman-collection-converter/');
  await page.fill('#in-collection', POSTMAN_POST);
  await expect(page.locator('#tool-output')).toContainText('# Users / Create user', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '# Users / Create user',
      "curl -X POST 'https://api.example.com/users' \\",
      "  -H 'Content-Type: application/json' \\",
      "  -H 'Authorization: Bearer abc123' \\",
      '  --data-raw \'{"name":"Ada"}\'',
    ].join('\n'),
  );
});

test('target=fetch emits a runnable fetch() snippet (enum choice)', async ({ page }) => {
  await page.goto('/tools/postman-collection-converter/');
  await page.fill('#in-collection', POSTMAN_POST);
  await page.selectOption('#in-target', 'fetch');
  await expect(page.locator('#tool-output')).toContainText('await fetch', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '// Users / Create user',
      "const response = await fetch('https://api.example.com/users', {",
      "  method: 'POST',",
      '  headers: {',
      "    'Content-Type': 'application/json',",
      "    'Authorization': 'Bearer abc123',",
      '  },',
      '  body: JSON.stringify({"name":"Ada"}),',
      '});',
      'console.log(await response.text());',
    ].join('\n'),
  );
});

test('insomnia export → axios (secondary input format + enum choice + env vars)', async ({ page }) => {
  await page.goto('/tools/postman-collection-converter/');
  await page.fill('#in-collection', INSOMNIA);
  await page.selectOption('#in-target', 'axios');
  await expect(page.locator('#tool-output')).toContainText('await axios', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '// Admin / List users',
      'const response = await axios({',
      "  method: 'get',",
      "  url: 'https://api.example.com/users',",
      '  headers: {',
      "    'Accept': 'application/json',",
      "    'Authorization': 'Bearer t-9',",
      '  },',
      '});',
      'console.log(response.data);',
    ].join('\n'),
  );
});

test('unchecking multi-line yields single-line curl (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/postman-collection-converter/');
  await page.fill('#in-collection', POSTMAN_GET);
  await page.uncheck('#in-multiline');
  await expect(page.locator('#tool-output')).toContainText('# Get user', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    "# Get user\ncurl 'https://api.example.com/users/1' -H 'Accept: application/json'",
  );
});

test('variables field fills {{placeholders}} (KEY=VALUE form)', async ({ page }) => {
  await page.goto('/tools/postman-collection-converter/');
  await page.fill(
    '#in-collection',
    JSON.stringify({
      info: { name: 'Demo' },
      item: [{ name: 'Ping', request: { method: 'GET', url: '{{baseUrl}}/ping?x={{missing}}' } }],
    }),
  );
  await page.fill('#in-variables', 'baseUrl=https://api.example.com');
  await expect(page.locator('#tool-output')).toContainText('# Ping', { timeout: 15000 });
  // Known variable resolved; unknown left verbatim.
  expect(await outputText(page)).toBe("# Ping\ncurl 'https://api.example.com/ping?x={{missing}}'");
});

test('request cap: 200 converts, 201 is rejected with the exact count', async ({ page }) => {
  await page.goto('/tools/postman-collection-converter/');
  await page.fill('#in-collection', bigCollection(200));
  await expect(page.locator('#tool-output')).toContainText('# r199', { timeout: 30000 });
  await page.fill('#in-collection', bigCollection(201));
  await expect(page.locator('#tool-output')).toContainText(
    'collection contains 201 requests; the limit is 200',
    { timeout: 30000 },
  );
});

test('deep-link pre-fills and auto-runs (?collection=&target=fetch)', async ({ page }) => {
  const url = `/tools/postman-collection-converter/?collection=${encodeURIComponent(POSTMAN_GET)}&target=fetch`;
  await page.goto(url);
  await expect(page.locator('#in-collection')).toHaveValue(POSTMAN_GET, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('await fetch', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '// Get user',
      "const response = await fetch('https://api.example.com/users/1', {",
      "  method: 'GET',",
      '  headers: {',
      "    'Accept': 'application/json',",
      '  },',
      '});',
      'console.log(await response.text());',
    ].join('\n'),
  );
});

test('invalid JSON shows a clear error', async ({ page }) => {
  await page.goto('/tools/postman-collection-converter/');
  await page.fill('#in-collection', 'not json');
  await expect(page.locator('#tool-output')).toContainText('collection is not valid JSON', {
    timeout: 15000,
  });
});
