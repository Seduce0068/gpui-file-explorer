// SPDX-License-Identifier: AGPL-3.0-or-later
//
// GFE Viewer の service worker。静的シェルと core wasm を precache し、
// オフライン起動（一覧 4 件の表示）を可能にする。
// 同一オリジンの GET だけを扱い、外部送信は行わない。
/* global self, caches, fetch, console, URL, Response, Headers */

const CACHE_NAME = 'gfe-viewer-v2';
const CACHE_PREFIX = 'gfe-viewer-';
const RUNTIME_CACHE = 'gfe-viewer-runtime-v1';
const RUNTIME_MAX_ENTRIES = 32;
const RUNTIME_TTL_MS = 24 * 60 * 60 * 1000;
const CACHED_AT_HEADER = 'x-gfe-cached-at';

// 静的 5 ファイル（エントリ / HTML / マニフェスト / JS / CSS）と
// core API に必要な 3 ファイル（glue / wasm / snippet）。
const PRECACHE = [
  './',
  './index.html',
  './manifest.webmanifest',
  './main.js',
  './style.css',
  './pkg/gfe_core.js',
  './pkg/gfe_core_bg.wasm',
  './pkg/snippets/gpui-file-explorer-core-89c13f06e4cc3d8b/inline0.js',
];

// runtime cache に保存してよい静的アセットの拡張子（許可リスト）。
// 検索 API などの動的レスポンスを際限なく保存しないための境界。
const CACHEABLE_EXTENSIONS = [
  '.js', '.wasm', '.css', '.webmanifest', '.json',
  '.svg', '.png', '.jpg', '.jpeg', '.webp', '.ico',
];

function precacheAll() {
  return (async () => {
    const cache = await caches.open(CACHE_NAME);
    // addAll() の失敗を console.warn で飲み込むと、空キャッシュのまま
    // skipWaiting → activate が旧キャッシュを削除し、オフラインが黙って死ぬ。
    // 個別 add() で失敗アセットを特定し、1 件でも失敗したら再 throw して
    // install 自体を失敗させる（旧 SW / 旧キャッシュを維持する）。
    for (const asset of PRECACHE) {
      try {
        await cache.add(asset);
      } catch (error) {
        console.error(`precache failed: ${asset}`, error);
        // BI04: この throw は install 時の precacheAll 失敗を表す。
        // fetch ハンドラの network 失敗（shell フォールバック）とは別経路。
        throw error;
      }
    }
    // skipWaiting() は install で呼ばない。待機中の新 SW はクライアントの
    // SKIP_WAITING メッセージを受けてから activate する（BH02 / BH16）。
  })();
}

self.addEventListener('install', (event) => {
  event.waitUntil(precacheAll());
});

// 更新通知を受けたページから SKIP_WAITING が来たときだけ控制权を取る。
self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SKIP_WAITING') self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      // 自アプリの古いキャッシュ（gfe-viewer- で始まり現行名でないもの）だけ
      // 削除する。他 SW のキャッシュは残し、現行の precache / runtime も残す。
      const keep = new Set([CACHE_NAME, RUNTIME_CACHE]);
      await Promise.all(
        keys
          .filter((key) => key.startsWith(CACHE_PREFIX) && !keep.has(key))
          .map((key) => caches.delete(key))
      );
      await self.clients.claim();
    })()
  );
});

function isCacheable(url) {
  // クエリごとのキャッシュ増殖を防ぐ（?v=... 付き URL は保存しない）。
  if (url.search) return false;
  const path = url.pathname.toLowerCase();
  return CACHEABLE_EXTENSIONS.some((ext) => path.endsWith(ext));
}

async function shellResponse() {
  // BI07: caches.match() は全キャッシュ横断なので使わず、precache / runtime を
  // 明示キャッシュ単位で参照する。precache のシェルを優先し、期限切れ runtime
  // エントリが古いシェルとして返されないようにする。
  const precache = await caches.open(CACHE_NAME);
  const pinned = (await precache.match('./index.html', { ignoreSearch: true }))
    || (await precache.match('./', { ignoreSearch: true }));
  if (pinned) return pinned;

  try {
    const runtime = await caches.open(RUNTIME_CACHE);
    const dynamic = await runtime.match('./index.html', { ignoreSearch: true });
    if (dynamic) {
      if (isFresh(dynamic)) return dynamic;
      // BJ10: 期限切れ runtime シェルを残すとオフライン復帰時に古い HTML が
      // 返り続ける。best-effort で削除してから null を返す（precache は残す）。
      try {
        await runtime.delete('./index.html');
      } catch (error) {
        console.warn('stale runtime shell removal failed (best-effort)', error);
      }
    }
  } catch (error) {
    console.warn('runtime shell lookup failed (best-effort)', error);
  }
  return null;
}

