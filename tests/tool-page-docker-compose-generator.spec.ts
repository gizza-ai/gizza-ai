import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const SIMPLE = `services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"`;

const STACK = `name: shop
services:
  web:
    image: nginx:alpine
    restart: unless-stopped
    ports:
      - "8080:80"
    networks:
      - appnet
    depends_on:
      - db
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_PASSWORD: "secret"
    volumes:
      - dbdata:/var/lib/postgresql/data
    networks:
      - appnet
volumes:
  dbdata:
networks:
  appnet:
    driver: bridge`;

test('docker-compose-generator page emits exact YAML for a minimal service', async ({ page }) => {
  await page.goto('/tools/docker-compose-generator/');
  await setField(page, '#in-services', 'web: nginx:alpine ports=8080:80');

  await expect(page.locator('#tool-output')).toContainText('services:', { timeout: 15_000 });
  expect(await outputText(page)).toBe(SIMPLE);
});

test('docker-compose-generator deep-link fills params and renders a full stack', async ({ page }) => {
  const params = new URLSearchParams({
    services:
      'web: nginx:alpine ports=8080:80 depends=db\n' +
      'db: postgres:16-alpine volumes=dbdata:/var/lib/postgresql/data env=POSTGRES_PASSWORD=secret',
    project_name: 'shop',
    compose_version: 'none',
    network: 'appnet',
    network_driver: 'bridge',
    restart: 'unless-stopped',
    env: '',
    env_file: '',
  });

  await page.goto(`/tools/docker-compose-generator/?${params.toString()}`);

  await expect(page.locator('#in-project_name')).toHaveValue('shop', { timeout: 15_000 });
  await expect(page.locator('#in-network')).toHaveValue('appnet');
  await expect(page.locator('#in-restart')).toHaveValue('unless-stopped');
  await expect(page.locator('#tool-output')).toContainText('volumes:', { timeout: 15_000 });
  expect(await outputText(page)).toBe(STACK);
});

test('docker-compose-generator reports validation errors on the page', async ({ page }) => {
  await page.goto('/tools/docker-compose-generator/');
  await setField(page, '#in-services', 'web: nginx portz=80');

  await expect(page.locator('#tool-output')).toContainText("unknown option 'portz=80'", {
    timeout: 15_000,
  });
});

test('docker-compose-generator shows a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/docker-compose-generator/');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool docker-compose-generator');
  expect(cli).toContain('web: nginx:alpine ports=8080:80');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
