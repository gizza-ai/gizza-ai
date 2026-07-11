# slack-export-reader — competitor analysis (2026-07-11)

Function: parse a Slack workspace export ZIP (`users.json`, `channels.json`, per-channel `YYYY-MM-DD.json` files) into a readable Markdown or HTML transcript. Pure Rust; chat + CLI file-input tool, no standalone page because the current page model has no binary-file-input → text-output surface.

## Competitors scanned (top 3)

1. **Slack export viewer / archive browser projects** — browse channels and messages from Slack export JSON, resolving users and channel names.
2. **slack-export-viewer style Python tools** — render Slack exports as static HTML with channels, timestamps, and message text.
3. **JSON-to-transcript scripts / converters** — convert channel JSON files into Markdown/HTML for archiving or migration.

## Table-stakes feature matrix

| Capability | Export viewers | Static HTML renderers | Converter scripts | in gizza | fit |
|---|---|---|---|---|---|
| Read ZIP export structure | ✅ | ✅ | mixed | ✅ | in-model |
| Parse `users.json` and resolve user IDs | ✅ | ✅ | ✅ | ✅ | in-model |
| Parse `channels.json` and resolve channel IDs | ✅ | ✅ | mixed | ✅ | in-model |
| Read per-channel `YYYY-MM-DD.json` message files | ✅ | ✅ | ✅ | ✅ | in-model |
| Render Markdown transcript | mixed | ➖ | ✅ | ✅ | in-model |
| Render standalone HTML transcript | ✅ | ✅ | mixed | ✅ | in-model |
| Channel filter | ✅ | ✅ | mixed | ✅ | in-model |
| Date filter | ✅ | mixed | mixed | ✅ | in-model |
| Resolve `<@U…>`, `<#C…|name>`, `<!here>`, `<url|label>` markup | ✅ | ✅ | mixed | ✅ | in-model |
| Attachments/files/thread reconstruction/reactions | ✅ | mixed | mixed | ❌ | out-of-scope |
| Interactive search UI | ✅ | mixed | ➖ | ❌ | out-of-scope |

## Decisions

- **Single-file input**: accepts the official Slack export ZIP. This matches gizza's `Input::File` model and avoids multi-file upload complexity.
- **Transcript formats**: `markdown` (default) and `html` cover the common archive/share outputs without requiring an interactive viewer.
- **Filters**: optional `channel` and `date` provide practical narrowing for large exports while keeping the descriptor small.
- **Slack markup**: user mentions, channel mentions, special commands, and labeled links are rewritten to readable text/links; unmodeled Slack fields are ignored rather than copied raw.
- **No page**: binary file input with text output is a chat/CLI surface in this repo; there is no generic browser page surface for arbitrary ZIP upload to text output.

## Out of scope

Attachments, file downloads, reactions, thread nesting, search indexes, and full interactive archive browsing are not built. They are larger viewer/database features, not necessary for a deterministic transcript conversion block.
