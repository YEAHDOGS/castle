<!-- App: the shell. One section shows at a time; the wheel (top right) switches section and
     the Castle mark (top left) switches person. Each section is its own interface.
     All of it works from the keyboard or a game controller too (lib/input.svelte.js). -->
<script>
  import { onMount } from 'svelte';
  import { scale } from 'svelte/transition';
  import { sectionIn } from './lib/motion.js';
  import { store } from './lib/store.svelte.js';
  import { shell } from './lib/shell.svelte.js';
  import HomeSection from './components/sections/HomeSection.svelte';
  import StorageSection from './components/sections/StorageSection.svelte';
  import AiSection from './components/sections/AiSection.svelte';
  import ArcadeSection from './components/sections/ArcadeSection.svelte';
  import TheaterSection from './components/sections/TheaterSection.svelte';
  import ClonesSection from './components/sections/ClonesSection.svelte';
  import NetworkSection from './components/sections/NetworkSection.svelte';
  import SecuritySection from './components/sections/SecuritySection.svelte';
  import SettingsSection from './components/sections/SettingsSection.svelte';
  import WheelMenu from './components/WheelMenu.svelte';
  import UserMenu from './components/UserMenu.svelte';
  import ContextMenu from './components/ContextMenu.svelte';
  import PreviewModal from './components/PreviewModal.svelte';
  import ConnectModal from './components/ConnectModal.svelte';
  import Toasts from './components/Toasts.svelte';
  import AskDialog from './components/AskDialog.svelte';
  import PadHints from './components/PadHints.svelte';

  const SECTION_VIEWS = { home: HomeSection, storage: StorageSection, ai: AiSection, arcade: ArcadeSection, theater: TheaterSection, clones: ClonesSection, network: NetworkSection, security: SecuritySection, settings: SettingsSection };
  const Section = $derived(SECTION_VIEWS[shell.section] || StorageSection);

  onMount(() => {
    store.init(); // no login wall: straight into the drive
  });
</script>

<!-- Changing section, or switching between live and demo, fades and settles the whole interface in, rather than cutting to it. -->
{#key `${shell.section}:${store.isDemo}`}
  <div in:scale={sectionIn()}>
    <Section />
  </div>
{/key}

<WheelMenu />
<UserMenu />
<ContextMenu />
<PreviewModal />
<ConnectModal />
<AskDialog />
<Toasts />
<PadHints />
