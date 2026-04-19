// loader.js — registers the Service Worker, waits for activation, reloads to
// take control. Once the SW is active, subsequent navigations hit /b/ui/
// (served by the gizza-ai/ui block) instead of this shell.

async function boot() {
  if (!('serviceWorker' in navigator)) {
    document.getElementById('app').innerText = 'Service Worker not available.';
    return;
  }
  try {
    await navigator.serviceWorker.register('/sw.js', { type: 'module' });
    await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) {
      // First install: reload so the SW controls this page.
      location.reload();
      return;
    }
    // SW is ready and controls this page. Redirect to the actual UI.
    location.replace('/');
  } catch (e) {
    document.getElementById('app').innerText = `Service Worker registration failed: ${e}`;
  }
}

boot();
