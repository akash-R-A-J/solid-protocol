export function bytesToHex(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

export function normalizeHex32(value, name = 'hex') {
  const hex = String(value ?? '').trim().replace(/^0x/i, '').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(hex)) {
    throw new Error(`${name} must be a 32-byte hex string`);
  }
  return hex;
}

export function hexToBytes32(value, name = 'hex') {
  const hex = normalizeHex32(value, name);
  const bytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

export function readU64Le(bytes, offset) {
  let value = 0n;
  for (let i = 7; i >= 0; i--) {
    value = (value << 8n) | BigInt(bytes[offset + i]);
  }
  return value;
}

export function writeJsonResponse(res, status, body) {
  const payload = JSON.stringify(body, jsonReplacer, 2);
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
    'access-control-allow-origin': '*',
    'access-control-allow-methods': 'GET,POST,PATCH,OPTIONS',
    'access-control-allow-headers': 'authorization,content-type,accept',
  });
  res.end(payload);
}

function jsonReplacer(_key, value) {
  return typeof value === 'bigint' ? value.toString() : value;
}
