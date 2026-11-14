// Client-side media conversion: nothing is uploaded. Images go through the canvas encoder; audio and video
// through Mediabunny (WebCodecs + muxers), which runs the same in a browser and the Tauri shell's webviews.
// Every converter can aim for a target file size instead of a quality setting.
import {
  ALL_FORMATS, BlobSource, BufferTarget, Conversion, Input, Output, canEncodeAudio, canEncodeVideo,
  FlacOutputFormat, MkvOutputFormat, MovOutputFormat, Mp3OutputFormat, Mp4OutputFormat, OggOutputFormat,
  WavOutputFormat, WebMOutputFormat,
} from 'mediabunny'
import { registerMp3Encoder } from '@mediabunny/mp3-encoder'

/** Output formats per kind of source. Video sources can also be exported as audio (the soundtrack). */
export const IMAGE_FORMATS = [
  { id: 'png', mime: 'image/png', lossy: false },
  { id: 'jpg', mime: 'image/jpeg', lossy: true },
  { id: 'webp', mime: 'image/webp', lossy: true },
  { id: 'avif', mime: 'image/avif', lossy: true },
  { id: 'ico', mime: 'image/x-icon', lossy: false },
]
export const AUDIO_FORMATS = [
  { id: 'mp3', codec: 'mp3', make: () => new Mp3OutputFormat(), minKbps: 8, maxKbps: 320 },
  { id: 'm4a', codec: 'aac', make: () => new Mp4OutputFormat(), minKbps: 16, maxKbps: 320 },
  { id: 'ogg', codec: 'opus', make: () => new OggOutputFormat(), minKbps: 6, maxKbps: 510 },
  { id: 'webm', codec: 'opus', make: () => new WebMOutputFormat(), minKbps: 6, maxKbps: 510 },
  { id: 'flac', codec: 'flac', make: () => new FlacOutputFormat(), lossless: true },
  { id: 'wav', codec: 'pcm-s16', make: () => new WavOutputFormat(), lossless: true },
]
// Keep in step with VIDEO_FORMAT_IDS in formats.js.
export const VIDEO_FORMATS = [
  { id: 'mp4', video: 'avc', audio: 'aac', make: () => new Mp4OutputFormat() },
  { id: 'webm', video: 'vp9', audio: 'opus', make: () => new WebMOutputFormat() },
  { id: 'mov', video: 'avc', audio: 'aac', make: () => new MovOutputFormat() },
  { id: 'mkv', video: 'avc', audio: 'opus', make: () => new MkvOutputFormat() },
]

const ICO_MAX_PX = 256
const ICO_HEADER_BYTES = 22
const SIZE_SEARCH_STEPS = 8
const SIZE_HEADROOM = 0.96 // aim a little under the target: container overhead and encoder drift
const MIN_VIDEO_KBPS = 50

let mp3Ready = false
const ensureMp3 = () => { if (!mp3Ready) { registerMp3Encoder(); mp3Ready = true } }

/** Which output formats this device can actually encode (browsers differ). */
export async function supportedFormats(kind) {
  if (kind === 'image') {
    const probe = new OffscreenCanvas(2, 2)
    probe.getContext('2d')
    const out = []
    for (const f of IMAGE_FORMATS) {
      if (f.id === 'ico' || f.id === 'png') { out.push(f); continue }
      // Unsupported types silently fall back to PNG, so check what actually came out.
      const b = await probe.convertToBlob({ type: f.mime })
      if (b.type === f.mime) out.push(f)
    }
    return out
  }
  ensureMp3()
  const audio = []
  // WAV (PCM) and MP3 (bundled encoder) always work; the rest depend on the browser's WebCodecs encoders.
  for (const f of AUDIO_FORMATS) if (f.codec === 'pcm-s16' || f.codec === 'mp3' || (await canEncodeAudio(/** @type {any} */ (f.codec)))) audio.push(f)
  if (kind === 'audio') return audio
  const video = []
  for (const f of VIDEO_FORMATS) if (await canEncodeVideo(/** @type {any} */ (f.video))) video.push(f)
  return [...video, ...audio]
}

