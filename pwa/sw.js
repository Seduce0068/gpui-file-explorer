// SPDX-License-Identifier: AGPL-3.0-or-later
//
// GFE Viewer の service worker。静的シェルと core wasm を precache し、
// オフライン起動（一覧 4 件の表示）を可能にする。
// 同一オリジンの GET だけを扱い、外部送信は行わない。
/* global self, caches, fetch, console, URL, Response */

const CACHE_NAME = 'gfe-viewer-v2';

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

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(CACHE_NAME)
      .then((cache) => cache.addAll(PRECACHE))
      .catch((error) => {
        // wasm-pack 再ビルドで snippet のハッシュが変わってもシェルは残す。
        console.warn('precaching failed:', error);
      })
      .then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key !== CACHE_NAME)
            .map((key) => caches.delete(key))
        )
      )
      .then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  event.respondWith(
    (async () => {
      // cache-first: オフラインでも precache 済みのファイルを返す。
      const cached = await caches.match(request);
      if (cached) return cached;
      if (request.mode === 'navigate') {
        const shell = await caches.match('./index.html');
        if (shell) return shell;
      }
      try {
        const response = await fetch(request);
        if (response && response.ok) {
          const cache = await caches.open(CACHE_NAME);
          await cache.put(request, response.clone());
        }
        return response;
      } catch (error) {
        if (request.mode === 'navigate') {
          const shell = await caches.match('./index.html');
          if (shell) return shell;
        }
        throw error;
      }
    })()
  );
});
