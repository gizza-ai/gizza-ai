import { test, expect } from './fixtures';

const tool = '/tools/dot-to-mermaid/';
const DOT = 'digraph { rankdir=LR; start [label="Start", shape=circle]; check [label="Tests pass?", shape=diamond]; start -> check [label="build"]; check -> ship [label="yes"]; check -> fix [label="no", style=dashed]; }';

const DEFAULT = `flowchart LR
    start(("Start"))
    check{"Tests pass?"}
    start -->|build| check
    check -->|yes| ship
    check -. no .-> fix`;

async function setDot(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-dot').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: import('@playwright/test').Page,
  params: {
    dot?: string;
    direction?: string;
    shapes?: string;
    edge_labels?: string;
    link_styles?: string;
    subgraphs?: string;
    colors?: string;
    warnings?: string;
    title?: string;
    fence?: string;
  } = {},
) {
  const p = {
    dot: DOT,
    direction: 'auto',
    shapes: 'true',
    edge_labels: 'true',
    link_styles: 'true',
    subgraphs: 'true',
    colors: 'true',
    warnings: 'true',
    title: '',
    fence: 'false',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/dot-to-mermaid/gizza_ai_dot_to_mermaid_web.js');
    await mod.default('/tools/dot-to-mermaid/gizza_ai_dot_to_mermaid_web_bg.wasm');
    return mod.run(
      args.dot,
      args.direction,
      args.shapes,
      args.edge_labels,
      args.link_styles,
      args.subgraphs,
      args.colors,
      args.warnings,
      args.title,
      args.fence,
    );
  }, p);
}

test('dot-to-mermaid page converts a labelled LR digraph exactly', async ({ page }) => {
  await page.goto(tool);
  await setDot(page, DOT);
  await expect(page.locator('#tool-output')).toHaveText(DEFAULT, { timeout: 15_000 });
});

test('dot-to-mermaid deep-link renders a fenced titled diagram', async ({ page }) => {
  const qs = new URLSearchParams({
    dot: DOT,
    direction: 'auto',
    edge_labels: 'false',
    title: 'Release pipeline',
    fence: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-title')).toHaveValue('Release pipeline', { timeout: 15_000 });
  await expect(page.locator('#in-fence')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    `\`\`\`mermaid
---
title: Release pipeline
---
flowchart LR
    start(("Start"))
    check{"Tests pass?"}
    start --> check
    check --> ship
    check -.-> fix
\`\`\``,
    { timeout: 15_000 },
  );
});

test('dot-to-mermaid wasm covers advertised controls and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-dot');

  await expect(runWasm(page)).resolves.toBe(DEFAULT);
  await expect(runWasm(page, { direction: 'TD' })).resolves.toContain('flowchart TD');
  await expect(runWasm(page, { shapes: 'false' })).resolves.toContain('start["Start"]');
  await expect(runWasm(page, { edge_labels: 'false' })).resolves.not.toContain('|build|');
  await expect(runWasm(page, { link_styles: 'false' })).resolves.toContain('check -->|no| fix');

  const clustered = await runWasm(page, {
    dot: 'digraph { subgraph cluster_api { label="API"; a; b; } a -> b; }',
  });
  expect(clustered).toContain('subgraph cluster_api["API"]');
  expect(clustered).toContain('a --> b');

  const undirected = await runWasm(page, { dot: 'graph { a -- b; b -- c; }' });
  expect(undirected).toContain('flowchart TD');
  expect(undirected).toContain('a --- b');

  const styled = await runWasm(page, {
    dot: 'digraph { a [label="Queue", style=filled, fillcolor="#ffcc00"]; a -> b [color=red, penwidth=2]; }',
  });
  expect(styled).toContain('style a fill:#ffcc00');
  expect(styled).toContain('linkStyle 0 stroke:red,stroke-width:2px');

  await expect(runWasm(page, { direction: 'sideways' })).rejects.toThrow(/unknown direction/);
  await expect(runWasm(page, { dot: '' })).rejects.toThrow(/DOT source is empty/);
  await expect(runWasm(page, { dot: 'digraph { a -> ' })).rejects.toThrow(/expected/i);
});

test('dot-to-mermaid ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Decision flow',
    'Clustered services',
    'Undirected graph',
    'Styled, fenced for a README',
  ]);
});
