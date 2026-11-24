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

function shellResponse() {
  // ignoreSearch: クエリ付きナビゲーションでも同じシェルを返す。
  return caches.match('./index.html', { ignoreSearch: true })
    .then((hit) => hit || caches.match('./', { ignoreSearch: true }));
}

function isFresh(response) {
  // precache エントリ（タイムスタンプなし）は常に有効とみなす。
  const cachedAt = Number(response.headers.get(CACHED_AT_HEADER));
  if (!Number.isFinite(cachedAt) || cachedAt <= 0) return true;
  return Date.now() - cachedAt <= RUNTIME_TTL_MS;
}

async function putRuntime(request, response) {
  const cache = await caches.open(RUNTIME_CACHE);
  const keys = await cache.keys();
  // 件数上限（古い順に 1 件追い出して無限増殖を防ぐ）。
  if (keys.length >= RUNTIME_MAX_ENTRIES) await cache.delete(keys[0]);
  // 鮮度上限のためタイムスタンプを付与してから保存する。
  const headers = new Headers(response.headers);
  headers.set(CACHED_AT_HEADER, String(Date.now()));
  const stamped = new Response(response.clone().body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
  await cache.put(request, stamped);
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  event.respondWith(
    (async () => {
      if (request.mode === 'navigate') {
        // 静的シェルの更新を永続反映するため navigation は network-first。
        // オンラインでは最新 HTML を取得し、オフラインでは shell へフォールバック。
        try {
          const response = await fetch(request);
          if (response && response.ok) {
            const cache = await caches.open(CACHE_NAME);
            await cache.put('./index.html', response.clone());
          }
          return response;
        } catch (error) {
          return (await shellResponse()) || Response.error();
        }
      }

      // 静的アセットは cache-first（precache 済みならオフラインでも返す）。
      const cached = await caches.match(request);
      if (cached && isFresh(cached)) return cached;
      if (cached) await caches.delete(request); // 期限切れエントリは破棄

      try {
        const response = await fetch(request);
        if (response && response.ok && isCacheable(url)) {
          await putRuntime(request, response);
        }
        return response;
      } catch (error) {
        throw error;
      }
    })()
  );
});
