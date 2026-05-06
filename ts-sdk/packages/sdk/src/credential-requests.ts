export type CredentialRequestStatus =
  | 'requested'
  | 'reviewing'
  | 'approved'
  | 'rejected'
  | 'issued'
  | 'cancelled';

export interface CredentialRequestRecord {
  id: string;
  network: string;
  schemaHash: string;
  schemaName: string;
  schemaVersion: number;
  issuerAuthority: string;
  issuerAccount: string | null;
  holderChannelPublicKey: string;
  holderPublicKeyX: string;
  holderPublicKeyY: string;
  holderLabel: string | null;
  status: CredentialRequestStatus;
  requestedAt: string;
  updatedAt: string;
  notes: string | null;
  rejectionReason: string | null;
  envelopeJson: string | null;
  commitment: string | null;
  transactionSignature: string | null;
}

export interface CreateCredentialRequestInput {
  network?: string;
  schemaHash: string;
  schemaName: string;
  schemaVersion: number;
  issuerAuthority: string;
  issuerAccount?: string | null;
  holderChannelPublicKey: string;
  holderPublicKeyX: string;
  holderPublicKeyY: string;
  holderLabel?: string | null;
  notes?: string | null;
}

export interface UpdateCredentialRequestInput {
  status?: CredentialRequestStatus;
  notes?: string | null;
  rejectionReason?: string | null;
  envelopeJson?: string | null;
  commitment?: string | null;
  transactionSignature?: string | null;
}

export interface CredentialRequestFilters {
  issuerAuthority?: string;
  issuerAccount?: string;
  holderPublicKeyX?: string;
  schemaHash?: string;
  status?: CredentialRequestStatus;
}

export class SolidCredentialRequestClient {
  private readonly baseUrl: string;

  constructor(baseUrl: string) {
    const trimmed = baseUrl.trim().replace(/\/+$/, '');
    if (!trimmed) throw new Error('Credential request API URL is required.');
    this.baseUrl = trimmed;
  }

  async list(filters: CredentialRequestFilters = {}): Promise<CredentialRequestRecord[]> {
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(filters)) {
      if (value) params.set(key, value);
    }
    const query = params.toString();
    const body = await this.fetchJson<{ requests: CredentialRequestRecord[] }>(
      `/v1/credential-requests${query ? `?${query}` : ''}`,
      { method: 'GET' },
    );
    return body.requests;
  }

  async get(id: string): Promise<CredentialRequestRecord> {
    return this.fetchJson<CredentialRequestRecord>(
      `/v1/credential-requests/${encodeURIComponent(id)}`,
      { method: 'GET' },
    );
  }

  async create(input: CreateCredentialRequestInput): Promise<CredentialRequestRecord> {
    return this.fetchJson<CredentialRequestRecord>('/v1/credential-requests', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    });
  }

  async update(id: string, input: UpdateCredentialRequestInput): Promise<CredentialRequestRecord> {
    return this.fetchJson<CredentialRequestRecord>(
      `/v1/credential-requests/${encodeURIComponent(id)}`,
      {
        method: 'PATCH',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(input),
      },
    );
  }

  private async fetchJson<T>(path: string, init: RequestInit): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: {
        accept: 'application/json',
        ...(init.headers ?? {}),
      },
    });
    const body = await response.json().catch(() => null) as unknown;
    if (!response.ok) {
      const message = isRecord(body) && typeof body.message === 'string'
        ? body.message
        : `Credential request API returned HTTP ${response.status}`;
      throw new Error(message);
    }
    return body as T;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
