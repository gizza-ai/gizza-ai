// bridge.js — gizza-ai JS functions exposed to Rust via wasm-bindgen.
//
// Two groups of exports:
//   1. Core services (db, storage, network, asset-loader) — copied from
//      solobase-web so BrowserDatabaseService etc. have a JS peer.
//   2. localLlmChatStream — gizza-specific bridge to the SW's WebLLM
//      handler (set on globalThis by sw.js).

// sql.js ESM wrapper is statically imported. Dynamic import() is forbidden in
// Service Workers, so this must be a static import. The ESM wrapper is created
// by the build (Makefile) from the UMD sql-wasm.js.
import initSqlJs from '/sql-wasm-esm.js';

// Module-level state
let _db = null;
const SQL_WASM_PATH = '/sql-wasm.wasm';
const DB_FILENAME = 'gizza.db';

// ─── Database (sql.js) ────────────────────────────────────────────────────────

/**
 * Load sql.js WASM, try to load existing DB from OPFS, create new if none exists.
 * Sets PRAGMA foreign_keys=ON.
 */
export async function dbInit() {
    const SQL = await initSqlJs({
        locateFile: () => SQL_WASM_PATH,
    });

    const root = await navigator.storage.getDirectory();
    let existingData = null;
    try {
        const fileHandle = await root.getFileHandle(DB_FILENAME);
        const file = await fileHandle.getFile();
        const buffer = await file.arrayBuffer();
        if (buffer.byteLength > 0) {
            existingData = new Uint8Array(buffer);
        }
    } catch (_e) {
        // File does not exist yet — start fresh
    }

    if (existingData) {
        _db = new SQL.Database(existingData);
    } else {
        _db = new SQL.Database();
    }

    _db.run('PRAGMA foreign_keys = ON;');
}

/**
 * Execute SQL that modifies data (INSERT/UPDATE/DELETE/DDL).
 * @param {string} sql
 * @param {string} paramsJson - JSON array of parameters
 * @returns {string} rows-modified count as string
 */
export function dbExecRaw(sql, paramsJson) {
    const params = JSON.parse(paramsJson);
    _db.run(sql, params);
    const rowsModified = _db.getRowsModified();
    return String(rowsModified);
}

/**
 * Execute a SELECT SQL query.
 * @param {string} sql
 * @param {string} paramsJson - JSON array of parameters
 * @returns {string} JSON array of row objects
 */
export function dbQueryRaw(sql, paramsJson) {
    const params = JSON.parse(paramsJson);
    const results = _db.exec(sql, params);
    if (!results || results.length === 0) {
        return '[]';
    }
    const { columns, values } = results[0];
    const rows = values.map((row) => {
        const obj = {};
        columns.forEach((col, i) => {
            obj[col] = row[i];
        });
        return obj;
    });
    return JSON.stringify(rows);
}

/**
 * Export the sql.js DB to a Uint8Array and write it to OPFS.
 */
export async function dbFlush() {
    if (!_db) return;
    const data = _db.export();
    const root = await navigator.storage.getDirectory();
    const fileHandle = await root.getFileHandle(DB_FILENAME, { create: true });
    const writable = await fileHandle.createWritable();
    await writable.write(data);
    await writable.close();
}

// ─── Storage (OPFS) ──────────────────────────────────────────────────────────

const STORAGE_DIR = 'storage';

async function getStorageRoot() {
    const root = await navigator.storage.getDirectory();
    return root.getDirectoryHandle(STORAGE_DIR, { create: true });
}

async function getFolderHandle(storageRoot, folder, create = false) {
    return storageRoot.getDirectoryHandle(folder, { create });
}

export async function storagePut(folder, key, data, contentType) {
    const storageRoot = await getStorageRoot();
    const folderHandle = await getFolderHandle(storageRoot, folder, true);

    const fileHandle = await folderHandle.getFileHandle(key, { create: true });
    const writable = await fileHandle.createWritable();
    await writable.write(data);
    await writable.close();

    const meta = { content_type: contentType, size: data.length };
    const metaHandle = await folderHandle.getFileHandle(`${key}.__meta__`, { create: true });
    const metaWritable = await metaHandle.createWritable();
    await metaWritable.write(JSON.stringify(meta));
    await metaWritable.close();
}

export async function storageGet(folder, key) {
    const storageRoot = await getStorageRoot();
    const folderHandle = await getFolderHandle(storageRoot, folder, false);

    const fileHandle = await folderHandle.getFileHandle(key);
    const file = await fileHandle.getFile();
    const buffer = await file.arrayBuffer();
    const dataArray = Array.from(new Uint8Array(buffer));

    let meta = { content_type: 'application/octet-stream', size: dataArray.length };
    try {
        const metaHandle = await folderHandle.getFileHandle(`${key}.__meta__`);
        const metaFile = await metaHandle.getFile();
        const metaText = await metaFile.text();
        meta = JSON.parse(metaText);
    } catch (_e) {
        // No metadata file — use defaults
    }

    return JSON.stringify({ data: dataArray, meta });
}

