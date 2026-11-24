// Client-side measurement engine for the rustspeed protocol.

export type Phase = 'idle' | 'ping' | 'download' | 'upload' | 'saving' | 'done' | 'error'

export interface Progress {
  phase: Phase
  /** Live value for the current phase (Mbps or ms). */
  live: number
  /** 0..1 progress of the current phase. */
  fraction: number
}

export interface Measurements {
  download_mbps: number
  upload_mbps: number
  ping_ms: number
  jitter_ms: number
}

export interface Options {
  pingSamples: number
  durationMs: number
  streams: number
}

export const defaultOptions: Options = { pingSamples: 20, durationMs: 10_000, streams: 6 }

type OnProgress = (p: Progress) => void

const API = '/api/v1'

export function median(xs: number[]): number {
  if (!xs.length) return 0
  const s = [...xs].sort((a, b) => a - b)
  const m = s.length >> 1
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2
}

/** Mean absolute difference between consecutive RTTs (simplified RFC 3550). */
export function jitter(xs: number[]): number {
  if (xs.length < 2) return 0
  let sum = 0
  for (let i = 1; i < xs.length; i++) sum += Math.abs(xs[i] - xs[i - 1])
  return sum / (xs.length - 1)
}

const mbps = (bytes: number, ms: number) => (ms > 0 ? (bytes * 8) / ms / 1000 : 0)

export async function measurePing(n: number, onProgress: OnProgress): Promise<{ ping: number; jitter: number }> {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const ws = new WebSocket(`${proto}//${location.host}${API}/measure/ws`)
  await new Promise<void>((resolve, reject) => {
    ws.onopen = () => resolve()
    ws.onerror = () => reject(new Error('WebSocket indisponible'))
  })
  const rtts: number[] = []
  try {
    // Warm-up frame, not counted.
    for (let i = -1; i < n; i++) {
      const t0 = performance.now()
      const rtt = await new Promise<number>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('ping: délai dépassé')), 5000)
        ws.onmessage = () => {
          clearTimeout(timer)
          resolve(performance.now() - t0)
        }
        ws.send(String(i))
      })
      if (i >= 0) {
        rtts.push(rtt)
        onProgress({ phase: 'ping', live: median(rtts), fraction: (i + 1) / n })
      }
      await new Promise((r) => setTimeout(r, 50))
    }
  } finally {
    ws.close()
  }
  return { ping: median(rtts), jitter: jitter(rtts) }
}

/**
 * Run `streams` parallel workers for `durationMs`, each looping `once()` which
 * reports transferred bytes through `add`. Throughput is computed over a
 * sliding window after a short warm-up to exclude TCP slow-start.
 */
async function saturate(
  phase: Phase,
  opts: Options,
  onProgress: OnProgress,
  once: (signal: AbortSignal, add: (n: number) => void) => Promise<void>,
): Promise<number> {
  const ctrl = new AbortController()
  let total = 0
  const add = (n: number) => (total += n)
  const start = performance.now()
  const warmup = Math.min(2000, opts.durationMs * 0.2)
  let baseBytes = 0
  let baseTime = 0
  const samples: { t: number; b: number }[] = []

  const tick = setInterval(() => {
    const now = performance.now()
    const t = now - start
    samples.push({ t: now, b: total })
    while (samples.length > 1 && now - samples[0].t > 1500) samples.shift()
    const first = samples[0]
    const live = mbps(total - first.b, now - first.t)
    if (!baseTime && t >= warmup) {
      baseBytes = total
      baseTime = now
    }
    onProgress({ phase, live, fraction: Math.min(1, t / opts.durationMs) })
    if (t >= opts.durationMs) ctrl.abort()
  }, 100)

  const worker = async () => {
    while (!ctrl.signal.aborted) {
      try {
        await once(ctrl.signal, add)
      } catch (e) {
        if (ctrl.signal.aborted) return
        throw e
      }
    }
  }
  try {
    await Promise.all(Array.from({ length: opts.streams }, worker))
  } finally {
    clearInterval(tick)
  }
  const end = performance.now()
  return baseTime ? mbps(total - baseBytes, end - baseTime) : mbps(total, end - start)
}

export function measureDownload(opts: Options, onProgress: OnProgress): Promise<number> {
  const chunk = 100 * 1024 * 1024
  return saturate('download', opts, onProgress, async (signal, add) => {
    const res = await fetch(`${API}/measure/download?bytes=${chunk}&r=${Math.random()}`, {
      signal,
      cache: 'no-store',
    })
    if (!res.ok || !res.body) throw new Error(`download: HTTP ${res.status}`)
    const reader = res.body.getReader()
    try {
      for (;;) {
        const { done, value } = await reader.read()
        if (done) break
        add(value.byteLength)
      }
    } catch (e) {
      if (!signal.aborted) throw e
    }
  })
}

let uploadPayload: Blob | null = null
function payload(): Blob {
  if (!uploadPayload) {
    // Random (incompressible) 8 MiB payload; crypto.getRandomValues is limited to 64 KiB per call.
    const buf = new Uint8Array(8 * 1024 * 1024)
    for (let i = 0; i < buf.length; i += 65536) crypto.getRandomValues(buf.subarray(i, i + 65536))
    uploadPayload = new Blob([buf])
  }
  return uploadPayload
}

export function measureUpload(opts: Options, onProgress: OnProgress): Promise<number> {
  const body = payload()
  return saturate('upload', opts, onProgress, (signal, add) =>
    new Promise<void>((resolve, reject) => {
      // XHR gives upload progress events; fetch does not.
      const xhr = new XMLHttpRequest()
      let last = 0
      xhr.upload.onprogress = (e) => {
        add(e.loaded - last)
        last = e.loaded
      }
      xhr.onload = () => {
        add(body.size - last)
        xhr.status === 200 ? resolve() : reject(new Error(`upload: HTTP ${xhr.status}`))
      }
      xhr.onerror = () => reject(new Error('upload: erreur réseau'))
      xhr.onabort = () => resolve()
      signal.addEventListener('abort', () => xhr.abort(), { once: true })
      xhr.open('POST', `${API}/measure/upload`)
      xhr.setRequestHeader('Content-Type', 'application/octet-stream')
      xhr.send(body)
    }),
  )
}

export async function runTest(onProgress: OnProgress, opts: Options = defaultOptions): Promise<Measurements> {
  const { ping, jitter: jit } = await measurePing(opts.pingSamples, onProgress)
  const download = await measureDownload(opts, onProgress)
  const upload = await measureUpload(opts, onProgress)
  return {
    download_mbps: round(download, 2),
    upload_mbps: round(upload, 2),
    ping_ms: round(ping, 2),
    jitter_ms: round(jit, 2),
  }
}

const round = (v: number, d: number) => Math.round(v * 10 ** d) / 10 ** d
