<!-- Sparkline: one tracked value over time. A 2px line on a light grid, the newest reading
     marked, the low and the high labelled, and a crosshair with the date and value under the
     pointer. One series, so no legend: the value's name is above it. -->
<script>
  /** points: [{ at, value }] oldest first. format(value) -> text. */
  let { points, label, format = (v) => String(v) } = $props();

  const H = 112;
  const PAD = { top: 14, right: 12, bottom: 18, left: 12 };
  const MARK = 4; // radius: an 8px marker

  let W = $state(600); // the figure's own width, so the drawing is never stretched
  let hover = $state(-1);
  let svg = $state(null);

  const geom = $derived.by(() => {
    const xs = points.map((p) => p.at);
    const ys = points.map((p) => p.value);
    const x0 = Math.min(...xs);
    const x1 = Math.max(...xs);
    let y0 = Math.min(...ys);
    let y1 = Math.max(...ys);
    if (y0 === y1) {
      y0 -= 1;
      y1 += 1;
    }
    const pad = (y1 - y0) * 0.12;
    y0 -= pad;
    y1 += pad;
    const x = (t) => PAD.left + (x1 === x0 ? (W - PAD.left - PAD.right) / 2 : ((t - x0) / (x1 - x0)) * (W - PAD.left - PAD.right));
    const y = (v) => PAD.top + (1 - (v - y0) / (y1 - y0)) * (H - PAD.top - PAD.bottom);
    const xy = points.map((p) => [x(p.at), y(p.value)]);
    const low = ys.indexOf(Math.min(...ys));
    const high = ys.indexOf(Math.max(...ys));
    return { xy, low, high, path: xy.map(([a, b], i) => `${i ? 'L' : 'M'}${a.toFixed(1)},${b.toFixed(1)}`).join(' ') };
  });
  const day = (ms) => new Date(ms).toLocaleDateString([], { month: 'short', day: 'numeric' });

  /** The reading nearest the pointer, along the time axis. */
  function move(e) {
    if (!svg) return;
    const box = svg.getBoundingClientRect();
    const px = ((e.clientX - box.left) / box.width) * W;
    let best = 0;
    geom.xy.forEach(([gx], i) => {
      if (Math.abs(gx - px) < Math.abs(geom.xy[best][0] - px)) best = i;
    });
    hover = best;
  }
</script>

<figure class="relative m-0" bind:clientWidth={W}>
  <svg
    bind:this={svg}
    viewBox="0 0 {W} {H}"
    class="block h-28 w-full overflow-visible text-live"
    role="img"
    aria-label="{label}: {points.length} readings, from {format(points[0].value)} on {day(points[0].at)} to {format(points.at(-1).value)} on {day(points.at(-1).at)}"
    onpointermove={move}
    onpointerleave={() => (hover = -1)}
  >
    {#each [0.25, 0.5, 0.75] as f (f)}
      <line x1={PAD.left} x2={W - PAD.right} y1={PAD.top + f * (H - PAD.top - PAD.bottom)} y2={PAD.top + f * (H - PAD.top - PAD.bottom)} class="stroke-white/[0.06]" stroke-width="1" />
    {/each}
    <path d={geom.path} fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke" />
    {#each [geom.low, geom.high] as i, k (k)}
      {#if points.length > 2 && i !== points.length - 1 && !(k === 1 && geom.high === geom.low)}
        <text x={geom.xy[i][0]} y={k === 0 ? geom.xy[i][1] + 14 : geom.xy[i][1] - 6} text-anchor="middle" class="fill-white/60 text-[11px]">{k === 0 ? 'low' : 'high'} {format(points[i].value)}</text>
      {/if}
    {/each}
    <circle cx={geom.xy.at(-1)[0]} cy={geom.xy.at(-1)[1]} r={MARK} fill="currentColor" class="stroke-panel-solid" stroke-width="2" />
    {#if hover >= 0}
      <line x1={geom.xy[hover][0]} x2={geom.xy[hover][0]} y1={PAD.top} y2={H - PAD.bottom} class="stroke-white/30" stroke-width="1" />
      <circle cx={geom.xy[hover][0]} cy={geom.xy[hover][1]} r={MARK} fill="currentColor" class="stroke-panel-solid" stroke-width="2" />
    {/if}
    <text x={PAD.left} y={H - 4} class="fill-white/40 text-[11px]">{day(points[0].at)}</text>
    <text x={W - PAD.right} y={H - 4} text-anchor="end" class="fill-white/40 text-[11px]">{day(points.at(-1).at)}</text>
  </svg>
  {#if hover >= 0}
    <figcaption
      class="pointer-events-none absolute top-0 -translate-x-1/2 border border-hairline bg-panel-solid px-2 py-1 text-[12px] whitespace-nowrap text-white"
      style="left: {(geom.xy[hover][0] / W) * 100}%"
    >
      {format(points[hover].value)} <span class="text-text-dim">· {day(points[hover].at)}</span>
    </figcaption>
  {/if}
</figure>
