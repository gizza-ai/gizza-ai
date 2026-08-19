import { test, expect } from './fixtures';

const SAMPLE = `window.YTD.tweets.part0 = [
  {"tweet":{"id_str":"1746900000000000001","created_at":"Mon Jan 15 09:30:00 +0000 2024","full_text":"Shipped the new parser today &amp; it is fast https://t.co/abc123 #rust","favorite_count":"42","retweet_count":"7","lang":"en","source":"<a href=\\\"https://x.com\\\" rel=\\\"nofollow\\\">Twitter Web App</a>","entities":{"hashtags":[{"text":"rust"}],"user_mentions":[],"urls":[{"url":"https://t.co/abc123","expanded_url":"https://example.com/parser","display_url":"example.com/parser"}]}}},
  {"tweet":{"id_str":"1746900000000000002","created_at":"Mon Jan 15 11:00:00 +0000 2024","full_text":"@bob good point, will fix","favorite_count":"3","retweet_count":"0","lang":"en","source":"<a href=\\\"https://x.com\\\" rel=\\\"nofollow\\\">Twitter for iPhone</a>","in_reply_to_status_id_str":"1746800000000000009","in_reply_to_screen_name":"bob","entities":{"hashtags":[],"user_mentions":[{"screen_name":"bob"}],"urls":[]}}},
  {"tweet":{"id_str":"1746900000000000003","created_at":"Tue Jan 16 08:15:00 +0000 2024","full_text":"RT @carol: release notes are up #rust","favorite_count":"0","retweet_count":"12","lang":"en","source":"<a href=\\\"https://x.com\\\" rel=\\\"nofollow\\\">Twitter Web App</a>","entities":{"hashtags":[{"text":"rust"}],"user_mentions":[{"screen_name":"carol"}],"urls":[]}}}
]`;

async function runWasm(
  page: any,
  tweets: string = SAMPLE,
  output = 'both',
  format = 'text',
  sort = 'newest',
  search = '',
  since = '',
  until = '',
  includeReplies = 'true',
  includeRetweets = 'true',
  expandUrls = 'true',
  topCount = '5',
  maxTweets = '0',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/twitter-archive-reader/gizza_ai_twitter_archive_reader_web.js');
    await mod.default('/tools/twitter-archive-reader/gizza_ai_twitter_archive_reader_web_bg.wasm');
    return mod.run(
      args.tweets,
      args.output,
      args.format,
      args.sort,
      args.search,
      args.since,
      args.until,
      args.includeReplies,
      args.includeRetweets,
      args.expandUrls,
      args.topCount,
      args.maxTweets,
    );
  }, { tweets, output, format, sort, search, since, until, includeReplies, includeRetweets, expandUrls, topCount, maxTweets });
}

test('twitter-archive-reader page renders real stats and transcript', async ({ page }) => {
  await page.goto('/tools/twitter-archive-reader/');
  await page.fill('#in-tweets', SAMPLE);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('**Tweets in file**: 3', { timeout: 20_000 });
  await expect(output).toContainText('**Tweets shown**: 3 (1 original, 1 replies, 1 retweets)');
  await expect(output).toContainText('### Top hashtags');
  await expect(output).toContainText('#rust');
  await expect(output).toContainText('Shipped the new parser today & it is fast https://example.com/parser #rust');
  await expect(output).toContainText('https://twitter.com/i/web/status/1746900000000000001');
});

test('twitter-archive-reader deep link covers CSV, date/search filters and checkbox state', async ({ page }) => {
  const params = new URLSearchParams({
    tweets: SAMPLE,
    output: 'transcript',
    format: 'csv',
    sort: 'oldest',
    search: 'parser',
    since: '2024-01-15',
    until: '2024-01-15',
    include_replies: 'false',
    include_retweets: 'false',
    expand_urls: 'true',
    top_count: '1',
    max_tweets: '1',
  });
  await page.goto(`/tools/twitter-archive-reader/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('csv', { timeout: 15_000 });
  await expect(page.locator('#in-sort')).toHaveValue('oldest');
  await expect(page.locator('#in-include_replies')).not.toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('date,id,kind,likes,retweets,language,source,text,permalink', { timeout: 20_000 });
  await expect(output).toContainText('2024-01-15T09:30:00Z,1746900000000000001,original,42,7,en,Twitter Web App,Shipped the new parser today & it is fast https://example.com/parser #rust,https://twitter.com/i/web/status/1746900000000000001');
  await expect(output).not.toContainText('@bob good point');
});

test('twitter-archive-reader wasm covers enum values, cap boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/twitter-archive-reader/');

  const markdown = await runWasm(page, SAMPLE, 'both', 'markdown', 'newest', '', '', '', 'true', 'true', 'true', '2', '0');
  expect(markdown).toContain('## Summary');
  expect(markdown).toContain('| #rust | 2 | 66.67% |');
  expect(markdown).toContain('### 2024-01-16 08:15:00 UTC');
  expect(markdown).toContain('*retweet · 0 likes · 12 retweets');

  const html = await runWasm(page, SAMPLE, 'transcript', 'html', 'likes');
  expect(html).toContain('<article>');
  expect(html).toContain('— original · 42 likes · 7 retweets');
  expect(html).toContain('https://example.com/parser');

  const statsCsv = await runWasm(page, SAMPLE, 'stats', 'csv', 'retweets', '', '', '', 'true', 'true', 'true', '1', '2');
  expect(statsCsv).toContain('summary,Tweets shown,"2 (1 original, 0 replies, 1 retweets)"');
  expect(statsCsv).toContain('summary,Truncated to,2 tweets');
  expect(statsCsv).toContain('top,2024-01-15 — Shipped the new parser today & it is fast https://example.com/parser #rust');

  const textNoRetweetsNoExpand = await runWasm(page, SAMPLE, 'transcript', 'text', 'oldest', '', '', '', 'true', 'false', 'false', '0', '0');
  expect(textNoRetweetsNoExpand).toContain('https://t.co/abc123');
  expect(textNoRetweetsNoExpand).not.toContain('RT @carol');

  await expect(runWasm(page, 'not json')).rejects.toThrow(/could not parse tweets\.js|not valid JSON|expected JSON/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool twitter-archive-reader');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
