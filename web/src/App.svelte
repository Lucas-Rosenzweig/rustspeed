<script lang="ts">
  import { onMount } from 'svelte'
  import Gauge from './lib/Gauge.svelte'
  import Metrics from './lib/Metrics.svelte'
  import { runTest, type Measurements, type Phase } from './lib/speedtest'
  import {
    getClientInfo,
    getResult,
    listResults,
    saveResult,
    shareUrl,
    fmtDate,
    fmtSpeed,
    type ClientInfo,
    type StoredResult,
  } from './lib/api'

  // Tiny router: "/" = test page, "/r/<token>" = shared result.
  const shared = location.pathname.match(/^\/r\/([A-Za-z0-9_-]{22})\/?$/)?.[1] ?? null
  const unknownRoute = !shared && location.pathname !== '/'

  let phase = $state<Phase>('idle')
  let live = $state(0)
  let fraction = $state(0)
  let m = $state<Partial<Measurements>>({})
  let saved = $state<StoredResult | null>(null)
  let error = $state<string | null>(null)
  let client = $state<ClientInfo | null>(null)
  let history = $state<StoredResult[]>([])
  let sharedResult = $state<StoredResult | null>(null)
  let copied = $state(false)

  const phaseLabel: Record<Phase, string> = {
    idle: 'Prêt',
    ping: 'Latence',
    download: 'Descendant',
    upload: 'Montant',
    saving: 'Enregistrement',
    done: 'Terminé',
    error: 'Erreur',
  }
  const liveKey = $derived<keyof Measurements | null>(
    phase === 'ping' ? 'ping_ms' : phase === 'download' ? 'download_mbps' : phase === 'upload' ? 'upload_mbps' : null,
  )
  const gaugeColor = $derived(phase === 'upload' ? 'var(--ul)' : phase === 'ping' ? 'var(--ping)' : 'var(--dl)')
  const running = $derived(['ping', 'download', 'upload', 'saving'].includes(phase))

  onMount(async () => {
    if (shared) {
      try {
        sharedResult = await getResult(shared)
      } catch (e) {
        error = (e as Error).message
      }
      return
    }
    getClientInfo()
      .then((c) => (client = c))
      .catch(() => {})
    listResults(8)
      .then((h) => (history = h))
      .catch(() => {})
  })

  async function start() {
    error = null
    saved = null
    copied = false
    m = {}
    live = 0
    try {
      const result = await runTest((p) => {
        phase = p.phase
        live = p.live
        fraction = p.fraction
        if (p.phase === 'ping') m = { ...m, ping_ms: p.live }
        if (p.phase === 'download') m = { ...m, download_mbps: p.live }
        if (p.phase === 'upload') m = { ...m, upload_mbps: p.live }
      })
      m = result
      phase = 'saving'
      const s = await saveResult(result)
      saved = s
      phase = 'done'
      history = [s, ...history].slice(0, 8)
    } catch (e) {
      error = (e as Error).message
      phase = 'error'
    }
  }

  async function copy(url: string) {
    try {
      await navigator.clipboard.writeText(url)
    } catch {
      // Clipboard API needs a secure context; fall back to a hidden textarea.
      const ta = document.createElement('textarea')
      ta.value = url
      document.body.append(ta)
      ta.select()
      document.execCommand('copy')
      ta.remove()
    }
    copied = true
    setTimeout(() => (copied = false), 2000)
  }
</script>

