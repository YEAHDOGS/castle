<script>
  //  CORRECT: Import onMount from svelte, and everything else from three
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/examples/jsm/controls/OrbitControls";

  // State Management for UI Layer
  let activeIndex = 0;
  let infoPanelActive = false;

  const components = [
    {
      id: "gatehouse",
      title: "The Gatehouse (Edge Router)",
      color: 0x3b82f6, // Blue
      desc: "Edge routing node running your localized configuration. Manages traffic translation, interfaces with your physical ISP line, and hosts WireGuard endpoints.",
      specs: [
        "Raspberry Pi Edge Cluster Node",
        "Stateful connection packet mapping",
        "VPN routing tunnel interface layer",
      ],
    },
    {
      id: "armoury",
      title: "The Armoury (Firewall & Security Core)",
      color: "0xef4444", // Red
      desc: "Separated perimeter defense and strict cryptographic logging array. Monitors internal state logs and blocks anomalies natively.",
      specs: [
        "Pi-hole DNS blocking sinkholes",
        "Isolated VLAN filtering tables",
        "Blockchain-backed immutable system ledger",
      ],
    },
    {
      id: "keep",
      title: "The Keep (Storage Vault & Central Server)",
      color: 0x10b981, // Emerald Green
      desc: "The master local database. Hosts distributed Samba storage and schedules automated device backups.",
      specs: [
        "High-capacity redundant array layout",
        "Encrypted local filesystem volumes",
        "Automated smartphone backup orchestrator",
      ],
    },
    {
      id: "stables",
      title: "The Stables (Hardware Lab Compute Cluster)",
      color: 0xf59e0b, // Amber
      desc: "Isolated, sandboxed runtime environments dedicated to keeping your legacy silicon and Android nodes online 24/7.",
      specs: [
        "Proxmox VE virtualization cluster",
        "Headless Termux instances via Android ADB",
        "VLAN-caged malicious sandbox testbeds",
      ],
    },
    {
      id: "courtyard",
      title: "The Inner Courtyard & Corridors",
      color: 0x8b5cf6, // Purple
      desc: "The high-speed networking paths, physical walkways, and API bridges binding the Castle infrastructure modules together.",
      specs: [
        "Internal physical network switching mesh",
        "Meshtastic radio USB API controller daemon",
        "High-speed intra-node storage syncing links",
      ],
    },
  ];

  let container;
  let scene, camera, renderer, controls;
  let componentMeshes = {}; // Tracks 3D objects to highlight them on selection

  // Handle Arrow Key navigation through components
  function handleKeyDown(event) {
    if (event.key === "ArrowRight") {
      activeIndex = (activeIndex + 1) % components.length;
      updateSelection();
    } else if (event.key === "ArrowLeft") {
      activeIndex = (activeIndex - 1 + components.length) % components.length;
      updateSelection();
    }
  }

  function selectComponent(index) {
    activeIndex = index;
    updateSelection();
  }

  function updateSelection() {
    infoPanelActive = true;
    const activeComp = components[activeIndex];

    // Reset all meshes to their base colors, highlight the chosen one
    Object.keys(componentMeshes).forEach((id) => {
      const group = componentMeshes[id];
      const compData = components.find((c) => c.id === id);
      const targetColor =
        id === activeComp.id ? 0xffffff : Number(compData.color);

      group.traverse((child) => {
        if (child.isMesh && child.material) {
          child.material.color.setHex(targetColor);
        }
      });
    });
  }

  onMount(() => {
    // --- Scene Initialization ---
    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0a0f1d);
    scene.fog = new THREE.FogExp2(0x0a0f1d, 0.012);

    camera = new THREE.PerspectiveCamera(
      50,
      container.clientWidth / container.clientHeight,
      0.1,
      1000,
    );
    camera.position.set(45, 35, 45);

    renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(container.clientWidth, container.clientHeight);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    container.appendChild(renderer.domElement);

    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.maxPolarAngle = Math.PI / 2 - 0.05;
    controls.minDistance = 15;
    controls.maxDistance = 120;

    // --- Ambient & Dynamic Lights ---
    const ambientLight = new THREE.AmbientLight(0x1a233a, 1.8);
    scene.add(ambientLight);

    const sunLight = new THREE.DirectionalLight(0x7dd3fc, 1.2);
    sunLight.position.set(40, 60, 20);
    sunLight.castShadow = true;
    sunLight.shadow.mapSize.width = 2048;
    sunLight.shadow.mapSize.height = 2048;
    sunLight.shadow.camera.near = 0.5;
    sunLight.shadow.camera.far = 200;
    const d = 40;
    sunLight.shadow.camera.left = -d;
    sunLight.shadow.camera.right = d;
    sunLight.shadow.camera.top = d;
    sunLight.shadow.camera.bottom = -d;
    scene.add(sunLight);

    // --- Environment Setup (Sky, Ground, Trees) ---
    // Ground
    const groundGeo = new THREE.PlaneGeometry(300, 300);
    const groundMat = new THREE.MeshStandardMaterial({
      color: 0x070a14,
      roughness: 0.9,
    });
    const ground = new THREE.Mesh(groundGeo, groundMat);
    ground.rotation.x = -Math.PI / 2;
    ground.receiveShadow = true;
    scene.add(ground);

    // Procedural Forest (Low-Poly Trees)
    const treeGroup = new THREE.Group();
    const trunkGeo = new THREE.CylinderGeometry(0.3, 0.5, 3, 5);
    const trunkMat = new THREE.MeshStandardMaterial({
      color: 0x2c1d11,
      roughness: 0.9,
    });
    const leavesGeo = new THREE.ConeGeometry(2, 4, 5);
    const leavesMat = new THREE.MeshStandardMaterial({
      color: 0x0f2d1e,
      roughness: 0.8,
    });

    for (let i = 0; i < 45; i++) {
      const singleTree = new THREE.Group();
      const trunk = new THREE.Mesh(trunkGeo, trunkMat);
      trunk.position.y = 1.5;

      const leaves = new THREE.Mesh(leavesGeo, leavesMat);
      leaves.position.y = 4;

      singleTree.add(trunk, leaves);

      // Distribute randomly around the exterior edges of the map
      const angle = Math.random() * Math.PI * 2;
      const radius = 45 + Math.random() * 40;
      singleTree.position.set(
        Math.cos(angle) * radius,
        0,
        Math.sin(angle) * radius,
      );

      const scale = 0.6 + Math.random() * 0.8;
      singleTree.scale.set(scale, scale, scale);
      treeGroup.add(singleTree);
    }
    scene.add(treeGroup);

    // --- Initialize Functional Groups ---
    components.forEach((c) => {
      componentMeshes[c.id] = new THREE.Group();
      scene.add(componentMeshes[c.id]);
    });

    // Share common shared material presets mapped to colors
    const mat = (hex) =>
      new THREE.MeshStandardMaterial({
        color: Number(hex),
        roughness: 0.7,
        metalness: 0.2,
      });

    // 1. THE MOAT (A Real Deep Geometric Inset Trench)
    const moatMat = new THREE.MeshStandardMaterial({
      color: 0x0b2545,
      roughness: 0.1,
      metalness: 0.8,
      transparent: true,
      opacity: 0.85,
    });
    const moatGroup = componentMeshes["armoury"];

    const moatFloor = new THREE.Mesh(
      new THREE.RingGeometry(24, 32, 4, 1),
      new THREE.MeshStandardMaterial({ color: 0x04060b }),
    );
    moatFloor.rotation.x = -Math.PI / 2;
    moatFloor.position.y = 0.1;
    moatGroup.add(moatFloor);

    const waterGeo = new THREE.RingGeometry(24.2, 31.8, 32);
    const water = new THREE.Mesh(waterGeo, moatMat);
    water.rotation.x = -Math.PI / 2;
    water.position.y = 0.4;
    moatGroup.add(water);

    // 2. THE FIREWALL/SECURITY (The Armoury - Red Guard Towers & Shield Walls)
    const armMat = mat(components.find((c) => c.id === "armoury").color);
    const armTower1 = new THREE.Mesh(
      new THREE.CylinderGeometry(2, 2.5, 12, 8),
      armMat,
    );
    armTower1.position.set(24, 6, -24);
    const armTower2 = new THREE.Mesh(
      new THREE.CylinderGeometry(2, 2.5, 12, 8),
      armMat,
    );
    armTower2.position.set(-24, 6, -24);
    moatGroup.add(armTower1, armTower2);

    // 3. THE GATEHOUSE (Edge Router Entrance - Blue)
    const gateGroup = componentMeshes["gatehouse"];
    const gMat = mat(components.find((c) => c.id === "gatehouse").color);

    const leftTower = new THREE.Mesh(
      new THREE.CylinderGeometry(2.5, 2.5, 14, 12),
      gMat,
    );
    leftTower.position.set(-4, 7, 24);
    const rightTower = new THREE.Mesh(
      new THREE.CylinderGeometry(2.5, 2.5, 14, 12),
      gMat,
    );
    rightTower.position.set(4, 7, 24);

    const crossArch = new THREE.Mesh(new THREE.BoxGeometry(10.5, 4, 4), gMat);
    crossArch.position.set(0, 13, 24);

    // Drawbridge (Extending across the water gap)
    const drawbridge = new THREE.Mesh(
      new THREE.BoxGeometry(6, 0.5, 9),
      new THREE.MeshStandardMaterial({ color: 0x3d2314, roughness: 0.9 }),
    );
    drawbridge.position.set(0, 0.5, 28.5);

    gateGroup.add(leftTower, rightTower, crossArch, drawbridge);

    // 4. THE KEEP (Storage Core / Primary Server Infrastructure - Green)
    const keepGroup = componentMeshes["keep"];
    const kMat = mat(components.find((c) => c.id === "keep").color);

    const mainKeepBlock = new THREE.Mesh(
      new THREE.BoxGeometry(10, 22, 10),
      kMat,
    );
    mainKeepBlock.position.set(0, 11, -4);

    const roofCone = new THREE.Mesh(
      new THREE.ConeGeometry(7.5, 8, 4),
      new THREE.MeshStandardMaterial({ color: 0x1e293b, roughness: 0.5 }),
    );
    roofCone.position.set(0, 26, -4);
    roofCone.rotation.y = Math.PI / 4;

    keepGroup.add(mainKeepBlock, roofCone);

    // 5. THE STABLES (Device Compute Parking Labs - Amber)
    const stabGroup = componentMeshes["stables"];
    const sMat = mat(components.find((c) => c.id === "stables").color);

    const stableShed = new THREE.Mesh(new THREE.BoxGeometry(14, 6, 7), sMat);
    stableShed.position.set(-14, 3, -14);
    stableShed.rotation.y = Math.PI / 4;

    // Low-poly stalls inside the hardware block
    for (let k = 0; k < 3; k++) {
      const partition = new THREE.Mesh(new THREE.BoxGeometry(0.3, 4, 5), sMat);
      partition.position.set(-16 + k * 2.5, 2, -11);
      stabGroup.add(partition);
    }
    stabGroup.add(stableShed);

    // 6. THE COURTYARD AND CONNECTING HALLWAYS (Purple Network Paths)
    const courtGroup = componentMeshes["courtyard"];
    const cMat = mat(components.find((c) => c.id === "courtyard").color);

    // Grid Walls enclosing the courtyard matrix
    const wallLeft = new THREE.Mesh(new THREE.BoxGeometry(1, 8, 48), cMat);
    wallLeft.position.set(-24, 4, 0);
    const wallRight = new THREE.Mesh(new THREE.BoxGeometry(1, 8, 48), cMat);
    wallRight.position.set(24, 4, 0);
    const wallBack = new THREE.Mesh(new THREE.BoxGeometry(48, 8, 1), cMat);
    wallBack.position.set(0, 4, -24);

    // Elevated internal transit corridors linking structures
    const corridorLeft = new THREE.Mesh(new THREE.BoxGeometry(3, 4, 18), cMat);
    corridorLeft.position.set(-10, 2, 6);
    corridorLeft.rotation.y = Math.PI / 6;

    const corridorRight = new THREE.Mesh(new THREE.BoxGeometry(3, 4, 18), cMat);
    corridorRight.position.set(10, 2, 6);
    corridorRight.rotation.y = -Math.PI / 6;

    courtGroup.add(wallLeft, wallRight, wallBack, corridorLeft, corridorRight);

    // Ensure shadows apply seamlessly across all instantiated low-poly components
    scene.traverse((node) => {
      if (node.isMesh) {
        node.castShadow = true;
        node.receiveShadow = true;
      }
    });

    updateSelection();

    // --- Main Composition Frame Render Loop ---
    let animationFrameId;
    function animate() {
      animationFrameId = requestAnimationFrame(animate);
      controls.update();

      // Animate water layer subtly to show an active system boundary
      water.rotation.z += 0.002;

      renderer.render(scene, camera);
    }
    animate();

    // Clean up variables and canvas buffers on element tear-down
    return () => {
      cancelAnimationFrame(animationFrameId);
      window.removeEventListener("keydown", handleKeyDown);
      renderer.dispose();
    };
  });