async function cachedAsset(request) {
  // BI07: ここでも全キャッシュ横断の caches.match / caches.delete を避ける。
  // precache は削除せず、期限切れの runtime エントリだけを呼び出し元が破棄する。
  const precache = await caches.open(CACHE_NAME);
  const pinned = await precache.match(request);
  if (pinned) return { response: pinned, staleCache: null };
  const runtime = await caches.open(RUNTIME_CACHE);
  const dynamic = await runtime.match(request);
  return dynamic ? { response: dynamic, staleCache: runtime } : { response: null, staleCache: null };
}

function isFresh(response) {
  // precache エントリ（タイムスタンプなし）は常に有効とみなす。
  const cachedAt = Number(response.headers.get(CACHED_AT_HEADER));
  if (!Number.isFinite(cachedAt) || cachedAt <= 0) return true;
  return Date.now() - cachedAt <= RUNTIME_TTL_MS;
}

async function putRuntime(request, response) {
  const cache = await caches.open(RUNTIME_CACHE);
  // 鮮度上限のためタイムスタンプを付与してから保存する。
  const headers = new Headers(response.headers);
  headers.set(CACHED_AT_HEADER, String(Date.now()));
  const stamped = new Response(response.clone().body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
  // BJ11: put を先に行う。以前は put 前に古いエントリを削除していたため、
  // put が失敗すると既存キャッシュだけが失われていた。
  await cache.put(request, stamped);

  // 件数上限は put 成功後に適用する。今書き込んだキーは除外し、
  // 古い順に超過分だけを追い出して無限増殖を防ぐ。
  const keys = await cache.keys();
  if (keys.length > RUNTIME_MAX_ENTRIES) {
    const writtenUrl = typeof request === 'string'
      ? new URL(request, self.location.href).href
      : request.url;
    const evictable = keys.filter((key) => key.url !== writtenUrl);
    const excess = Math.min(keys.length - RUNTIME_MAX_ENTRIES, evictable.length);
    for (const key of evictable.slice(0, excess)) await cache.delete(key);
  }
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  // BK02: navigation の best-effort cache 書き込みを event.waitUntil に登録する。
  // respondWith が解決した後に waitUntil を呼ぶと InvalidStateError になるため、
  // respondWith より前に同期して deferred promise を作り、ここで登録する。
  // 書き込みを await しないので BJ09 の即時 return は維持され、SW は書き込み
  // 完了まで生き続ける（terminate による書き込み消失を防ぐ）。
  const navigationRequest = request.mode === 'navigate';
  let settleCacheWrite = null;
  if (navigationRequest) {
    const cacheWriteDone = new Promise((resolve) => { settleCacheWrite = resolve; });
    event.waitUntil(cacheWriteDone);
  }

  event.respondWith(
    (async () => {
      if (request.mode === 'navigate') {
        // 静的シェルの更新は新 SW の precache + reload で反映するため、
        // navigation は network-first（オフラインでは shell へフォールバック）。
        // BI01: cache.put の成否にかかわらず network の response を返す。
        // 以前は put 失敗を同じ catch で拾い、古いシェルへ差し替えていた。
        let response;
        try {
          response = await fetch(request);
        } catch (error) {
          settleCacheWrite();
          return (await shellResponse()) || Response.error();
        }
        // BI06: navigation 応答は現行 CACHE_NAME へ書かない。更新保留中の
        // precache を汚染し、ロールバック不能になるため。専用 runtime cache へ
        // best-effort で退避し、失敗しても返すレスポンスは変更しない。
        if (response && response.ok) {
          // BJ09: putRuntime を await しない。キャッシュ書き込みが完了するまで
          // navigation レスポンスを待たせないため、開始して即 return する。
          // BK02: 開始した promise を deferred で waitUntil へ受け渡す。
          settleCacheWrite(putRuntime('./index.html', response.clone()).catch((error) => {
            console.warn('navigation cache write failed (best-effort)', error);
          }));
        } else {
          settleCacheWrite();
        }
        return response;
      }

      // 静的アセットは cache-first（precache 済みならオフラインでも返す）。
      const { response: cached, staleCache } = await cachedAsset(request);
      if (cached && isFresh(cached)) return cached;
      if (staleCache) await staleCache.delete(request); // 期限切れ runtime のみ破棄

      const response = await fetch(request);
      if (response && response.ok && isCacheable(url)) {
        try {
          await putRuntime(request, response);
        } catch (error) {
          console.warn('runtime cache write failed (best-effort)', error);
        }
      }
      return response;
    })()
  );
});
