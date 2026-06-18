// Tools modal: a searchable directory of every standalone tool page.
// Triggered by the composer hammer button (#open-tools). The list is a static
// JSON index (/tools/_index.json) emitted at build time and served under the
// /tools/ Service-Worker bypass; we fetch it once on first open and filter
// client-side, so the page itself ships none of it.

export { filterTools } from './tools-index.js';

const INDEX_URL = '/tools/_index.json';

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
  // Close paths, belt-and-suspenders: the ✕ uses a native <form method="dialog">
  // (works with no JS), but we also wire a JS click as a fallback in case the
  // rendered markup is an older plain button. And close on a backdrop click.
  const closeBtn = dialog.querySelector('#tools-close');
  if (closeBtn) closeBtn.addEventListener('click', () => dialog.close());
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