/** Basic facts about a media blob: duration, and size for images/video. */
export async function probe(blob, kind) {
  if (kind === 'image') {
    const bmp = await createImageBitmap(blob)
    const info = { width: bmp.width, height: bmp.height }
    bmp.close()
    return info
  }
  const input = new Input({ source: new BlobSource(blob), formats: ALL_FORMATS })
  try {
    const video = await input.getPrimaryVideoTrack()
    return {
      duration: await input.computeDuration(),
      width: video ? await video.getDisplayWidth() : undefined,
      height: video ? await video.getDisplayHeight() : undefined,
    }
  } finally {
    input.dispose?.()
  }
}

// ---------------------------------------------------------------- images

async function encodeImage(bmp, format, width, height, quality) {
  const fit = format.id === 'ico' ? Math.min(1, ICO_MAX_PX / Math.max(width, height)) : 1
  const canvas = new OffscreenCanvas(Math.max(1, Math.round(width * fit)), Math.max(1, Math.round(height * fit)))
  const ctx = /** @type {OffscreenCanvasRenderingContext2D} */ (canvas.getContext('2d'))
  ctx.imageSmoothingQuality = 'high'
  ctx.drawImage(bmp, 0, 0, canvas.width, canvas.height)
  if (format.id === 'ico') return pngToIco(await canvas.convertToBlob({ type: 'image/png' }), canvas.width, canvas.height)
  return canvas.convertToBlob({ type: format.mime, quality })
}

// A single-image ICO is a 22-byte header followed by a PNG.
function pngToIco(png, width, height) {
  const h = new DataView(new ArrayBuffer(ICO_HEADER_BYTES))
  h.setUint16(2, 1, true)
  h.setUint16(4, 1, true)
  h.setUint8(6, width % ICO_MAX_PX)
  h.setUint8(7, height % ICO_MAX_PX)
  h.setUint16(10, 1, true)
  h.setUint16(12, 32, true)
  h.setUint32(14, png.size, true)
  h.setUint32(18, ICO_HEADER_BYTES, true)
  return new Blob([h.buffer, png], { type: 'image/x-icon' })
}

/**
 * @param {Blob} blob
 * @param {{ format: string, width?: number, height?: number, quality?: number, targetBytes?: number }} o
 *   quality 0..1 for lossy formats. With targetBytes, quality (then size) is searched to fit under it.
 */
export async function convertImage(blob, o) {
  const format = IMAGE_FORMATS.find((f) => f.id === o.format) || IMAGE_FORMATS[0]
  const bmp = await createImageBitmap(blob)
  try {
    let w = o.width || bmp.width
    let h = o.height || bmp.height
    if (!o.targetBytes) return await encodeImage(bmp, format, w, h, o.quality ?? 0.9)
    const target = o.targetBytes * SIZE_HEADROOM
    let best = null
    // Lossy: binary-search quality at full size. If even the lowest quality is too big (or the format is
    // lossless), shrink the dimensions by the square root of the overshoot and try again.
    for (let round = 0; round < 6; round++) {
      if (format.lossy) {
        let lo = 0.02, hi = 1
        for (let i = 0; i < SIZE_SEARCH_STEPS; i++) {
          const q = (lo + hi) / 2
          const out = await encodeImage(bmp, format, w, h, q)
          if (out.size <= target) { best = out; lo = q } else hi = q
        }
        if (best) return best
        const floor = await encodeImage(bmp, format, w, h, 0.02)
        const shrink = Math.sqrt(target / floor.size)
        w = Math.max(16, Math.floor(w * shrink)); h = Math.max(16, Math.floor(h * shrink))
      } else {
        const out = await encodeImage(bmp, format, w, h)
        if (out.size <= target) return out
        const shrink = Math.sqrt(target / out.size) * 0.98
        w = Math.max(16, Math.floor(w * shrink)); h = Math.max(16, Math.floor(h * shrink))
      }
    }
    return encodeImage(bmp, format, w, h, format.lossy ? 0.02 : undefined)
  } finally {
    bmp.close()
  }
}

// ---------------------------------------------------------------- audio + video

// Encoders only take certain bitrates (Windows' AAC encoder: 96 kbps and up for stereo), so ask the encoder.
const STANDARD_KBPS = [6, 8, 12, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320]
const kbpsCache = new Map()

