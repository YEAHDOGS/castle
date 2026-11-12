// App state: models, jobs and downloads from the server, plus what the user has selected.
import { api, subscribe } from './api.js'

export const KINDS = [
  { id: 'image', label: 'Image' },
  { id: 'video', label: 'Video' },
  { id: 'audio', label: 'Audio' },
]
export const VIEWS = [
  { id: 'create', label: 'Create' },
  { id: 'chat', label: 'Chat' },
  { id: 'library', label: 'Library' },
  { id: 'models', label: 'Models' },
]
const HEALTH_POLL_MS = 10_000
const FINISHED = ['done', 'error', 'cancelled']

class Store {
  kind = $state('image')
  /** @type {any} */ catalog = $state({ models: [], families: [], locations: [], unrecognized: [] })
  /** @type {any[]} */ jobs = $state([])
  /** @type {any[]} */ downloads = $state([])
  /** @type {any} */ health = $state(null)
  connected = $state(false)
  error = $state('')
  scanning = $state(false)
  selected = $state(/** @type {Record<string, string>} */ ({}))
  /** @type {any} */ preview = $state(null)

  models = $derived(this.catalog.models.filter((m) => m.kind === this.kind))
  ready = $derived(this.models.filter((m) => m.status === 'ready'))
  model = $derived(this.ready.find((m) => m.id === this.selected[this.kind]) || this.ready[0] || null)
  kindJobs = $derived(this.jobs.filter((j) => j.kind === this.kind))
  /** Every finished output, newest first, for the Library. */
  media = $derived(this.jobs.filter((j) => j.status === 'done').flatMap((j) => j.outputs.map((o) => ({ ...o, job: j, kind: o.mime.split('/')[0] }))))
  active = $derived(this.jobs.filter((j) => !FINISHED.includes(j.status)))
  /** Video jobs made from an image, keyed by that image's outputs path (their params.start_image), newest first. */
  animations = $derived(this.jobs.reduce((/** @type {Map<string, any[]>} */ m, j) => {
    const src = j.kind === 'video' && j.params?.start_image
    if (src) m.set(src, [...(m.get(src) || []), j])
    return m
  }, new Map()))

  #unsub = () => {}
  #timer = 0

  /** Direction of the last kind change (+1 later kind, -1 earlier), so pages slide the way the switch moved. */
  dir = $state(1)
  view = $state('create')
  /** Same as `dir`, for the main views (Create / Library / Models). */
  viewDir = $state(1)
  /** @type {any} */ llm = $state({ up: false, models: [], disk: {} })
  /** Image handed from the Library to Create → Video as a start frame. @type {{ file: string, url: string } | null} */
  startFrame = $state(null)
  /** Prompt text handed from the Library's Prompts to the Create composer. @type {string | null} */
  draftPrompt = $state(null)

  /** @param {string} kind */
  setKind(kind) {
    if (kind === this.kind) return
    this.dir = KINDS.findIndex((k) => k.id === kind) > KINDS.findIndex((k) => k.id === this.kind) ? 1 : -1
    this.kind = kind
  }

  /** @param {string} view */
  setView(view) {
    if (view === this.view) return
    this.viewDir = VIEWS.findIndex((v) => v.id === view) > VIEWS.findIndex((v) => v.id === this.view) ? 1 : -1
    this.view = view
  }

  async loadLlm() { this.llm = await api('/api/llm') }

  /** Delete a finished job and its files. @param {string} id */
  async removeJob(id) {
    await api(`/api/jobs/${id}?files=1`, { method: 'DELETE' })
    this.jobs = this.jobs.filter((j) => j.id !== id)
  }

  /** Delete scanned weight files by id; returns { deleted, freed, errors }. @param {string[]} ids */
  async deleteModelFiles(ids) {
    const r = await api('/api/models/delete', { method: 'POST', json: { ids } })
    await this.scan()
    return r
  }

  /** @param {string} name */
  async deleteLlm(name) {
    await api(`/api/llm/${encodeURIComponent(name)}`, { method: 'DELETE' })
    await this.loadLlm()
  }

  /** @param {string} name */
  async unloadLlm(name) {
    await api(`/api/llm/${encodeURIComponent(name)}/unload`, { method: 'POST' })
    await this.loadLlm()
  }

  connecting = $state(false)

  async connect() {
    this.#unsub()
    clearInterval(this.#timer)
    this.connecting = true
    try {
      // Load everything before flipping `connected`, so the first page renders once with real data.
      this.health = await api('/api/health')
      await Promise.all([this.scan(false), this.loadJobs(), this.loadDownloads()])
      this.connected = true
      this.error = ''
      this.#unsub = subscribe((j) => this.#onJob(j), (d) => this.#onDownload(d))
      this.#timer = setInterval(() => this.refreshHealth().catch(() => {}), HEALTH_POLL_MS)
      this.loadLlm().catch(() => {})
    } catch (e) {
      this.connected = false
      this.error = String(e.message || e)
    } finally {
      this.connecting = false
    }
  }

  async refreshHealth() {
    try {
      this.health = await api('/api/health')
      this.connected = true
    } catch (e) {
      this.connected = false
      throw e
    }
  }

  async scan(refresh = true) {
    this.scanning = true
    try { this.catalog = await api(`/api/models${refresh ? '?refresh=1' : ''}`) } finally { this.scanning = false }
  }

  async loadJobs() { this.jobs = await api('/api/jobs?limit=200') }
  async loadDownloads() { this.downloads = await api('/api/downloads') }

  /** @param {Record<string, any>} req */
  async generate(req) {
    const job = await api('/api/generate', { method: 'POST', json: { ...req, client: 'app' } })
    this.#onJob(job)
    return job
  }

  /** One-click image to video: the server picks the model, size and prompt. @param {string} file @param {string} [motion] */
  async animate(file, motion = '') {
    const job = await api('/api/animate', { method: 'POST', json: { image: file, motion, client: 'app' } })
    this.#onJob(job)
    return job
  }

  /** Open Create → Video with this image as the start frame, for full control over the settings. @param {{ file: string, url: string }} img */
  openInCreate(img) {
    this.startFrame = { file: img.file, url: img.url }
    this.setKind('video')
    this.setView('create')
  }

  /** @param {string} id */
  cancel(id) { return api(`/api/jobs/${id}`, { method: 'DELETE' }) }

  /** @param {string} target */
  async download(target) {
    const d = await api('/api/downloads', { method: 'POST', json: { target } })
    this.#onDownload(d)
  }

  /** Add a chat GGUF from Hugging Face to Ollama. @param {string} repo @param {string} file */
  async pullLlm(repo, file) {
    this.#onDownload(await api('/api/llm/pull', { method: 'POST', json: { repo, file } }))
  }

  #onJob(j) {
    const i = this.jobs.findIndex((x) => x.id === j.id)
    if (i >= 0) this.jobs[i] = j
    else this.jobs = [j, ...this.jobs]
  }

  #onDownload(d) {
    const i = this.downloads.findIndex((x) => x.id === d.id)
    if (i >= 0) this.downloads[i] = d
    else this.downloads = [d, ...this.downloads]
    if (d.status === 'done') (d.ollama ? this.loadLlm() : this.scan()).catch(() => {})
  }
}

export const store = new Store()
