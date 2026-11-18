export type Anchor = { left: number; top: number; bottom: number; width: number }

export function anchorOf(el: Element): Anchor {
  const r = el.getBoundingClientRect()
  return { left: r.left, top: r.top, bottom: r.bottom, width: r.width }
}