/** The highest bitrate <= wanted that this device encodes (or the lowest it does, if none is that low). 0 = none. */
async function encodableKbps(codec, wanted, channels, sampleRate) {
  if (codec === 'mp3') return STANDARD_KBPS.filter((k) => k >= 8 && k <= 320).reduce((best, k) => (k <= wanted ? k : best), 8)
  const key = `${codec}/${channels}/${sampleRate}`
  if (!kbpsCache.has(key)) {
    const ok = []
    for (const k of STANDARD_KBPS) {
      if (await canEncodeAudio(codec, { numberOfChannels: channels, sampleRate, bitrate: k * 1000 })) ok.push(k)
    }
    kbpsCache.set(key, ok)
  }
  const ok = kbpsCache.get(key)
  if (!ok.length) return 0
  return ok.filter((k) => k <= wanted).at(-1) ?? ok[0]
}

/**
 * Audio from an audio or video source, or video from a video source.
 * @param {Blob} blob
 * @param {{ format: string, kbps?: number, videoKbps?: number, width?: number, sampleRate?: number, channels?: number,
 *           targetBytes?: number, onProgress?: (fraction: number) => void, signal?: AbortSignal }} o
 * @returns {Promise<Blob>}
 */
export async function convertMedia(blob, o) {
  ensureMp3()
  const vf = VIDEO_FORMATS.find((f) => f.id === o.format)
  const af = AUDIO_FORMATS.find((f) => f.id === o.format)
  const fmt = vf || af
  if (!fmt) throw new Error(`unknown format ${o.format}`)

  const input = new Input({ source: new BlobSource(blob), formats: ALL_FORMATS })
  const target = new BufferTarget()
  const output = new Output({ format: fmt.make(), target })
  try {
    const audioCodec = af ? af.codec : vf.audio
    const lossless = !!af?.lossless
    const srcAudio = await input.getPrimaryAudioTrack()
    const channels = o.channels || (srcAudio ? await srcAudio.getNumberOfChannels() : 2)
    const sampleRate = o.sampleRate || (srcAudio ? await srcAudio.getSampleRate() : 48000)
    let kbps = o.kbps
    let videoKbps = o.videoKbps
    // With a target size, spread the byte budget over the duration: a small share to audio, the rest to video.
    const budgetKbps = o.targetBytes ? (o.targetBytes * 8 * SIZE_HEADROOM) / 1000 / Math.max(0.1, await input.computeDuration()) : 0
    if (budgetKbps) kbps = vf ? Math.min(kbps || 128, Math.floor(budgetKbps * 0.15)) : Math.floor(budgetKbps)
    if (srcAudio && kbps && !lossless) {
      kbps = await encodableKbps(audioCodec, kbps, channels, sampleRate)
      if (!kbps) throw new Error(`this device can't encode ${audioCodec.toUpperCase()} audio for ${o.format.toUpperCase()}`)
    }
    if (budgetKbps && vf) videoKbps = Math.max(MIN_VIDEO_KBPS, Math.floor(budgetKbps - (srcAudio ? kbps || 0 : 0)))
    const audio = {
      codec: audioCodec,
      ...(kbps && !lossless ? { bitrate: kbps * 1000 } : {}),
      ...(o.sampleRate ? { sampleRate: o.sampleRate } : {}),
      ...(o.channels ? { numberOfChannels: o.channels } : {}),
    }
    const conversion = await Conversion.init({
      input, output,
      video: vf
        ? { codec: /** @type {any} */ (vf.video), ...(o.width ? { width: o.width } : {}), ...(videoKbps ? { bitrate: videoKbps * 1000 } : {}) }
        : { discard: true },
      audio: /** @type {any} */ (audio),
    })
    // Never hand back a file that silently lost its soundtrack (or picture).
    const lost = conversion.discardedTracks.filter((t) => t.reason !== 'discarded_by_user' && t.reason !== 'max_track_count_of_type_reached')
    if (!conversion.isValid || lost.length) {
      const why = (lost.length ? lost : conversion.discardedTracks).map((t) => `${t.track.type}: ${t.reason.replace(/_/g, ' ')}`).join(', ')
      throw new Error(`can't convert to ${o.format} on this device (${why || 'no usable tracks'})`)
    }
    if (o.onProgress) conversion.onProgress = (p) => o.onProgress(p)
    o.signal?.addEventListener('abort', () => conversion.cancel())
    await conversion.execute()
    return new Blob([/** @type {ArrayBuffer} */ (target.buffer)], { type: await output.getMimeType() })
  } finally {
    input.dispose?.()
  }
}
