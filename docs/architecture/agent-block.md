# `gizza-ai/agent` block

The agent block is the chat surface's runtime entry point: it accepts a user
turn (text plus optional uploads) and streams back assistant tokens or
skill output via Server-Sent Events.

## Routes

### `POST /b/agent/chat`

**Request body** (JSON):
```jsonc
{
  "user_message": "...",          // required
  "messages": [...],              // prior chat turns
  "model_id": "...",              // optional WebLLM model id
  "uploads": [...],               // optional, base64+mime
  "confirm_yes": { ... }          // optional, see "Confirm chips"
}
```

**Response**: `text/event-stream`

| Event         | Payload                                                              | When                                |
|---------------|----------------------------------------------------------------------|-------------------------------------|
| `token`       | `{ "delta": "..." }`                                                 | Assistant LLM streaming text        |
| `tool_result` | `{ "id", "input", "result", "for_ui"? }`                             | Skill output (slash-command dispatch) |
| `confirm`     | `{ "question", "yes": { "cmd", "params" } }`                         | LLM emitted `__unsure` during extraction |
| `done`        | `{ "reason": "stop" \| "error", "error"? }`                          | Stream terminator                   |

The `tool_result.input` field carries the JSON params actually dispatched to
the skill, so the UI can render a side-by-side Input/Output view of what the
LLM understood from the user's slash command.

### `GET /b/agent/commands`

Returns `application/json`:
```json
[ { "cmd": "<short-name>", "description": "..." }, ... ]
```

Used by the UI to populate the slash-command autocomplete picker.

## Slash-command flow

When `user_message` begins with `/`:

1. **Parse** the leading `/<cmd>` token and its remainder text.
2. **Look up** `gizza-ai/<cmd>` in the block registry. Unknown command → emit
   a `done` event with `reason: "error"`.
3. **Read** `BlockInfo::tool` (the skill's description + JSON-Schema
   parameters), then build the params payload via one of two paths:
   - **Verbatim path**: if the schema is `{"prompt": string, required:
     ["prompt"]}`-shaped, the remainder text is dispatched as
     `{"prompt": "<remainder>"}` without invoking the LLM.
   - **LLM extraction path**: otherwise, the agent issues a `chat_stream`
     call asking the LLM to emit a JSON object matching the schema. If the
     LLM responds with `{"__unsure": "...", "__params": {...}}`, the agent
     emits a `confirm` SSE event carrying the best-guess params and ends the
     stream — the UI surfaces a confirm chip so the user can approve.
4. **Dispatch** via `ctx.call_block_buffered_with_attachments`.
5. **Parse** the skill's response envelope (`_for_llm` / `_for_ui`) and emit a
   single `tool_result` SSE event, then `done`.

## Non-slash flow

Plain LLM chat — no `tools[]` advertisement. Tokens are buffered into the SSE
response body as `token` events. There is no multi-round agent loop; that
surface is intentionally replaced by user-invoked slash commands.

## Confirm chips

When the user clicks a confirm chip, the UI replays the original turn with
`confirm_yes: { "cmd": "...", "params": {...} }` set. The agent skips the
slash-parsing path and dispatches the confirmed params directly.
