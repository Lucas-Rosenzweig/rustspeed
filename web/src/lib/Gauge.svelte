<script lang="ts">
  // Semi-circular gauge on a log scale (1 Mbit/s → 10 Gbit/s).
  interface Props {
    value?: number
    unit?: string
    label?: string
    color?: string
    active?: boolean
  }
  let { value = 0, unit = 'Mbit/s', label = '', color = '#f97316', active = false }: Props = $props()

  const MIN = 1
  const MAX = 10_000
  const R = 90
  const CIRC = Math.PI * R

  let frac = $derived(
    value <= MIN ? 0 : Math.min(1, Math.log10(value / MIN) / Math.log10(MAX / MIN)),
  )
  let display = $derived(
    unit === 'ms' ? value.toFixed(1) : value >= 1000 ? (value / 1000).toFixed(2) : value >= 100 ? value.toFixed(0) : value.toFixed(1),
  )
  let displayUnit = $derived(unit === 'ms' ? 'ms' : value >= 1000 ? 'Gbit/s' : 'Mbit/s')
  const ticks = [1, 10, 100, 1000, 10000]
</script>

<div class="gauge" class:active>
  <svg viewBox="0 0 220 130" aria-hidden="true">
    <path d="M20 115 A90 90 0 0 1 200 115" class="track" />
    <path
      d="M20 115 A90 90 0 0 1 200 115"
      class="bar"
      style:stroke={color}
      style:stroke-dasharray={CIRC}
      style:stroke-dashoffset={unit === 'ms' ? CIRC : CIRC * (1 - frac)}
    />
    {#if unit !== 'ms'}
      {#each ticks as t, i}
        {@const a = Math.PI * (1 - i / (ticks.length - 1))}
        <text x={110 + 72 * Math.cos(a)} y={118 - 72 * Math.sin(a)} class="tick">
          {t >= 1000 ? `${t / 1000}G` : t}
        </text>
      {/each}
    {/if}
  </svg>
  <div class="readout">
    <span class="value">{display}</span>
    <span class="unit">{displayUnit}</span>
  </div>
  <div class="label">{label}</div>
</div>

<style>
  .gauge {
    position: relative;
    width: 100%;
    max-width: 340px;
    margin: 0 auto;
  }
  svg {
    width: 100%;
    display: block;
  }
  .track,
  .bar {
    fill: none;
    stroke-width: 12;
    stroke-linecap: round;
  }
  .track {
    stroke: var(--surface-2);
  }
  .bar {
    transition: stroke-dashoffset 0.25s ease-out;
    filter: drop-shadow(0 0 6px currentColor);
  }
  .tick {
    fill: var(--muted);
    font-size: 9px;
    text-anchor: middle;
    font-variant-numeric: tabular-nums;
  }
  .readout {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 22%;
    text-align: center;
    line-height: 1;
  }
  .value {
    font-size: clamp(2.2rem, 8vw, 3.4rem);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.03em;
  }
  .unit {
    display: block;
    color: var(--muted);
    font-size: 0.9rem;
    margin-top: 0.35rem;
  }
  .label {
    text-align: center;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    font-size: 0.75rem;
    margin-top: 0.25rem;
  }
  .active .label {
    color: var(--text);
  }
</style>
