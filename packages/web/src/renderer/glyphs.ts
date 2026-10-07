import { type GlyphPlan, glyphPlan } from '@termysh/core'
import type { CellMetrics } from './metrics.ts'

/** Code points that may use Termy's geometry instead of the font. */
export function isCustomGlyphCandidate(cp: number): boolean {
  return (
    (cp >= 0x2500 && cp <= 0x259f) || // box drawing, block elements
    (cp >= 0x2800 && cp <= 0x28ff) || // Braille
    (cp >= 0x25e2 && cp <= 0x25e5) || // triangles
    (cp >= 0x1fb00 && cp <= 0x1fbff) // sextants, legacy computing
  )
}

const MAX_CACHE = 4096

export class GlyphCache {
  #plans = new Map<string, GlyphPlan | null>()
  #metrics: CellMetrics | undefined

  setMetrics(metrics: CellMetrics): void {
    this.#metrics = metrics
    this.#plans.clear()
  }

  plan(cp: number, neighbors: [number, number, number, number]): GlyphPlan | undefined {
    const metrics = this.#metrics
    if (!metrics) return undefined
    const key = `${cp},${neighbors.join(',')}`
    let plan = this.#plans.get(key)
    if (plan === undefined) {
      plan = glyphPlan(cp, neighbors, metrics.deviceWidth, metrics.deviceHeight, metrics.fontSize) ?? null
      if (this.#plans.size >= MAX_CACHE) this.#plans.clear()
      this.#plans.set(key, plan)
    }
    return plan ?? undefined
  }
}

/** Paint a normalized plan into the cell at device pixel `(x, y)`. */
export function drawGlyphPlan(
  ctx: CanvasRenderingContext2D,
  plan: GlyphPlan,
  x: number,
  y: number,
  width: number,
  height: number,
): void {
  for (const rect of plan.rects) {
    const outward = rect.snap === 'outward'
    const left = outward ? Math.floor(x + rect.left * width) : Math.round(x + rect.left * width)
    const top = outward ? Math.floor(y + rect.top * height) : Math.round(y + rect.top * height)
    const right = outward ? Math.ceil(x + rect.right * width) : Math.round(x + rect.right * width)
    const bottom = outward ? Math.ceil(y + rect.bottom * height) : Math.round(y + rect.bottom * height)
    if (right <= left || bottom <= top) continue
    ctx.globalAlpha = rect.alpha
    ctx.fillRect(left, top, right - left, bottom - top)
  }
  ctx.globalAlpha = 1
  if (plan.strokes.length === 0) return
  ctx.save()
  ctx.strokeStyle = ctx.fillStyle
  ctx.lineCap = 'butt'
  for (const stroke of plan.strokes) {
    const p = stroke.points.map((point) => [x + point.x * width, y + point.y * height] as const)
    ctx.lineWidth = Math.max(1, stroke.width * width)
    ctx.beginPath()
    ctx.moveTo(p[0]![0], p[0]![1])
    if (stroke.kind === 'roundedCorner' && p.length >= 6) {
      ctx.lineTo(p[1]![0], p[1]![1])
      ctx.bezierCurveTo(p[2]![0], p[2]![1], p[3]![0], p[3]![1], p[4]![0], p[4]![1])
      ctx.lineTo(p[5]![0], p[5]![1])
    } else {
      for (const point of p.slice(1)) ctx.lineTo(point[0], point[1])
    }
    ctx.stroke()
  }
  ctx.restore()
}
