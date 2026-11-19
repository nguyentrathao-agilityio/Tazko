import { t } from '@/i18n'

export type FieldType = 'text' | 'number' | 'date' | 'select' | 'check'

export type FieldDef = { id: number; name: string; type: FieldType; options: string[]; show_on_card: boolean }

export type FieldValue = string | number | boolean

export type Reaction = { emoji: string; count: number; mine: boolean; people: string[] }

export const EMOJI = ['👍', '❤️', '🎉', '👀', '🚀', '✅']

export function fieldText(def: FieldDef, v: FieldValue | undefined): string {
  if (v === undefined || v === null || v === '' || v === false) return ''
  if (def.type === 'check') return t('fields.yes')
  if (def.type === 'date' && typeof v === 'string') {
    const d = new Date(`${v}T12:00:00`)
    return Number.isNaN(d.getTime()) ? v : d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
  }

  return String(v)
}

export function imageId(background: string): number | null {
  const m = /^img:(\d+)$/.exec(background)
  return m ? Number(m[1]) : null
}
