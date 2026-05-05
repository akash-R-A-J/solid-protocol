import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const repoRoot = resolve(new URL('../..', import.meta.url).pathname);
const manifest = JSON.parse(readFileSync(resolve(repoRoot, 'deployments/devnet.json'), 'utf8'));
const port = Number(process.env.PORT ?? '8787');

function json(res, status, body) {
  const payload = JSON.stringify(body, null, 2);
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
  });
  res.end(payload);
}

function notIndexed(res, resource) {
  json(res, 503, {
    ok: false,
    error: 'NOT_INDEXED',
    resource,
    message: 'This reference server exposes the SolID indexer contract and refuses to return fake proofs.',
  });
}

createServer((req, res) => {
  const url = new URL(req.url ?? '/', `http://${req.headers.host ?? 'localhost'}`);

  if (url.pathname === '/v1/health') {
    json(res, 200, {
      ok: true,
      network: manifest.network,
      slot: 0,
      indexed_slot: 0,
      lag_slots: 0,
      mode: 'contract-only',
    });
    return;
  }

  if (url.pathname === '/v1/manifest') {
    json(res, 200, manifest);
    return;
  }

  if (url.pathname === '/v1/schemas') {
    json(res, 200, manifest.schemas ?? []);
    return;
  }

  if (url.pathname === '/v1/issuers') {
    notIndexed(res, 'issuers');
    return;
  }

  if (url.pathname === '/v1/roots/current') {
    json(res, 200, {
      slot: 0,
      global_state_root: null,
      issuer_tree_root: manifest.trees?.issuer_tree?.current_root ?? null,
      schema_tree_roots: [],
    });
    return;
  }

  if (/^\/v1\/merkle-proof\//.test(url.pathname)) {
    notIndexed(res, 'merkle-proof');
    return;
  }

  if (/^\/v1\/issuers\/[^/]+\/proof$/.test(url.pathname)) {
    notIndexed(res, 'issuer-proof');
    return;
  }

  if (/^\/v1\/schemas\/[^/]+\/tree$/.test(url.pathname)) {
    const schemaHash = decodeURIComponent(url.pathname.split('/')[3] ?? '');
    const tree = (manifest.trees?.schema_trees ?? []).find((entry) => entry.schema_hash === schemaHash);
    if (!tree) {
      json(res, 404, { ok: false, error: 'SCHEMA_TREE_NOT_FOUND', schema_hash: schemaHash });
      return;
    }
    json(res, 200, tree);
    return;
  }

  json(res, 404, { ok: false, error: 'NOT_FOUND' });
}).listen(port, () => {
  console.log(`SolID indexer contract server listening on http://127.0.0.1:${port}`);
});
