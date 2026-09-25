import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('TypeScript output infers nested types and optional nulls', async ({ page }) => {
  await page.goto('/tools/json-to-types/');
  await setTextarea(page, '#in-json', '{"id":1,"name":"Ada","email":null,"tags":["admin"],"profile":{"active":true}}');
  await page.fill('#in-root_name', 'User');

  await expect(page.locator('#tool-output')).toContainText('export interface User', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('export interface Profile');
  expect(text).toContain('email?: unknown;');
  expect(text).toContain('tags: string[];');
});

test('deep-link pre-fills Rust output with serde rename', async ({ page }) => {
  const json = '{"user_id":1,"displayName":"Ada","roles":[{"name":"admin","level":2}]}';
  await page.goto(
    '/tools/json-to-types/?json=' +
      encodeURIComponent(json) +
      '&output_language=rust&root_name=User&optional_strategy=optional&json_annotations=true&export=true',
  );

  await expect(page.locator('#in-json')).toHaveValue(json, { timeout: 15000 });
  await expect(page.locator('#in-output_language')).toHaveValue('rust');
  const text = await outText(page);
  expect(text).toContain('pub struct Role');
  expect(text).toContain('#[serde(rename = "displayName")]');
  expect(text).toContain('pub display_name: String');
});

test('Go output exercises array merging and optional fields', async ({ page }) => {
  await page.goto('/tools/json-to-types/');
  await setTextarea(page, '#in-json', '[{"id":1,"name":"Ada"},{"id":2,"name":"Grace","email":"g@example.com"}]');
  await page.selectOption('#in-output_language', 'go');
  await page.fill('#in-root_name', 'User');

  await expect(page.locator('#tool-output')).toContainText('type UserItem struct', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('Email *string `json:"email,omitempty"`');
  expect(text).toContain('type User = []UserItem');
});

test('Python output handles reserved keys and checkbox off state', async ({ page }) => {
  await page.goto('/tools/json-to-types/');
  await setTextarea(page, '#in-json', '{"id":1,"class":"gold","settings":{"newsletter":false}}');
  await page.selectOption('#in-output_language', 'python');
  await page.fill('#in-root_name', 'Account');
  await page.uncheck('#in-export');

  await expect(page.locator('#tool-output')).toContainText('class Account:', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('class_: str  # JSON key: "class"');
  expect(text).toContain('settings: Settings');
});

test('advertised language enum, strategy enum and error surface render', async ({ page }) => {
  await page.goto('/tools/json-to-types/');
  await setTextarea(page, '#in-json', '{"value":null}');
  await page.selectOption('#in-optional_strategy', 'nullable');
  await expect(page.locator('#tool-output')).toContainText('value: null;', { timeout: 15000 });

  await setTextarea(page, '#in-json', '{not json');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});
