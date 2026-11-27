// Module-resolution hook for js/model-worker.test.js: the real model-worker.js
// imports ./transformers.min.js; the test runs it against a fake Transformers.js.
// Registered with module.register() (Node >= 20.6), so it works on the Node 20
// that CI runs as well as on newer releases.
const FAKE = new URL("./fake-transformers.js", import.meta.url).href;

export async function resolve(specifier, context, next) {
  if (specifier === "./transformers.min.js" && String(context.parentURL).endsWith("/model-worker.js")) {
    return { url: FAKE, shortCircuit: true };
  }
  return next(specifier, context);
}
