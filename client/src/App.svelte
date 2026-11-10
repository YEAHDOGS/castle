<!-- App: the shell. One section shows at a time; the wheel (top right) switches section and
     the Castle mark (top left) switches person. Each section is its own interface. -->
<script>
  import { onMount } from 'svelte';
  import { store } from './lib/store.svelte.js';
  import { shell } from './lib/shell.svelte.js';
  import StorageSection from './components/sections/StorageSection.svelte';
  import AiSection from './components/sections/AiSection.svelte';
  import ArcadeSection from './components/sections/ArcadeSection.svelte';
  import ClonesSection from './components/sections/ClonesSection.svelte';
  import NetworkSection from './components/sections/NetworkSection.svelte';
  import SettingsSection from './components/sections/SettingsSection.svelte';
  import WheelMenu from './components/WheelMenu.svelte';
  import UserMenu from './components/UserMenu.svelte';
  import ContextMenu from './components/ContextMenu.svelte';
  import PreviewModal from './components/PreviewModal.svelte';
  import ConnectModal from './components/ConnectModal.svelte';
  import Toasts from './components/Toasts.svelte';

  const SECTION_VIEWS = { storage: StorageSection, ai: AiSection, arcade: ArcadeSection, clones: ClonesSection, network: NetworkSection, settings: SettingsSection };
  const Section = $derived(SECTION_VIEWS[shell.section] || StorageSection);

  onMount(() => {
    store.init(); // no login wall: straight into the drive
  });
</script>

{#key shell.section}
  <Section />
{/key}

<WheelMenu />
<UserMenu />
<ContextMenu />
<PreviewModal />
<ConnectModal />
<Toasts />
