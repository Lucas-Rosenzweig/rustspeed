<script lang="ts">
  import { fmtSpeed } from './api'
  import type { Measurements } from './speedtest'

  let { m, live = null }: { m: Partial<Measurements>; live?: keyof Measurements | null } = $props()

  const cards: { key: keyof Measurements; label: string; kind: 'speed' | 'ms'; color: string }[] = [
    { key: 'download_mbps', label: 'Descendant', kind: 'speed', color: 'var(--dl)' },
    { key: 'upload_mbps', label: 'Montant', kind: 'speed', color: 'var(--ul)' },
    { key: 'ping_ms', label: 'Ping', kind: 'ms', color: 'var(--ping)' },
    { key: 'jitter_ms', label: 'Gigue', kind: 'ms', color: 'var(--ping)' },
  ]

  function show(v: number | undefined, kind: 'speed' | 'ms') {
    if (v === undefined) return { value: '—', unit: kind === 'ms' ? 'ms' : 'Mbit/s' }
    return kind === 'ms' ? { value: v.toFixed(1), unit: 'ms' } : fmtSpeed(v)
  }
</script>

<div class="grid">
  {#each cards as c}
    {@const s = show(m[c.key], c.kind)}
    <div class="card" class:live={live === c.key} style:--accent={c.color}>
      <div class="label">{c.label}</div>
      <div class="value">{s.value}<span>{s.unit}</span></div>
    </div>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.75rem;
  }
  @media (max-width: 640px) {
    .grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-top: 3px solid var(--accent);
    border-radius: 12px;
    padding: 0.8rem 1rem;
    transition: box-shadow 0.2s;
  }
  .card.live {
    box-shadow: 0 0 0 1px var(--accent), 0 0 24px -8px var(--accent);
  }
  .label {
    color: var(--muted);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }
  .value {
    font-size: 1.6rem;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    margin-top: 0.2rem;
  }
  .value span {
    font-size: 0.8rem;
    color: var(--muted);
    font-weight: 400;
    margin-left: 0.3rem;
  }
</style>