</script>

<svelte:window on:keydown={handleKeyDown} />

<main
  class="relative w-screen h-screen bg-[#0a0f1d] overflow-hidden select-none text-[#e2e8f0]"
>
  <div bind:this={container} class="absolute inset-0 w-full h-full z-0"></div>

  <div
    class="absolute inset-0 pointer-events-none z-10 flex flex-col justify-between p-6 box-sizing-border"
  >
    <header
      class="pointer-events-auto max-w-lg bg-[#0f172a]/90 backdrop-blur-md border border-slate-800 p-5 rounded-xl shadow-2xl transition-all duration-200"
    >
      <h1 class="text-xs font-bold uppercase tracking-widest text-sky-400 mb-1">
        Infrastructure Control Node
      </h1>
      <h2 class="text-2xl font-black tracking-tight text-white mb-2">
        Castle Sovereign Grid
      </h2>
      <p class="text-xs text-slate-400 leading-relaxed">
        Use <kbd
          class="px-1.5 py-0.5 bg-slate-800 border border-slate-700 rounded text-slate-200 font-mono shadow text-[10px]"
          >←</kbd
        >
        and
        <kbd
          class="px-1.5 py-0.5 bg-slate-800 border border-slate-700 rounded text-slate-200 font-mono shadow text-[10px]"
          >→</kbd
        >
        arrows on your keyboard to navigate through network components, or click
        individual modules in the directory below.
      </p>
    </header>

    <div
      class="flex flex-1 items-center justify-between w-full my-4 overflow-hidden"
    >
      <nav
        class="pointer-events-auto flex flex-col gap-2 bg-[#0f172a]/70 p-4 rounded-xl border border-slate-800/60 backdrop-blur-sm shadow-xl"
      >
        <span
          class="text-[10px] uppercase font-bold text-slate-500 tracking-wider mb-1 px-2"
          >Layer Directory</span
        >
        {#each components as item, idx}
          <button
            on:click={() => selectComponent(idx)}
            class="flex items-center gap-3 px-4 py-2.5 rounded-lg text-left text-sm font-medium border transition-all duration-150 group {activeIndex ===
            idx
              ? 'bg-slate-800 border-slate-600 text-white shadow-lg'
              : 'bg-transparent border-transparent text-slate-400 hover:bg-slate-800/40 hover:text-slate-200'}"
          >
            <span
              class="w-2.5 h-2.5 rounded-full shadow-inner group-hover:scale-110 transition-transform duration-100"
              style="background-color: {item.id === 'armoury'
                ? '#ef4444'
                : '#' + item.color.toString(16).padStart(6, '0')}"
            ></span>
            {item.title.split(" (")[0]}
          </button>
        {/each}
      </nav>

      <section
        class="pointer-events-auto w-96 max-h-[70vh] flex flex-col bg-[#0f172a]/95 backdrop-blur-lg border border-slate-800 rounded-xl shadow-2xl overflow-y-auto transform transition-all duration-300 {infoPanelActive
          ? 'translate-x-0 opacity-100'
          : 'translate-x-12 opacity-0'}"
      >
        <div class="p-6">
          <div
            class="flex items-center justify-between border-b border-slate-800 pb-3 mb-4"
          >
            <h3 class="text-lg font-bold text-white tracking-tight">
              {components[activeIndex].title}
            </h3>
            <span
              class="text-[10px] font-mono font-bold bg-slate-800 px-2 py-0.5 rounded text-sky-400 uppercase tracking-wider border border-slate-700"
              >Audit Active</span
            >
          </div>

          <p class="text-sm text-slate-300 leading-relaxed mb-5">
            {components[activeIndex].desc}
          </p>

          <h4
            class="text-xs font-bold uppercase tracking-wider text-slate-400 mb-3"
          >
            Cryptographic Node Parameters
          </h4>
          <ul class="flex flex-col gap-2">
            {#each components[activeIndex].specs as spec}
              <li
                class="flex items-start gap-2.5 text-xs text-slate-400 bg-slate-900/50 p-2.5 rounded-lg border border-slate-800/80 font-mono"
              >
                <span class="text-sky-500 font-bold">▶</span>
                {spec}
              </li>
            {/each}
          </ul>
        </div>
      </section>
    </div>

    <footer
      class="w-full text-center text-[10px] font-mono uppercase tracking-widest text-slate-600"
    >
      Secure Network Handshake Protocol Stack V2.0.26 // Local Loopback
      Established
    </footer>
  </div>
</main>

<style lang="scss">
  :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
    outline: none;
  }

  // Handle fine-grained scrollbar configurations inside webkit environments
  section {
    &::-webkit-scrollbar {
      width: 4px;
    }
    &::-webkit-scrollbar-track {
      background: transparent;
    }
    &::-webkit-scrollbar-thumb {
      background: #1e293b;
      border-radius: 9999px;
      &:hover {
        background: #334155;
      }
    }
  }

  kbd {
    box-shadow:
      0 1px 0px rgba(255, 255, 255, 0.1),
      inset 0 1px 0px rgba(0, 0, 0, 0.8);
  }
</style>
