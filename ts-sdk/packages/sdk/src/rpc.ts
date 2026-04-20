/**
 * ResilientConnection
 *
 * v0.2 (2026-04 R-2 remediation):
 *   The previous implementation wrapped `@lightprotocol/stateless.js`.
 *   Since SolID now stores compressed credentials in SPL Account Compression
 *   trees (readable via any standard Solana RPC) and no longer depends on
 *   Photon, this wrapper provides priority-failover over a list of Solana
 *   `Connection` endpoints instead.
 *
 *   Design goals:
 *     - Zero dependency on Light Protocol / Photon (R-2).
 *     - Sticky active endpoint: successful calls keep pinning to the same
 *       RPC to preserve confirmation consistency across a session.
 *     - Idempotent, side-effect-free failover: each attempt creates a fresh
 *       `Connection` lazily and never mutates `this.activeIndex` until a
 *       call succeeds.
 */

import { Connection, ConnectionConfig } from '@solana/web3.js';

export class ResilientConnection {
  private readonly endpoints: readonly string[];
  private readonly config?: ConnectionConfig | string;
  private activeIndex: number = 0;
  private cached: Map<number, Connection> = new Map();

  constructor(endpoints: string[], commitmentOrConfig?: ConnectionConfig | string) {
    if (endpoints.length === 0) {
      throw new Error('ResilientConnection: at least one endpoint is required.');
    }
    this.endpoints = endpoints;
    this.config = commitmentOrConfig;
  }

  /** Get the current active `Connection`.  Callers can use this for one-shot
   *  RPC calls when they don't need failover (e.g., `getLatestBlockhash`
   *  during transaction assembly).  For any call that must survive a node
   *  outage, use {@link call} instead. */
  get connection(): Connection {
    return this.getOrCreate(this.activeIndex);
  }

  /** Execute an RPC call with automatic priority failover.  Attempts the
   *  current active endpoint first; on failure, walks the list in order.
   *  When a failover succeeds, the active index is updated so that
   *  subsequent calls pin to the newly healthy endpoint. */
  async call<T>(fn: (connection: Connection) => Promise<T>): Promise<T> {
    const total = this.endpoints.length;
    let lastError: unknown;

    for (let i = 0; i < total; i++) {
      const attemptIndex = (this.activeIndex + i) % total;
      const conn = this.getOrCreate(attemptIndex);
      try {
        const result = await fn(conn);
        if (i > 0) {
          this.activeIndex = attemptIndex;
        }
        return result;
      } catch (err) {
        lastError = err;
      }
    }

    throw new Error(
      `ResilientConnection: all ${total} endpoint(s) failed. Last error: ${
        lastError instanceof Error ? lastError.message : String(lastError)
      }`,
    );
  }

  private getOrCreate(index: number): Connection {
    const cached = this.cached.get(index);
    if (cached) return cached;
    const conn =
      typeof this.config === 'string' || this.config === undefined
        ? new Connection(this.endpoints[index], this.config)
        : new Connection(this.endpoints[index], this.config);
    this.cached.set(index, conn);
    return conn;
  }
}
