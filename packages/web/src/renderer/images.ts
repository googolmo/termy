import type { GraphicsPlacement, TermyCore } from '@termysh/core'
import type { CellMetrics } from './metrics.ts'

/** Kitty graphics placements with z below this paint under cell backgrounds. */
const BELOW_TEXT = 0

/**
 * Paints kitty graphics into two canvases: one under the text layer (negative
 * z-index) and one over it. Decoded images are cached per image generation.
 */
export class ImageLayer {
  readonly under: HTMLCanvasElement
  readonly over: HTMLCanvasElement
  #bitmaps = new Map<string, ImageBitmap | null>()
  #placements: GraphicsPlacement[] = []
  #revision = -1
  #viewKey = ''
  #onReady: () => void

  constructor(onReady: () => void) {
    this.under = layerCanvas()
    this.over = layerCanvas()
    this.#onReady = onReady
  }

  get hasUnderImages(): boolean {
    return this.#placements.some((placement) => placement.zIndex < BELOW_TEXT)
  }

  setSize(width: number, height: number, cssWidth: number, cssHeight: number): void {
    for (const canvas of [this.under, this.over]) {
      canvas.width = width
      canvas.height = height
      canvas.style.width = `${cssWidth}px`
      canvas.style.height = `${cssHeight}px`
    }
    this.#viewKey = ''
  }

  /** Re-read placements when graphics, scroll position or geometry changed. */
  update(core: TermyCore, metrics: CellMetrics, enabled: boolean): void {
    if (!enabled) {
      if (this.#placements.length > 0) {
        this.#placements = []
        this.#paint(metrics)
      }
      return
    }
    const revision = core.graphicsRevision
    const viewKey = `${core.displayOffset},${core.historySize},${core.cols},${core.rows},${metrics.deviceWidth},${metrics.deviceHeight}`
    if (revision === this.#revision && viewKey === this.#viewKey) return
    this.#revision = revision
    this.#viewKey = viewKey
    this.#placements = core.readGraphics(metrics.width, metrics.height)
    const live = new Set<string>()
    for (const placement of this.#placements) {
      const key = imageKey(placement)
      live.add(key)
      if (!this.#bitmaps.has(key)) this.#decode(core, placement, key)
    }
    for (const [key, bitmap] of this.#bitmaps) {
      if (!live.has(key)) {
        bitmap?.close()
        this.#bitmaps.delete(key)
      }
    }
    this.#paint(metrics)
  }

  /** Milliseconds until the next animation frame of a visible image, or -1. */
  deadline(core: TermyCore): number {
    return this.#placements.length === 0 ? -1 : core.graphicsDeadline()
  }

  repaint(metrics: CellMetrics): void {
    this.#paint(metrics)
  }

  dispose(): void {
    for (const bitmap of this.#bitmaps.values()) bitmap?.close()
    this.#bitmaps.clear()
    this.under.remove()
    this.over.remove()
  }

  #decode(core: TermyCore, placement: GraphicsPlacement, key: string): void {
    const image = core.graphicsImage(placement.index)
    if (!image || typeof createImageBitmap !== 'function') return
    this.#bitmaps.set(key, null)
    const source =
      image.format === 'png'
        ? new Blob([image.data as Uint8Array<ArrayBuffer>], { type: 'image/png' })
        : new ImageData(new Uint8ClampedArray(image.data), placement.imageWidth, placement.imageHeight)
    createImageBitmap(source).then(
      (bitmap) => {
        if (!this.#bitmaps.has(key)) {
          bitmap.close()
          return
        }
        this.#bitmaps.set(key, bitmap)
        this.#onReady()
      },
      () => this.#bitmaps.delete(key),
    )
  }

  #paint(metrics: CellMetrics): void {
    const dpr = metrics.dpr
    const contexts = [this.under.getContext('2d'), this.over.getContext('2d')]
    for (const ctx of contexts) ctx?.clearRect(0, 0, this.under.width, this.under.height)
    const ordered = [...this.#placements].sort((a, b) => a.zIndex - b.zIndex || a.placementSerial - b.placementSerial)
    for (const placement of ordered) {
      const bitmap = this.#bitmaps.get(imageKey(placement))
      const ctx = contexts[placement.zIndex < BELOW_TEXT ? 0 : 1]
      if (!bitmap || !ctx || placement.clip.width <= 0 || placement.clip.height <= 0) continue
      ctx.save()
      ctx.beginPath()
      ctx.rect(placement.clip.left * dpr, placement.clip.top * dpr, placement.clip.width * dpr, placement.clip.height * dpr)
      ctx.clip()
      ctx.drawImage(
        bitmap,
        placement.sourceX,
        placement.sourceY,
        Math.max(1, placement.sourceWidth),
        Math.max(1, placement.sourceHeight),
        placement.draw.left * dpr,
        placement.draw.top * dpr,
        placement.draw.width * dpr,
        placement.draw.height * dpr,
      )
      ctx.restore()
    }
  }
}

function imageKey(placement: GraphicsPlacement): string {
  return `${placement.imageId}:${placement.imageGeneration}:${placement.imageWidth}x${placement.imageHeight}`
}

function layerCanvas(): HTMLCanvasElement {
  const canvas = document.createElement('canvas')
  canvas.style.position = 'absolute'
  canvas.style.left = '0'
  canvas.style.top = '0'
  canvas.style.pointerEvents = 'none'
  return canvas
}
