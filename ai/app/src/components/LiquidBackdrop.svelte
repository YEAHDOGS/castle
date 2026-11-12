<!-- The void: pure black with slow pools of liquid light drifting underneath the glass. Pure CSS gradients
     (no blur filters), animated with transforms only, so it costs next to nothing to render. -->
<div class="void" aria-hidden="true">
  <span class="blob a"></span>
  <span class="blob b"></span>
  <span class="blob c"></span>
  <span class="sheen"></span>
</div>

<style>
  .void {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background: radial-gradient(120% 80% at 50% 120%, #0a0306 0%, #020203 55%);
  }

  .blob {
    position: absolute;
    width: 70vmax;
    height: 70vmax;
    border-radius: 50%;
    opacity: 0.55;
    will-change: transform;
  }

  /* Deep red licorice glow */
  .a {
    top: -35vmax;
    right: -25vmax;
    background: radial-gradient(circle at center, rgb(255 51 68 / 22%) 0%, rgb(140 10 30 / 10%) 35%, transparent 65%);
    animation: drift-a 38s cubic-bezier(0.45, 0, 0.55, 1) infinite alternate;
  }

  /* Wine */
  .b {
    bottom: -40vmax;
    left: -30vmax;
    background: radial-gradient(circle at center, rgb(120 8 40 / 22%) 0%, rgb(60 4 24 / 10%) 40%, transparent 68%);
    animation: drift-b 46s cubic-bezier(0.45, 0, 0.55, 1) infinite alternate;
  }

  /* A cold violet glint, very faint */
  .c {
    top: 20%;
    left: 30%;
    width: 50vmax;
    height: 50vmax;
    background: radial-gradient(circle at center, rgb(110 60 255 / 8%) 0%, transparent 60%);
    animation: drift-c 54s cubic-bezier(0.45, 0, 0.55, 1) infinite alternate;
  }

  /* A slow glossy band sweeping across, like light on a wet surface */
  .sheen {
    position: absolute;
    inset: -50%;
    background: linear-gradient(115deg, transparent 42%, rgb(255 255 255 / 2.2%) 50%, transparent 58%);
    animation: sheen 22s linear infinite;
  }

  @keyframes drift-a {
    to {
      transform: translate(-18vmax, 14vmax) scale(1.15);
    }
  }

  @keyframes drift-b {
    to {
      transform: translate(22vmax, -16vmax) scale(1.2);
    }
  }

  @keyframes drift-c {
    to {
      transform: translate(-20vmax, 18vmax) scale(0.85);
    }
  }

  @keyframes sheen {
    from {
      transform: translateX(-30%);
    }
    to {
      transform: translateX(30%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .blob,
    .sheen {
      animation: none;
    }
  }
</style>
