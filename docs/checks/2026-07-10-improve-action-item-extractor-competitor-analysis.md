# action-item-extractor — competitor scan (2026-07-10)

Scan of meeting action-item extractors, AI meeting assistants, and checklist parsers before implementation. Findings are paraphrased; no competitor copy or branding is reused.

## Competitors skimmed

1. AI meeting assistants such as Otter/Fireflies/MeetGeek-style products: ingest transcripts, summarize meetings, infer decisions and action items, assign speakers, and sync tasks to project tools.
2. Note apps and task plugins (Notion AI, Obsidian task queries, Roam/Todoist workflows): turn explicit `TODO`, checkbox, `@owner`, and tag patterns into lists; some use AI to rewrite or summarize.
3. Project-management importers (Asana/Trello/Jira paste/import flows): expect explicit rows or checklist syntax, preserve owners/dates when given, and avoid inferring hidden tasks.
4. Lightweight online "action item extractor" utilities: paste text, return bullets; most rely on LLM semantics and can invent or rewrite tasks.

## Table-stakes and model fit

| Feature | Tag | Decision |
| --- | --- | --- |
| Extract explicit TODO/ACTION/task markers | in-model | Built with deterministic line markers. |
| Detect owner assignments (`Alice will...`, `Bob to...`) | in-model | Built; short capitalized leading names only. |
| Detect `@handle` owner mentions | in-model | Built and normalized into owner labels. |
| Preserve verbatim task wording rather than summarizing | in-model | Built; output is cleaned but not rewritten semantically. |
| Pull decision lines (`Decided:`, agreed, resolved, approved) | in-model | Built as a Decisions section / JSON array. |
| Markdown checklist and JSON output | in-model | Built via `format=markdown|json`. |
| Group by owner | in-model | Built via `group_by=type|owner`. |
| Speaker diarization / transcript segmentation | out-of-model | Not built; requires transcript metadata or speech model. |
| LLM inference of implicit commitments | out-of-model | Not built; would hallucinate in a deterministic browser tool. |
| Due-date extraction from natural language | out-of-model | Not built; date parsing and reminder semantics should be a separate tool. |
| Project-tool sync | out-of-model | Not built; gizza tools are local transforms, not account integrations. |

## Descriptor decisions

The tool exposes `input` (multiline notes), `format` (`markdown` or `json`), `group_by` (`type` or `owner`), and `include_decisions` (boolean). Defaults favor a readable Markdown checklist with decisions included. The page copy emphasizes that the extractor is deterministic: it promotes explicit signals only and does not invent tasks the notes did not state.
