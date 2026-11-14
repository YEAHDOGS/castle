import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { installInput } from './lib/input.svelte.js'

// The keyboard and controllers, before anything mounts, so the app's own navigation listens
// on the window after every section's handlers (see lib/input.svelte.js).
installInput()

const app = mount(App, {
  target: document.getElementById('app'),
})

export default app
