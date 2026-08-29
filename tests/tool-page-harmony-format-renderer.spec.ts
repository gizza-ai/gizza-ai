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

const MINIMAL_JSON = '[{"role":"user","content":"What is 2 + 2?"}]';
const TOOL_LINES = 'user: what is the weather in Oslo?\nassistant[analysis]: the user wants the current weather\nassistant[commentary] to=get_weather: {"city":"Oslo"}\ntool:get_weather: {"c":21}\nassistant: It is 21 C in Oslo.';
const TOOL_SCHEMA = '[{"name":"get_weather","description":"Get weather for a city.","parameters":{"type":"object","properties":{"city":{"type":"string","description":"City name."},"unit":{"type":"string","enum":["celsius","fahrenheit"],"default":"celsius"}},"required":["city"]}}]';

test('renders a minimal Harmony prompt with exact system and user tokens', async ({ page }) => {
  await page.goto('/tools/harmony-format-renderer/');
  await setTextarea(page, '#in-messages', MINIMAL_JSON);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<|start|>system<|message|>You are ChatGPT', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toBe('<|start|>system<|message|>You are ChatGPT, a large language model trained by OpenAI.\nKnowledge cutoff: 2024-06\n\nReasoning: medium\n\n# Valid channels: analysis, commentary, final. Channel must be included for every message.<|end|><|start|>user<|message|>What is 2 + 2?<|end|><|start|>assistant');
});

test('deep-link pre-fills controls and renders conversation-only without system metadata', async ({ page }) => {
  await page.goto(
    '/tools/harmony-format-renderer/?messages=' +
      encodeURIComponent('user: hi') +
      '&input_format=lines&include_system=false&render_target=conversation&auto_drop_analysis=true&output_format=text',
  );

  await expect(page.locator('#in-messages')).toHaveValue('user: hi', { timeout: 15000 });
  await expect(page.locator('#in-input_format')).toHaveValue('lines');
  await expect(page.locator('#in-include_system')).not.toBeChecked();
  await expect(page.locator('#in-render_target')).toHaveValue('conversation');
  await expect(page.locator('#tool-output')).toContainText('<|start|>user<|message|>hi<|end|>', { timeout: 15000 });
  expect(await outText(page)).toBe('<|start|>user<|message|>hi<|end|>');
});

test('line format, tool schema and non-default checkbox render tool calls', async ({ page }) => {
  await page.goto('/tools/harmony-format-renderer/');
  await setTextarea(page, '#in-messages', TOOL_LINES);
  await page.selectOption('#in-input_format', 'lines');
  await setTextarea(page, '#in-instructions', 'Always give temperatures in Celsius.');
  await setTextarea(page, '#in-tools', TOOL_SCHEMA);
  await page.selectOption('#in-render_target', 'conversation');
  await page.uncheck('#in-auto_drop_analysis');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('namespace functions', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain("Calls to these tools must go to the commentary channel: 'functions'.");
  expect(text).toContain('type get_weather = (_: {');
  expect(text).toContain('<|start|>assistant<|channel|>analysis<|message|>the user wants the current weather<|end|>');
  expect(text).toContain('<|start|>assistant<|channel|>commentary to=functions.get_weather <|constrain|>json<|message|>{"city":"Oslo"}<|call|>');
  expect(text).toContain('<|start|>functions.get_weather to=assistant<|channel|>commentary<|message|>{"c":21}<|end|>');
});

test('JSON output reports counts and accepted enum values', async ({ page }) => {
  await page.goto('/tools/harmony-format-renderer/');
  await setTextarea(page, '#in-messages', MINIMAL_JSON);
  await page.selectOption('#in-reasoning_effort', 'high');
  await page.fill('#in-model_identity', 'You are a release-notes assistant.');
  await page.fill('#in-knowledge_cutoff', '2025-01');
  await page.selectOption('#in-output_format', 'json');

  await expect(page.locator('#tool-output')).toContainText('"message_count": 1', { timeout: 15000 });
  const parsed = JSON.parse(await outText(page));
  expect(parsed.message_count).toBe(1);
  expect(parsed.rendered_message_count).toBe(1);
  expect(parsed.dropped_analysis_count).toBe(0);
  expect(parsed.tool_count).toBe(0);
  expect(parsed.stop_tokens).toEqual(['<|return|>', '<|call|>']);
  expect(parsed.prompt).toContain('You are a release-notes assistant.');
  expect(parsed.prompt).toContain('Knowledge cutoff: 2025-01');
  expect(parsed.prompt).toContain('Reasoning: high');
});

test('cap boundary succeeds and one over cap fails clearly', async ({ page }) => {
  await page.goto('/tools/harmony-format-renderer/');
  const boundary = 'user: ' + 'a'.repeat(199_994);
  await setTextarea(page, '#in-messages', boundary);
  await page.selectOption('#in-input_format', 'lines');
  await page.uncheck('#in-include_system');
  await expect(page.locator('#tool-output')).toContainText('<|start|>user<|message|>', { timeout: 15000 });

  await setTextarea(page, '#in-messages', boundary + 'b');
  await expect(page.locator('#tool-output')).toContainText('messages is too large: expected at most 200000 characters', { timeout: 15000 });
});
