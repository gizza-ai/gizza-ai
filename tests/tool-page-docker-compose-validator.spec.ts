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

const CLEAN = `services:
  web:
    image: nginx:1.27-alpine
    ports:
      - "127.0.0.1:8080:80"
    depends_on:
      - api
    networks:
      - appnet
  api:
    build: ./api
    restart: unless-stopped
    networks:
      - appnet
networks:
  appnet:`;

const BROKEN = `services:
  web:
    image: nginx:latest
    ports:
      - "8080:80"
    depends_on:
      - api
    volumes:
      - data:/usr/share/nginx/html`;

const BAD_PORTS = `services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
  admin:
    image: nginx:alpine
    ports:
      - "8080:8080"
  typo:
    image: busybox:1.36
    ports:
      - "70000:80"`;

test('docker-compose-validator page reports an exact valid result', async ({ page }) => {
  await page.goto('/tools/docker-compose-validator/');
  await setField(page, '#in-input', CLEAN);

  await expect(page.locator('#tool-output')).toContainText('No problems found.', { timeout: 15_000 });
  expect(await outputText(page)).toBe(
    'VALID — 2 services, 1 network, 0 volumes\n' +
      'preset default — 0 errors, 0 warnings, 0 hints\n\n' +
      'No problems found.'
  );
});

test('docker-compose-validator deep-link fills params and reports undefined references', async ({ page }) => {
  const params = new URLSearchParams({
    input: BROKEN,
    preset: 'default',
    disable: '',
    strict_warnings: 'false',
    min_severity: 'hint',
    report_format: 'report',
  });

  await page.goto(`/tools/docker-compose-validator/?${params.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(BROKEN, { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('undefined-depends-on', { timeout: 15_000 });
  const out = await outputText(page);
  expect(out).toContain('INVALID — 1 service, 0 networks, 0 volumes');
  expect(out).toContain('undefined-volume');
  expect(out).toContain('image-tag');
});

test('docker-compose-validator exercises enum choices and strict warnings checkbox', async ({ page }) => {
  await page.goto('/tools/docker-compose-validator/');
  await setField(page, '#in-input', BAD_PORTS);
  await page.selectOption('#in-preset', 'essential');
  await page.selectOption('#in-min_severity', 'error');
  await page.selectOption('#in-report_format', 'json');
  await page.check('#in-strict_warnings');

  await expect(page.locator('#tool-output')).toContainText('duplicate-host-port', { timeout: 15_000 });
  const out = await outputText(page);
  expect(out).toContain('"valid": false');
  expect(out).toContain('"rule": "port-syntax"');
  expect(out).toContain('"rule": "duplicate-host-port"');
});

test('docker-compose-validator shows a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/docker-compose-validator/');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool docker-compose-validator');
  expect(cli).toContain('services:');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
