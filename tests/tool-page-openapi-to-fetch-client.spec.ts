import { test, expect } from './fixtures';

const petSpec = JSON.stringify({
  openapi: '3.1.0',
  servers: [{ url: 'https://api.example.com' }],
  paths: {
    '/pets/{petId}': {
      get: {
        operationId: 'getPet',
        tags: ['pets'],
        summary: 'Get a pet',
        parameters: [
          { name: 'petId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'include', in: 'query', schema: { type: 'string' } },
        ],
        responses: {
          '200': {
            description: 'OK',
            content: { 'application/json': { schema: { $ref: '#/components/schemas/Pet' } } },
          },
        },
      },
    },
  },
});

const sessionSpec = JSON.stringify({
  openapi: '3.1.0',
  paths: {
    '/sessions': {
      post: {
        operationId: 'createSession',
        requestBody: {
          required: true,
          content: { 'application/json': { schema: { $ref: '#/components/schemas/Login' } } },
        },
        responses: {
          '201': {
            description: 'Created',
            content: { 'application/json': { schema: { $ref: '#/components/schemas/Session' } } },
          },
        },
      },
    },
  },
});

test('openapi-to-fetch-client generates a typed fetch function from a spec', async ({ page }) => {
  await page.goto('/tools/openapi-to-fetch-client/');
  await page.fill('#in-spec', petSpec);
  await page.fill('#in-types_module', './api-types');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('import type { Pet } from "./api-types";', { timeout: 15000 });
  await expect(out).toContainText('export async function getPet');
  await expect(out).toContainText('petId: string;');
  await expect(out).toContainText('encodeURIComponent(String(params.petId))');
  await expect(out).toContainText('export class ApiError');
});

test('openapi-to-fetch-client supports deep-linked class/result output', async ({ page }) => {
  const params = new URLSearchParams({
    spec: sessionSpec,
    style: 'class',
    client_name: 'AuthClient',
    error_handling: 'result',
    types_module: './schema-types',
  });
  await page.goto(`/tools/openapi-to-fetch-client/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('export class AuthClient', { timeout: 15000 });
  await expect(out).toContainText('export type ApiResult<T>');
  await expect(out).toContainText('async createSession');
  await expect(out).toContainText('Promise<ApiResult<Session>>');
  await expect(out).toContainText('import type { Login, Session } from "./schema-types";');
});

test('openapi-to-fetch-client supports positional/path naming and jsdoc off', async ({ page }) => {
  await page.goto('/tools/openapi-to-fetch-client/');
  await page.fill('#in-spec', petSpec);
  await page.selectOption('#in-param_style', 'positional');
  await page.selectOption('#in-naming', 'path');
  await page.uncheck('#in-jsdoc');
  await page.fill('#in-types_module', '');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('export async function getPetsByPetId', { timeout: 15000 });
  await expect(out).toContainText('petId: string, params: GetPetsByPetIdRequest = {}');
  await expect(out).toContainText('export type Pet = unknown;');
  await expect(out).not.toContainText('/**');
});
