import { test, expect } from './fixtures';

const IOS_CHAT = `[2024-01-05, 21:07:33] Alice: Hey Bob 😂😂 how are you?
[2024-01-05, 21:08:01] Bob: good thanks! www.example.com
[2024-01-06, 09:15:00] Alice: image omitted
[2024-01-06, 09:16:10] Bob: nice 😂`;

const ANDROID_CHAT = `05/01/2024, 21:07 - Alice: pizza tonight? 🍕
05/01/2024, 21:09 - Bob: yes please 🍕🍕
06/01/2024, 08:00 - Alice: morning`;

async function setMaybeSelect(page, selector: string, value: string) {
  const el = page.locator(selector);
  const tag = await el.evaluate((node) => node.tagName.toLowerCase());
  if (tag === 'select') {
    await el.selectOption(value);
  } else if (tag === 'input') {
    const type = await el.getAttribute('type');
    if (type === 'checkbox') {
      const checked = value === 'true';
      if ((await el.isChecked()) !== checked) await el.setChecked(checked);
    } else {
      await el.fill(value);
    }
  } else {
    await el.fill(value);
  }
}

test('whatsapp-chat-analyzer page summarizes an iOS export', async ({ page }) => {
  await page.goto('/tools/whatsapp-chat-analyzer/');
  await page.waitForSelector('#in-chat');
  await page.fill('#in-chat', IOS_CHAT);
  await setMaybeSelect(page, '#in-date_format', 'auto');
  await page.fill('#in-top', '10');
  await page.fill('#in-min_word_length', '3');
  await setMaybeSelect(page, '#in-ignore_stopwords', 'true');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Messages: 4');
  await expect(output).toContainText('Participants: 2');
  await expect(output).toContainText('Media messages: 1');
  await expect(output).toContainText('Links: 1');
  await expect(output).toContainText('2   50.00%  Alice');
  await expect(output).toContainText('2   50.00%  Bob');
  await expect(output).toContainText('3  😂');
});

test('whatsapp-chat-analyzer honors query params for Android export and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    chat: ANDROID_CHAT,
    date_format: 'dmy',
    top: '0',
    min_word_length: '1',
    ignore_stopwords: 'false',
  });
  await page.goto(`/tools/whatsapp-chat-analyzer/?${params.toString()}`);
  await page.waitForSelector('#in-chat');

  await expect(page.locator('#in-chat')).toHaveValue(ANDROID_CHAT);
  await expect(page.locator('#in-date_format')).toHaveValue('dmy');
  await expect(page.locator('#in-top')).toHaveValue('0');
  await expect(page.locator('#in-min_word_length')).toHaveValue('1');
  await expect(page.locator('#in-ignore_stopwords')).not.toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Messages: 3');
  await expect(output).toContainText('2   66.67%  Alice');
  await expect(output).toContainText('1   33.33%  Bob');
  await expect(output).toContainText('Top words (min length 1)');
  await expect(output).toContainText('3  🍕');
});

test('whatsapp-chat-analyzer wasm export reports parse errors', async ({ page }) => {
  await page.goto('/tools/whatsapp-chat-analyzer/');
  await page.waitForSelector('#in-chat');
  const result = await page.evaluate(async ({ chat }) => {
    const mod = await import('/tools/whatsapp-chat-analyzer/gizza_ai_whatsapp_chat_analyzer_web.js');
    await mod.default('/tools/whatsapp-chat-analyzer/gizza_ai_whatsapp_chat_analyzer_web_bg.wasm');
    return mod.run(chat, 'mdy', '10', '3', 'true');
  }, { chat: '[2024-01-05, 10:00:00] A: hi 😂' });
  expect(result).toContain('Messages: 1');
  expect(result).toContain('A');
  expect(result).toContain('😂');

  await expect(page.evaluate(async () => {
    const mod = await import('/tools/whatsapp-chat-analyzer/gizza_ai_whatsapp_chat_analyzer_web.js');
    await mod.default('/tools/whatsapp-chat-analyzer/gizza_ai_whatsapp_chat_analyzer_web_bg.wasm');
    return mod.run('not an exported chat', 'auto', '10', '3', 'true');
  })).rejects.toThrow(/No WhatsApp messages/);
});
