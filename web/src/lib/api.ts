import type { Measurements } from './speedtest'

export interface ClientInfo {
  ip: string
  hostname: string | null
  isp: string | null
  asn: number | null
}

export interface StoredResult extends Measurements {
  share_token: string
  created_at: string
  client_ip: string
  client_isp: string | null
  user_agent: string | null
  expires_at: string | null
}

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const body = await res.json().catch(() => ({}))
    throw new Error(body.error ?? `HTTP ${res.status}`)
  }
  return res.json()
}

export const getClientInfo = () => fetch('/api/v1/client-info').then((r) => json<ClientInfo>(r))

export const saveResult = (m: Measurements) =>
  fetch('/api/v1/results', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(m),
  }).then((r) => json<StoredResult>(r))

export const getResult = (token: string) =>
  fetch(`/api/v1/results/${encodeURIComponent(token)}`).then((r) => json<StoredResult>(r))

export const listResults = (limit = 10) => fetch(`/api/v1/results?limit=${limit}`).then((r) => json<StoredResult[]>(r))

export const shareUrl = (token: string) => `${location.origin}/r/${token}`

export function fmtSpeed(mbps: number): { value: string; unit: string } {
  if (mbps >= 1000) return { value: (mbps / 1000).toFixed(2), unit: 'Gbit/s' }
  return { value: mbps >= 100 ? mbps.toFixed(0) : mbps.toFixed(1), unit: 'Mbit/s' }
}

export const fmtDate = (iso: string) =>
  new Date(iso).toLocaleString('fr-FR', { dateStyle: 'medium', timeStyle: 'short' })