export async function storageDelete(folder, key) {
    const storageRoot = await getStorageRoot();
    const folderHandle = await getFolderHandle(storageRoot, folder, false);
    await folderHandle.removeEntry(key);
    try {
        await folderHandle.removeEntry(`${key}.__meta__`);
    } catch (_e) {
        // Metadata may not exist
    }
}

export async function storageList(folder, prefix, limit, offset) {
    const storageRoot = await getStorageRoot();
    const folderHandle = await getFolderHandle(storageRoot, folder, false);

    const keys = [];
    for await (const [name] of folderHandle.entries()) {
        if (name.endsWith('.__meta__')) continue;
        if (!prefix || name.startsWith(prefix)) {
            keys.push(name);
        }
    }

    keys.sort();
    const page = keys.slice(offset, limit > 0 ? offset + limit : undefined);
    return JSON.stringify(page);
}

export async function storageCreateFolder(name) {
    const storageRoot = await getStorageRoot();
    await storageRoot.getDirectoryHandle(name, { create: true });
}

export async function storageDeleteFolder(name) {
    const storageRoot = await getStorageRoot();
    await storageRoot.removeEntry(name, { recursive: true });
}

export async function storageListFolders() {
    const storageRoot = await getStorageRoot();
    const folders = [];
    for await (const [name, handle] of storageRoot.entries()) {
        if (handle.kind === 'directory') {
            folders.push(name);
        }
    }
    folders.sort();
    return JSON.stringify(folders);
}

// ─── Asset loader bridge (SW → main thread) ─────────────────────────────────
//
// The Rust SwAssetLoader (running inside this SW) calls loadAsset() to ask the
// main thread to fetch + verify + init an external asset (WebLLM model files,
// etc). We postMessage a 'load-asset-request' to the first window client, then
// wait for the matching 'load-asset-response' to arrive at sw.js's message
// listener. sw.js routes the response back here via
// globalThis.__gizzaCompleteAssetLoad.

const _pendingAssetLoads = new Map(); // correlationId -> resolve fn

export async function loadAsset(assetId, manifestJson) {
    const manifest = JSON.parse(manifestJson);

    const clients = await self.clients.matchAll({ type: 'window', includeUncontrolled: false });
    if (clients.length === 0) {
        return { status: 'failed', error: 'no active page — open the app in a tab to load assets' };
    }

    const correlationId = `asset-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
    const replyPromise = new Promise((resolve) => {
        _pendingAssetLoads.set(correlationId, resolve);
        setTimeout(() => {
            if (_pendingAssetLoads.has(correlationId)) {
                _pendingAssetLoads.delete(correlationId);
                resolve({ status: 'failed', error: 'load-asset timed out' });
            }
        }, 120_000);
    });

    clients[0].postMessage({
        type: 'load-asset-request',
        id: correlationId,
        manifest,
    });

    return await replyPromise;
}

export function _completeAssetLoad(correlationId, reply) {
    const resolve = _pendingAssetLoads.get(correlationId);
    if (resolve) {
        _pendingAssetLoads.delete(correlationId);
        resolve(reply);
    }
}

globalThis.__gizzaCompleteAssetLoad = _completeAssetLoad;

// ─── Network (fetch) ─────────────────────────────────────────────────────────

export async function httpFetch(method, url, headersJson, body) {
    const headersObj = JSON.parse(headersJson);
    const init = {
        method,
        headers: headersObj,
    };

    if (body && body.length > 0) {
        init.body = body;
    }

    const response = await fetch(url, init);

    const responseHeaders = {};
    response.headers.forEach((value, name) => {
        responseHeaders[name] = value;
    });

    const responseBuffer = await response.arrayBuffer();
    const responseBody = Array.from(new Uint8Array(responseBuffer));

    return JSON.stringify({
        status: response.status,
        headers: responseHeaders,
        body: responseBody,
    });
}

// ─── gizza-ai: local-llm chat_stream bridge ──────────────────────────────────

/**
 * Invoke the SW's chat_stream handler directly, collecting the SSE response
 * into a single byte array that the Rust agent block can parse.
 *
 * Returns a Uint8Array containing the full SSE text, or throws on error.
 *
 * @param {string} bodyJson - JSON-encoded string like {"messages":[...],"tools":[...]}
 * @returns {Promise<Uint8Array>}
 */
export async function localLlmChatStream(bodyJson) {
    if (typeof globalThis.__gizzaHandleLocalLlm !== 'function') {
        throw new Error('__gizzaHandleLocalLlm not available — sw.js must set it');
    }
    const request = new Request('/b/local-llm/api/chat_stream', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: bodyJson,
    });
    const response = await globalThis.__gizzaHandleLocalLlm(request);
    if (!response.ok) {
        const text = await response.text();
        throw new Error(`chat_stream HTTP ${response.status}: ${text}`);
    }
    const buf = await response.arrayBuffer();
    return new Uint8Array(buf);
}
