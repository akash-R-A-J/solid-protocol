import { createRpc, LightRpc } from "@lightprotocol/stateless.js";

/**
 * ResilientLightClient
 * 
 * A high-availability wrapper for Light Protocol's RPC.
 * Implements Priority Failover (Risk 1) to eliminate single points of failure.
 */
export class ResilientLightClient {
  private rpcs: string[];
  private activeRpcIndex: number = 0;
  private currentClient: LightRpc;

  constructor(endpoints: string[]) {
    if (endpoints.length === 0) {
      throw new Error("ResilientLightClient: At least one endpoint is required.");
    }
    this.rpcs = endpoints;
    this.currentClient = createRpc(this.rpcs[0]);
  }

  /**
   * Execute an RPC call with automatic failover.
   * If the primary indexer is down, it tries the secondaries in order.
   */
  async call<T>(fn: (client: LightRpc) => Promise<T>): Promise<T> {
    let lastError: any;

    for (let i = 0; i < this.rpcs.length; i++) {
        const attemptIndex = (this.activeRpcIndex + i) % this.rpcs.length;
        const client = i === 0 ? this.currentClient : createRpc(this.rpcs[attemptIndex]);

        try {
            const result = await fn(client);
            
            // If this attempt succeeded and was a failover, update the active index
            if (i > 0) {
                this.activeRpcIndex = attemptIndex;
                this.currentClient = client;
                console.warn(`SolID SDK: Failover successful to ${this.rpcs[attemptIndex]}`);
            }
            
            return result;
        } catch (err: any) {
            lastError = err;
            console.error(`SolID SDK: RPC attempt ${attemptIndex + 1} failed: ${err.message}`);
            // Continue to next endpoint...
        }
    }

    throw new Error(`SolID SDK: All RPC endpoints failed. Last error: ${lastError.message}`);
  }

  /**
   * Get the current active client for direct access.
   */
  get client(): LightRpc {
    return this.currentClient;
  }
}
