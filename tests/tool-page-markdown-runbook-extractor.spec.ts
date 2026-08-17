import { test, expect } from './fixtures';

const tool = '/tools/markdown-runbook-extractor/';
const RUNBOOK = `# Deploy the API

## Install dependencies

\`\`\`console
$ npm ci
added 42 packages
\`\`\`

## Run migrations

\`\`\`bash name=migrate
./manage.py migrate
\`\`\`

## Rollback (do not run)

\`\`\`bash skip
./manage.py migrate --rollback
\`\`\``;

const SCRIPT = `#!/usr/bin/env bash
# Runbook: 2 runnable task(s) of 3 extracted from Markdown.
# Tasks:
#   1. Install dependencies
#   2. migrate
#   3. Rollback (do not run)  [skipped: tagged skip]

set -euo pipefail

# --- 1/3 · Install dependencies (console, line 5) ---
echo "==> [1/3] Install dependencies"
npm ci

# --- 2/3 · migrate (bash, line 12) ---
echo "==> [2/3] migrate"
./manage.py migrate

# --- 3/3 · Rollback (do not run) (bash, line 18) — SKIPPED, tagged skip ---
# ./manage.py migrate --rollback`;

async function setMarkdown(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-markdown').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: import('@playwright/test').Page,
  params: {
    markdown?: string;
    language?: string;
    output?: string;
    tags?: string;
    strip_prompts?: string;
    echo_steps?: string;
    fail_fast?: string;
    skip_marked?: string;
  } = {},
) {
  const p = {
    markdown: RUNBOOK,
    language: 'auto',
    output: 'script',
    tags: '',
    strip_prompts: 'true',
    echo_steps: 'true',
    fail_fast: 'true',
    skip_marked: 'true',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/markdown-runbook-extractor/gizza_ai_markdown_runbook_extractor_web.js');
    await mod.default('/tools/markdown-runbook-extractor/gizza_ai_markdown_runbook_extractor_web_bg.wasm');
    return mod.run(
      args.markdown,
      args.language,
      args.output,
      args.tags,
      args.strip_prompts,
      args.echo_steps,
      args.fail_fast,
      args.skip_marked,
    );
  }, p);
}

test('markdown-runbook-extractor page turns a runbook into a runnable script', async ({ page }) => {
  await page.goto(tool);
  await setMarkdown(page, RUNBOOK);
  await expect(page.locator('#tool-output')).toHaveText(SCRIPT, { timeout: 15_000 });
});

test('markdown-runbook-extractor deep-link renders a task checklist', async ({ page }) => {
  const qs = new URLSearchParams({ markdown: RUNBOOK, output: 'tasks' });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('tasks', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toHaveText(
    `# Runbook tasks (2 runnable of 3)

- [ ] 1. Install dependencies — \`console\`, 1 line, line 5
- [ ] 2. migrate — \`bash\`, 1 line, line 12
- [ ] 3. ~~Rollback (do not run)~~ — \`bash\`, 1 line, line 18, tags: skip — skipped, tagged skip`,
    { timeout: 15_000 },
  );
});

test('markdown-runbook-extractor wasm covers controls and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-markdown');

  await expect(runWasm(page)).resolves.toBe(SCRIPT);
  await expect(runWasm(page, { output: 'json' })).resolves.toContain('"count": 3');
  await expect(runWasm(page, { tags: 'deploy' })).rejects.toThrow(/no code blocks matched tags 'deploy'/);
  await expect(
    runWasm(page, {
      markdown: '# Release\n\n```bash#build\nmake build\n```\n\n```bash#test\nmake test\n```\n\n```bash#deploy\nmake deploy\n```',
      tags: 'build,deploy',
    }),
  ).resolves.toContain('make deploy');
  await expect(runWasm(page, { strip_prompts: 'false' })).resolves.toContain('added 42 packages');
  await expect(runWasm(page, { echo_steps: 'false' })).resolves.not.toContain('==> [1/3]');
  await expect(runWasm(page, { fail_fast: 'false' })).resolves.not.toContain('set -euo pipefail');
  await expect(runWasm(page, { skip_marked: 'false' })).resolves.toContain('./manage.py migrate --rollback');
  await expect(
    runWasm(page, {
      markdown: '## Load\n\n```pycon\n>>> import pandas as pd\n>>> len(df)\n1042\n```',
      language: 'python',
    }),
  ).resolves.toContain('import pandas as pd');
  await expect(runWasm(page, { language: 'ruby' })).rejects.toThrow(/expected language/);
  await expect(runWasm(page, { markdown: '# No code here' })).rejects.toThrow(/no fenced code blocks found/);
});

test('markdown-runbook-extractor ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Runbook → runnable script',
    'Ordered task checklist',
    'Steps as JSON',
    'Only build + deploy tags',
    'Python notebook-style runbook',
  ]);
});
