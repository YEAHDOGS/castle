<!-- RadarScope: one radar node's view as a motion-tracker sweep. The fan is what the
     sensor covers (120° and 6 m for an LD2450), drawn from the sensor at the bottom. A pulse
     runs out from the sensor over and over; each target is a blip that flares as the pulse
     passes it and fades until the next. Blips glide between readings rather than jump.

     With reduced motion there is no pulse and no glide: the fan is drawn once for each
     reading, every blip at full brightness. -->
<script>
  import { onMount } from 'svelte';
  import { reducedMotion } from '../../lib/motion.js';

  /** targets: [{ x, y, speed }] in millimetres; online: whether the node is reporting. */
  let { targets = [], online = false, range = 6000, fov = 120, label = 'Radar' } = $props();

  const PULSE_MS = 1600;
  const GLIDE_MS = 450;
  const RING_MM = 1000;
  const CYAN = '0, 215, 255';
  const ORANGE = '255, 106, 0';

  let canvas;
  let size = { w: 0, h: 0 };
  let from = []; // where the blips were drawn when the newest reading arrived
  let to = []; // the newest reading
  let changedAt = 0;
  let frame = 0;
  let still = false;

  const half = () => ((fov / 2) * Math.PI) / 180;

  /** Where each blip is at time `t`: on its way from the last reading to the newest. */
  function blipsAt(t) {
    const k = still ? 1 : Math.min(1, (t - changedAt) / GLIDE_MS);
    const ease = 1 - Math.pow(1 - k, 3);
    return to.map((b, i) => {
      const a = from[i];
      if (!a) return b;
      return { ...b, x: a.x + (b.x - a.x) * ease, y: a.y + (b.y - a.y) * ease };
    });
  }

  // A new reading: the blips glide from where they are now to where they were seen.
  $effect(() => {
    const next = (targets || []).map((t) => ({ x: t.x, y: t.y, speed: t.speed || 0 }));
    const now = performance.now();
    from = blipsAt(now);
    to = next;
    changedAt = now;
    if (still) draw(now);
  });

  $effect(() => {
    online;
    if (still) draw(performance.now());
  });

  function draw(t) {
    if (!canvas || !size.w) return;
    const ratio = window.devicePixelRatio || 1;
    const ctx = canvas.getContext('2d');
    const { w, h } = size;
    if (canvas.width !== Math.round(w * ratio)) canvas.width = Math.round(w * ratio);
    if (canvas.height !== Math.round(h * ratio)) canvas.height = Math.round(h * ratio);
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const a = half();
    const cx = w / 2;
    const oy = h - 10;
    const R = Math.max(10, Math.min(w / 2 / Math.sin(a) - 6, oy - 8));
    const scale = R / range;
    const start = -Math.PI / 2 - a;
    const end = -Math.PI / 2 + a;
    const tone = online ? CYAN : '255, 255, 255';

    // The fan
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(cx, oy);
    ctx.arc(cx, oy, R, start, end);
    ctx.closePath();
    const glow = ctx.createRadialGradient(cx, oy, 0, cx, oy, R);
    glow.addColorStop(0, `rgba(${tone}, ${online ? 0.1 : 0.04})`);
    glow.addColorStop(1, `rgba(${tone}, 0.015)`);
    ctx.fillStyle = glow;
    ctx.fill();
    ctx.clip();

    // Range rings, a metre apart, and spokes every 30°
    ctx.lineWidth = 1;
    ctx.strokeStyle = `rgba(${tone}, ${online ? 0.16 : 0.07})`;
    for (let r = RING_MM; r <= range; r += RING_MM) {
      ctx.beginPath();
      ctx.arc(cx, oy, r * scale, start, end);
      ctx.stroke();
    }
    for (let deg = -fov / 2; deg <= fov / 2; deg += 30) {
      const th = (deg * Math.PI) / 180;
      ctx.beginPath();
      ctx.moveTo(cx, oy);
      ctx.lineTo(cx + Math.sin(th) * R, oy - Math.cos(th) * R);
      ctx.stroke();
    }

    // The pulse
    const phase = still ? 0 : (t % PULSE_MS) / PULSE_MS;
    if (online && !still) {
      const pr = phase * R;
      const band = ctx.createRadialGradient(cx, oy, Math.max(0, pr - 26), cx, oy, pr);
      band.addColorStop(0, `rgba(${CYAN}, 0)`);
      band.addColorStop(1, `rgba(${CYAN}, ${0.22 * (1 - phase)})`);
      ctx.fillStyle = band;
      ctx.beginPath();
      ctx.moveTo(cx, oy);
      ctx.arc(cx, oy, pr, start, end);
      ctx.closePath();
      ctx.fill();
      ctx.strokeStyle = `rgba(${CYAN}, ${0.7 * (1 - phase)})`;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(cx, oy, pr, start, end);
      ctx.stroke();
    }

    // Blips: brightest just after the pulse passes them
    if (online) {
      for (const b of blipsAt(t)) {
        const d = Math.hypot(b.x, b.y);
        const since = still ? 0 : (((phase - Math.min(1, d / range)) % 1) + 1) % 1;
        const bright = still ? 1 : 1 - since * 0.75;
        const px = cx + b.x * scale;
        const py = oy - b.y * scale;
        const halo = ctx.createRadialGradient(px, py, 0, px, py, 16);
        halo.addColorStop(0, `rgba(${ORANGE}, ${0.55 * bright})`);
        halo.addColorStop(1, `rgba(${ORANGE}, 0)`);
        ctx.fillStyle = halo;
        ctx.beginPath();
        ctx.arc(px, py, 16, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = `rgba(${ORANGE}, ${0.35 + 0.65 * bright})`;
        ctx.beginPath();
        ctx.arc(px, py, 4.5, 0, Math.PI * 2);
        ctx.fill();
      }
    }
    ctx.restore();

    // Edge of the fan, the sensor, and the range marks
    ctx.strokeStyle = `rgba(${tone}, ${online ? 0.35 : 0.12})`;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(cx, oy);
    ctx.arc(cx, oy, R, start, end);
    ctx.closePath();
    ctx.stroke();
    ctx.fillStyle = `rgba(${tone}, ${online ? 0.9 : 0.3})`;
    ctx.fillRect(cx - 5, oy - 1, 10, 4);
    ctx.font = '600 9px ui-monospace, monospace';
    ctx.fillStyle = `rgba(${tone}, ${online ? 0.45 : 0.2})`;
    ctx.textAlign = 'center';
    for (let r = 2 * RING_MM; r <= range; r += 2 * RING_MM) {
      ctx.fillText(`${r / 1000}M`, cx, oy - r * scale + 11);
    }
  }

  function loop(t) {
    draw(t);
    frame = requestAnimationFrame(loop);
  }

  onMount(() => {
    still = reducedMotion();
    const watch = new ResizeObserver(([entry]) => {
      size = { w: entry.contentRect.width, h: entry.contentRect.height };
      if (still) draw(performance.now());
    });
    watch.observe(canvas);
    if (!still) frame = requestAnimationFrame(loop);
    return () => {
      watch.disconnect();
      cancelAnimationFrame(frame);
    };
  });
</script>

<canvas bind:this={canvas} class="block h-full w-full" aria-hidden="true"></canvas>
<span class="sr-only">{label}: {online ? `${targets.length} ${targets.length === 1 ? 'target' : 'targets'} seen` : 'not reporting'}</span>
