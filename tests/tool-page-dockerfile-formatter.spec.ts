import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const messy = 'from   alpine:3.20 as   build\nrun apk add --no-cache curl \\\n  && curl --version\n#runtime\nfrom alpine\ncopy --from=build /usr/bin/curl /usr/bin/curl\n';

test('dockerfile-formatter normalizes the default Dockerfile style', async ({ page }) => {
  await page.goto('/tools/dockerfile-formatter/');
  await setField(page, '#in-input', messy);

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    'FROM alpine:3.20 AS build\nRUN apk add --no-cache curl \\\n    && curl --version\n\n# runtime\nFROM alpine\nCOPY --from=build /usr/bin/curl /usr/bin/curl\n',
    { timeout: 15_000 },
  );
});

test('dockerfile-formatter honors deep-linked lower-case aligned settings', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'FROM node:22 AS deps\nRUN npm ci \\\n&& npm cache clean --force\nCOPY . /app',
    instruction_case: 'lower',
    indent: '2',
    align_continuations: 'true',
    max_blank_lines: '1',
    blank_line_between_stages: 'true',
    normalize_comments: 'true',
  });
  await page.goto(`/tools/dockerfile-formatter/?${params.toString()}`);

  await expect(page.locator('#in-align_continuations')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'from node:22 as deps\nrun npm ci \\\n  && npm cache clean --force\ncopy . /app\n',
    { timeout: 15_000 },
  );
});

test('dockerfile-formatter covers advertised enum choices and toggles', async ({ page }) => {
  await page.goto('/tools/dockerfile-formatter/');

  await setField(page, '#in-input', 'From alpine As build\nRUN echo hi\n');
  await page.selectOption('#in-instruction_case', 'preserve');
  await expect(page.locator('#tool-output')).toHaveText('From alpine As build\nRUN echo hi\n', {
    timeout: 15_000,
  });

  await page.selectOption('#in-instruction_case', 'lower');
  await expect(page.locator('#tool-output')).toHaveText('from alpine as build\nrun echo hi\n');

  await setField(page, '#in-input', '#note\nFROM alpine\n\n\nUSER app\n');
  await page.selectOption('#in-instruction_case', 'upper');
  await setField(page, '#in-max_blank_lines', '0');
  await page.uncheck('#in-normalize_comments');
  await page.uncheck('#in-blank_line_between_stages');
  await expect(page.locator('#tool-output')).toHaveText('#note\nFROM alpine\nUSER app\n');
});

test('dockerfile-formatter preserves parser directives and heredocs', async ({ page }) => {
  await page.goto('/tools/dockerfile-formatter/');
  await setField(page, '#in-input', '# escape=`\nfrom mcr.microsoft.com/windows/nanoserver\nrun <<EOF\n  keep   spaces\nEOF\n');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('# escape=`', { timeout: 15_000 });
  await expect(out).toContainText('FROM mcr.microsoft.com/windows/nanoserver');
  await expect(out).toContainText('RUN <<EOF\n  keep   spaces\nEOF');
});

test('dockerfile-formatter enforces numeric boundaries and syntax errors', async ({ page }) => {
  await page.goto('/tools/dockerfile-formatter/');
  await setField(page, '#in-input', 'FROM alpine\n');
  await setField(page, '#in-indent', '9');
  await expect(page.locator('#tool-output')).toContainText('indent must be between 0 and 8', {
    timeout: 15_000,
  });

  await setField(page, '#in-indent', '8');
  await setField(page, '#in-max_blank_lines', '6');
  await expect(page.locator('#tool-output')).toContainText('max_blank_lines must be between 0 and 5');

  await setField(page, '#in-max_blank_lines', '5');
  await setField(page, '#in-input', 'FROM alpine\nRNU echo hi\n');
  await expect(page.locator('#tool-output')).toContainText("line 2: unknown Dockerfile instruction 'RNU'");
});
