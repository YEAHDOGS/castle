// Chat state: conversations, the open one, and the streaming reply. Talks to /api/chats on the engine, which
// relays Ollama. Replies stream in as NDJSON events (thinking / token / done / error).
import { api, url, settings } from './api.js'

class Chat {
  /** @type {any[]} */ list = $state([])
  /** @type {any} */ current = $state(null)
  streaming = $state(false)
  /** Partial reply while streaming. */ draft = $state({ content: '', thinking: '' })
  error = $state('')
  model = $state('')
  think = $state(false)
  /** @type {AbortController | null} */ #abort = null

  async load() { this.list = await api('/api/chats') }

  /** @param {string} id */
  async open(id) {
    this.error = ''
    this.current = await api(`/api/chats/${id}`)
    if (this.current.model) this.model = this.current.model
  }

  /** @param {{ characterId?: string }} [o] */
  async create(o = {}) {
    this.error = ''
    this.current = await api('/api/chats', { method: 'POST', json: { model: this.model || undefined, characterId: o.characterId } })
    this.list = [this.current, ...this.list]
    return this.current
  }

  /** @param {string} id */
  async remove(id) {
    await api(`/api/chats/${id}`, { method: 'DELETE' })
    this.list = this.list.filter((c) => c.id !== id)
    if (this.current?.id === id) this.current = null
  }

  stop() { this.#abort?.abort() }

  /** Send a message and stream the reply into `draft`, then append both to the open chat. @param {string} text */
  async send(text) {
    if (this.streaming || !text.trim()) return
    if (!this.current) await this.create()
    const chat = this.current
    this.error = ''
    chat.messages = [...chat.messages, { id: `local-${Date.now()}`, role: 'user', content: text, created: Date.now() }]
    this.streaming = true
    this.draft = { content: '', thinking: '' }
    this.#abort = new AbortController()
    try {
      const headers = { 'content-type': 'application/json', ...(settings.key ? { authorization: `Bearer ${settings.key}` } : {}) }
      const res = await fetch(url(`/api/chats/${chat.id}/messages`), {
        method: 'POST', headers, signal: this.#abort.signal,
        body: JSON.stringify({ content: text, model: this.model || undefined, think: this.think }),
      })
      if (!res.ok || !res.body) throw new Error((await res.json().catch(() => ({}))).error || `HTTP ${res.status}`)
      const reader = res.body.pipeThrough(new TextDecoderStream()).getReader()
      let buf = ''
      for (;;) {
        const { value, done } = await reader.read()
        if (done) break
        buf += value
        let nl
        while ((nl = buf.indexOf('\n')) >= 0) {
          const line = buf.slice(0, nl).trim()
          buf = buf.slice(nl + 1)
          if (line) this.#onEvent(chat, JSON.parse(line))
        }
      }
    } catch (e) {
      if (e?.name !== 'AbortError') this.error = String(e.message || e)
      else if (this.draft.content) chat.messages = [...chat.messages, { id: `local-${Date.now()}`, role: 'assistant', content: this.draft.content + '\n\n_(stopped)_', thinking: this.draft.thinking }]
    } finally {
      this.streaming = false
      this.#abort = null
      this.load().catch(() => {})
    }
  }

  /** @param {any} chat @param {any} ev */
  #onEvent(chat, ev) {
    if (ev.type === 'token') this.draft.content += ev.text
    else if (ev.type === 'thinking') this.draft.thinking += ev.text
    else if (ev.type === 'error') this.error = ev.error
    else if (ev.type === 'done') chat.messages = [...chat.messages, ev.message]
  }
}

export const chat = new Chat()
