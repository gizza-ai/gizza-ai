// Pure URL-query → field-value mapping for tool pages. No DOM/window access so
// it is Node-unit-testable; tool.js applies the result to the page.
//
// Query-param names are the tool's input names (the same names used by the chat
// schema). `source="clock"` inputs are never query-params; a `source="file"`
// input is driven by `?url=` (a remote file to fetch), handled by tool.js.
export function queryPrefill(inputs, searchString) {
  const params = new URLSearchParams(searchString || '');
  const fields = [];
  for (const inp of inputs || []) {
    if (inp.source === 'field') {
      const v = params.get(inp.name);
      if (v != null) fields.push({ elementId: inp.elementId, value: v });
    }
  }
  return { fields, url: params.get('url') };
}
