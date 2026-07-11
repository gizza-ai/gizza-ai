import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('extracts explicit actions, owners and decisions as markdown', async ({ page }) => {
  await page.goto('/tools/action-item-extractor/');
  await page.fill('#in-input', 'ACTION: update the roadmap\nAlice will send the deck\nBook the venue @bob\nDecided: ship on Friday');
  await expect(page.locator('#tool-output')).toContainText('## Action Items', { timeout: 15000 });
  expect(await output(page)).toBe(
    '## Action Items\n\n' +
      '- [ ] update the roadmap — _Unassigned_\n' +
      '- [ ] send the deck — **@Alice**\n' +
      '- [ ] Book the venue — **@Bob**\n\n' +
      '## Decisions\n\n' +
      '- ship on Friday',
  );
});

test('groups markdown output by owner', async ({ page }) => {
  await page.goto('/tools/action-item-extractor/');
  await page.fill('#in-input', 'Alice will send the deck\nBob to book the venue\nUpdate the wiki');
  await page.selectOption('#in-group_by', 'owner');
  await expect(page.locator('#tool-output')).toContainText('### Alice', { timeout: 15000 });
  expect(await output(page)).toBe(
    '## Action Items\n\n' +
      '### Alice\n\n' +
      '- [ ] send the deck\n\n' +
      '### Bob\n\n' +
      '- [ ] book the venue\n\n' +
      '### Unassigned\n\n' +
      '- [ ] Update the wiki',
  );
});

test('json format returns machine-readable action_items and decisions', async ({ page }) => {
  await page.goto('/tools/action-item-extractor/');
  await page.fill('#in-input', 'Alice will send the deck\nDecided: ship Friday');
  await page.selectOption('#in-format', 'json');
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    '{"action_items":[{"task":"send the deck","owner":"Alice"}],"decisions":["ship Friday"]}',
  );
});

test('deep-link pre-fills input and include_decisions=false hides decisions', async ({ page }) => {
  const input = encodeURIComponent('Decided: ship Friday\nAlice will test');
  await page.goto(`/tools/action-item-extractor/?input=${input}&include_decisions=false`);
  await expect(page.locator('#tool-output')).toContainText('## Action Items', { timeout: 15000 });
  await expect(page.locator('#in-include_decisions')).not.toBeChecked();
  expect(await output(page)).toBe('## Action Items\n\n- [ ] test — **@Alice**');
});
