import { mount } from 'svelte'
import App from './App.svelte'
import { setupShell } from './lib/shell.js'
import './app.css'

setupShell()

export default mount(App, { target: /** @type {HTMLElement} */ (document.getElementById('app')) })
