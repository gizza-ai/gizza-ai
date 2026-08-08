import { test, expect } from './fixtures';

const sdl = `type Query {
  user: User
}

type User {
  id: ID!
  fullName: String!
  email: String!
  role: Role!
}

enum Role { ADMIN EDITOR VIEWER }`;

const directiveSdl = `type Person {
  salutation: String! @examples(values: ["Mx", "Dr"])
  handle: String! @fake(type: firstName)
  nicknames: [String!]! @listLength(min: 4, max: 4)
}`;

const output = (page) => page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');
const parsed = async (page) => JSON.parse(await output(page));

test('graphql-mock-from-sdl page creates a query response with deterministic mock JSON', async ({ page }) => {
  await page.goto('/tools/graphql-mock-from-sdl/');
  await page.fill('#in-sdl', sdl);
  await page.selectOption('#in-mode', 'query-response');
  await page.fill('#in-list_length', '2');
  await page.fill('#in-depth', '3');
  await page.selectOption('#in-nullable_fields', 'fill');
  await page.fill('#in-seed', '1');

  await expect(page.locator('#tool-output')).toContainText('"data"', { timeout: 15000 });
  const json = await parsed(page);
  expect(json).toHaveProperty(['data', 'user', 'id']);
  expect(json.data.user.id).toMatch(/^[0-9a-f-]{36}$/);
  expect(json.data.user.email).toContain('@example.');
  expect(json.data.user.fullName).toContain(' ');
  expect(['ADMIN', 'EDITOR', 'VIEWER']).toContain(json.data.user.role);
});

test('graphql-mock-from-sdl deep-link applies single-type, typename, list length and directives', async ({ page }) => {
  const qs = new URLSearchParams({
    sdl: directiveSdl,
    mode: 'single-type',
    type_name: 'Person',
    list_length: '2',
    depth: '3',
    nullable_fields: 'fill',
    smart_values: 'true',
    typename: 'true',
    seed: '9',
    pretty: 'true',
  });
  await page.goto(`/tools/graphql-mock-from-sdl/?${qs.toString()}`);

  await expect(page.locator('#in-mode')).toHaveValue('single-type');
  await expect(page.locator('#in-type_name')).toHaveValue('Person');
  await expect(page.locator('#in-typename')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('__typename', { timeout: 15000 });
  const json = await parsed(page);
  expect(json.__typename).toBe('Person');
  expect(['Mx', 'Dr']).toContain(json.salutation);
  expect(json.nicknames).toHaveLength(4);
  expect(typeof json.handle).toBe('string');
});

test('graphql-mock-from-sdl page exposes nullable and compact-output options', async ({ page }) => {
  await page.goto('/tools/graphql-mock-from-sdl/');
  await page.fill('#in-sdl', 'type Post { id: ID! title: String! body: String views: Int }');
  await page.selectOption('#in-mode', 'single-type');
  await page.fill('#in-type_name', 'Post');
  await page.selectOption('#in-nullable_fields', 'null');
  await page.uncheck('#in-pretty');

  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"body":null', { timeout: 15000 });
  expect(await output(page)).not.toContain('\n');
  const json = await parsed(page);
  expect(json.id).toMatch(/^[0-9a-f-]{36}$/);
  expect(json.title).toBeTruthy();
  expect(json.body).toBeNull();
  expect(json.views).toBeNull();
});
