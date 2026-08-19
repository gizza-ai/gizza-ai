import { test, expect } from './fixtures';

const STACK = `services:
  web:
    image: nginx:1.27
    ports: ['8080:80']
    depends_on:
      api:
        condition: service_started
    networks: [front]
  api:
    build: ./api
    depends_on:
      db:
        condition: service_healthy
    networks: [front, back]
  db:
    image: postgres:16
    volumes: ['db-data:/var/lib/postgresql/data:ro']
    networks: [back]
networks:
  front: {}
  back: {}
volumes:
  db-data: {}
`;

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('compose-to-diagram page renders real Mermaid for a Compose stack', async ({ page }) => {
  await page.goto('/tools/compose-to-diagram/');
  await page.fill('#in-compose', STACK);
  await page.selectOption('#in-direction', 'LR');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('flowchart LR', { timeout: 15_000 });
  const text = await output(page);
  expect(text).toContain('svc_web["web<br/>nginx:1.27"]');
  expect(text).toContain('svc_api -->|"healthy"| svc_db');
  expect(text).toContain('port_web_0(["8080:80"]) --> svc_web');
  expect(text).toContain('vol_db_data[("db-data")]');
});

test('compose-to-diagram deep link pre-fills params and renders summary output', async ({ page }) => {
  const params = new URLSearchParams({
    compose: STACK,
    direction: 'BT',
    networks: 'off',
    ports: 'false',
    volumes: 'false',
    labels: 'name',
    styled: 'false',
    title: 'Checkout stack',
    output: 'summary',
  });
  await page.goto(`/tools/compose-to-diagram/?${params.toString()}`);

  await expect(page.locator('#in-direction')).toHaveValue('BT', { timeout: 15_000 });
  await expect(page.locator('#in-networks')).toHaveValue('off');
  await expect(page.locator('#in-ports')).not.toBeChecked();
  await expect(page.locator('#in-volumes')).not.toBeChecked();
  await expect(page.locator('#in-labels')).toHaveValue('name');
  await expect(page.locator('#in-styled')).not.toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('summary');

  const text = await output(page);
  expect(text).toContain('Compose summary — Checkout stack');
  expect(text).toContain('Services (3)');
  expect(text).toContain('depends_on: db (service_healthy)');
  expect(text).toContain('front: web, api');
});

test('compose-to-diagram page covers network node, full labels, markdown output and unchecked ports', async ({ page }) => {
  await page.goto('/tools/compose-to-diagram/');
  await page.fill('#in-compose', STACK);
  await page.selectOption('#in-networks', 'node');
  await page.selectOption('#in-labels', 'full');
  await page.selectOption('#in-output', 'markdown');
  await page.uncheck('#in-ports');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('```mermaid', { timeout: 15_000 });
  const text = await output(page);
  expect(text).toContain('flowchart TD');
  expect(text).toContain('net_front{{"net: front"}}');
  expect(text).toContain('svc_api -.-> net_back');
  expect(text).not.toContain('8080:80');
  expect(text.trimEnd()).toMatch(/```$/);
});

test('compose-to-diagram page reports YAML errors clearly', async ({ page }) => {
  await page.goto('/tools/compose-to-diagram/');
  await page.fill('#in-compose', 'services:\n  app: [unclosed\n');

  await expect(page.locator('#tool-output')).toContainText('invalid YAML', { timeout: 15_000 });
});