<header>
  <a href="/" class="brand">
    <img src="/favicon.svg" alt="" width="28" height="28" />
    <span>rust<b>speed</b></span>
  </a>
  {#if client && !shared}
    <div class="client" title={client.hostname ?? ''}>
      <span class="ip">{client.ip}</span>
      {#if client.isp}<span class="isp">{client.isp}{client.asn ? ` · AS${client.asn}` : ''}</span>{/if}
    </div>
  {/if}
</header>

<main>
  {#if unknownRoute}
    <section class="panel center">
      <h1>Page introuvable</h1>
      <a class="btn" href="/">Lancer un test</a>
    </section>
  {:else if shared}
    <section class="panel">
      {#if sharedResult}
        <p class="eyebrow">Résultat partagé · {fmtDate(sharedResult.created_at)}</p>
        <Metrics m={sharedResult} />
        <dl class="meta">
          <dt>IP</dt>
          <dd>{sharedResult.client_ip}</dd>
          {#if sharedResult.client_isp}<dt>FAI</dt>
            <dd>{sharedResult.client_isp}</dd>{/if}
          {#if sharedResult.expires_at}<dt>Expire le</dt>
            <dd>{fmtDate(sharedResult.expires_at)}</dd>{/if}
        </dl>
        <a class="btn" href="/">Faire mon propre test</a>
      {:else if error}
        <h1>Résultat introuvable</h1>
        <p class="muted">{error}</p>
        <a class="btn" href="/">Lancer un test</a>
      {:else}
        <p class="muted">Chargement…</p>
      {/if}
    </section>
  {:else}
    <section class="panel hero">
      <Gauge
        value={running ? live : phase === 'done' ? (m.download_mbps ?? 0) : 0}
        unit={phase === 'ping' ? 'ms' : 'Mbit/s'}
        label={phaseLabel[phase]}
        color={gaugeColor}
        active={running}
      />
      {#if running}
        <div class="progress"><div style:width="{fraction * 100}%" style:background={gaugeColor}></div></div>
      {/if}
      <button class="btn big" onclick={start} disabled={running}>
        {running ? 'Test en cours…' : phase === 'idle' ? 'Démarrer' : 'Relancer'}
      </button>
      {#if error}<p class="error">⚠ {error}</p>{/if}
    </section>

    <Metrics {m} live={liveKey} />

    {#if saved}
      {@const url = shareUrl(saved.share_token)}
      <section class="panel share">
        <span class="muted">Lien de partage</span>
        <code>{url}</code>
        <button class="btn small" onclick={() => copy(url)}>{copied ? 'Copié ✓' : 'Copier'}</button>
      </section>
    {/if}

    {#if history.length}
      <section class="panel">
        <h2>Derniers tests</h2>
        <table>
          <thead>
            <tr><th>Date</th><th>Desc.</th><th>Mont.</th><th>Ping</th><th class="hide-sm">IP</th></tr>
          </thead>
          <tbody>
            {#each history as h (h.share_token)}
              {@const d = fmtSpeed(h.download_mbps)}
              {@const u = fmtSpeed(h.upload_mbps)}
              <tr>
                <td><a href="/r/{h.share_token}">{fmtDate(h.created_at)}</a></td>
                <td>{d.value} <small>{d.unit}</small></td>
                <td>{u.value} <small>{u.unit}</small></td>
                <td>{h.ping_ms.toFixed(1)} <small>ms</small></td>
                <td class="hide-sm muted">{h.client_ip}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}
  {/if}
</main>

<footer>
  <a href="https://github.com/Lucas-Rosenzweig/rustspeed" rel="noreferrer">rustspeed</a> · auto-hébergé, aucune donnée
  envoyée à un tiers
</footer>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem 0 1.5rem;
    flex-wrap: wrap;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    color: var(--text);
    text-decoration: none;
    font-size: 1.35rem;
    letter-spacing: -0.02em;
  }
  .brand b {
    color: var(--dl);
  }
  .client {
    text-align: right;
    font-size: 0.85rem;
    line-height: 1.3;
  }
  .client .ip {
    display: block;
    font-family: var(--mono);
  }
  .client .isp {
    color: var(--muted);
  }
  main {
    display: grid;
    gap: 1rem;
  }
  .panel {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 1.25rem;
  }
  .hero {
    display: grid;
    justify-items: center;
    gap: 1rem;
    padding: 1.5rem 1.25rem 1.75rem;
  }
  .center {
    text-align: center;
  }
  .progress {
    width: min(340px, 100%);
    height: 4px;
    background: var(--surface-2);
    border-radius: 2px;
    overflow: hidden;
  }
  .progress div {
    height: 100%;
    transition: width 0.1s linear;
  }
  .btn {
    display: inline-block;
    background: var(--dl);
    color: #111;
    border: 0;
    border-radius: 999px;
    padding: 0.6rem 1.4rem;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    text-decoration: none;
    margin-top: 0.5rem;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: progress;
  }
  .btn.big {
    font-size: 1.1rem;
    padding: 0.8rem 2.6rem;
  }
  .btn.small {
    padding: 0.35rem 1rem;
    margin: 0;
  }
  .error {
    color: #f87171;
    margin: 0;
  }
  .share {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .share code {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    font-family: var(--mono);
    background: var(--surface-2);
    padding: 0.4rem 0.6rem;
    border-radius: 8px;
  }
  h1,
  h2 {
    margin: 0 0 0.75rem;
  }
  h2 {
    font-size: 1rem;
    color: var(--muted);
    font-weight: 500;
  }
  .eyebrow {
    color: var(--muted);
    margin: 0 0 1rem;
  }
  .meta {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.3rem 1rem;
    margin: 1.25rem 0 0.5rem;
  }
  .meta dt {
    color: var(--muted);
  }
  .meta dd {
    margin: 0;
    font-family: var(--mono);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-variant-numeric: tabular-nums;
  }
  th,
  td {
    text-align: left;
    padding: 0.45rem 0.4rem;
    border-bottom: 1px solid var(--border);
  }
  th {
    color: var(--muted);
    font-weight: 500;
    font-size: 0.8rem;
  }
  tr:last-child td {
    border-bottom: 0;
  }
  small,
  .muted {
    color: var(--muted);
  }
  td a {
    color: var(--text);
  }
  @media (max-width: 640px) {
    .hide-sm {
      display: none;
    }
  }
  footer {
    text-align: center;
    color: var(--muted);
    font-size: 0.8rem;
    padding: 2rem 0 1rem;
  }
  footer a {
    color: inherit;
  }
</style>
