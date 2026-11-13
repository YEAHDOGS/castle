// Full screen for the whole app: the window itself in the desktop app, the page in a browser.
// F11 does the same in the desktop app (a browser already has its own F11).

import { isDesktopApp } from './box.js';

class FullScreen {
  on = $state(false);
  #listening = false;

  async #window() {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return getCurrentWindow();
  }

  /** Reads the real state, for when the window or the browser changed it without us. */
  async refresh() {
    if (isDesktopApp()) {
      try {
        this.on = await (await this.#window()).isFullscreen();
      } catch {
        /* an older shell without the permission: keep what we had */
      }
    } else {
      this.on = !!document.fullscreenElement;
    }
  }

  /** @returns {Promise<string | null>} an error to show, or null */
  async toggle() {
    try {
      if (isDesktopApp()) {
        const win = await this.#window();
        await win.setFullscreen(!(await win.isFullscreen()));
      } else if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch {
      await this.refresh();
      return 'Full screen is not available here';
    }
    await this.refresh();
    return null;
  }

  /** Called once by the shell: keeps `on` true to the window, and F11 in the desktop app. */
  listen() {
    if (this.#listening || typeof window === 'undefined') return;
    this.#listening = true;
    document.addEventListener('fullscreenchange', () => this.refresh());
    window.addEventListener('resize', () => this.refresh());
    if (isDesktopApp()) {
      window.addEventListener('keydown', (e) => {
        if (e.key !== 'F11' || e.defaultPrevented) return;
        e.preventDefault();
        this.toggle();
      });
    }
    this.refresh();
  }
}

export const fullScreen = new FullScreen();
