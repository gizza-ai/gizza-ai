// Tools modal: a searchable directory of every standalone tool page.
// Triggered by the composer hammer button (#open-tools). The list is a static
// JSON index (/tools/_index.json) emitted at build time and served under the
// /tools/ Service-Worker bypass; we fetch it once on first open and filter
// client-side, so the page itself ships none of it.

const INDEX_URL = '/tools/_index.json';

/** Pure: substring match (case-insensitive) over title + description + slug + tags.
 * Slug is included so a tool's common name (e.g. "clock") finds it even when the
 * title doesn't contain it ("Current UTC Time"); tags are extra search keywords
 * (not displayed) authored in each tool's meta.toml. */
export function filterTools(list, query) {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter((t) => {
    const hay = [t.title, t.description, t.slug, ...(t.tags || [])].join(' ').toLowerCase();
    return hay.includes(q);
  });
}

function rowHtml(t) {
  const esc = (s) =>
    String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return (
    `<li><a href="/tools/${encodeURIComponent(t.slug)}/">` +
    `<span class="tool-title">${esc(t.title)}</span>` +
    `<span class="tool-desc">${esc(t.description)}</span>` +
    `</a></li>`
  );
}

function initToolsModal() {
  const dialog = document.getElementById('tools-modal');
  const openBtn = document.getElementById('open-tools');
  if (!dialog || !openBtn) return;

  const search = dialog.querySelector('#tools-search');
  const results = dialog.querySelector('#tools-results');
  const empty = dialog.querySelector('#tools-empty');
  const errorBox = dialog.querySelector('#tools-error');
  const retryBtn = dialog.querySelector('#tools-retry');
  if (!search || !results || !empty || !errorBox || !retryBtn) return;

  let all = null; // cached index once loaded

  function render() {
    if (!all) return;
    const matches = filterTools(all, search.value);
    results.innerHTML = matches.map(rowHtml).join('');
    empty.hidden = matches.length > 0;
  }

  async function load() {
    errorBox.hidden = true;
    empty.hidden = true;
    if (all) {
      render();
      return;
    }
    results.innerHTML = '';
    try {
      const res = await fetch(INDEX_URL);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      all = await res.json();
      render();
    } catch (e) {
      console.error('[gizza] tools index load failed:', e);
      errorBox.hidden = false;
    }
  }

  openBtn.addEventListener('click', () => {
    dialog.showModal();
    search.focus();
    load();
  });
  search.addEventListener('input', render);
  // Close ✕ uses native <form method="dialog"> (no JS needed). Also close on
  // a backdrop click (clicking outside the modal content).
  dialog.addEventListener('click', (e) => {
    if (e.target === dialog) dialog.close();
  });
  retryBtn.addEventListener('click', () => {
    all = null;
    load();
  });
}

// Only wire DOM in a browser — keeps filterTools importable under node:test.
if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initToolsModal);
  } else {
    initToolsModal();
  }
}
