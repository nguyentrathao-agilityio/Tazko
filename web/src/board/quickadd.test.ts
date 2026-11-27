import { describe, expect, it } from 'vitest'
import { applySuggestion, parseDate, parseLine, suggest } from '@/board/quickadd'
import type { BoardPerson, Label } from '@/board/types'

const labels: Label[] = [
  { id: 1, name: 'bug', color: 1 },
  { id: 2, name: 'urgent', color: 2 },
  { id: 3, name: 'In review', color: 3 },
]
const people: BoardPerson[] = [
  { id: 10, name: 'Trung Ngô', email: 'trung@a.example', role: 'member', access: true },
  { id: 11, name: 'Mai Tran', email: 'mai.tran@a.example', role: 'owner', access: true },
  { id: 12, name: 'Olga Viewer', email: 'olga@a.example', role: 'observer', access: true },
]
const friday = new Date(2026, 10, 27, 9, 0)
const noon = (y: number, m: number, d: number) => Math.floor(new Date(y, m - 1, d, 12).getTime() / 1000)

describe('quick add', () => {
  it('reads labels, people and a due date out of the title', () => {
    expect(parseLine('Fix login #bug @trung ^tue !urgent', labels, people, friday)).toEqual({
      title: 'Fix login',
      labels: [1, 2],
      members: [10],
      due_at: noon(2026, 12, 1),
      unknown: [],
    })
  })

  it('matches names without accents, case or spaces', () => {
    expect(parseLine('Ship #In-Review @NGO @mai.tran', labels, people, friday)).toMatchObject({ labels: [3], members: [11], unknown: [{ kind: 'person', text: 'NGO' }] })
    expect(parseLine('Pair with @Trung', labels, people, friday).members).toEqual([10])
  })

  it('leaves unknown words, numbers and escaped signs in the title', () => {
    const p = parseLine('Close \\#12 and #123 #nolabel @olga ^someday', labels, people, friday)
    expect(p.title).toBe('Close #12 and #123 #nolabel @olga ^someday')
    expect(p.unknown).toEqual([
      { kind: 'label', text: 'nolabel' },
      { kind: 'person', text: 'olga' },
      { kind: 'date', text: 'someday' },
    ])
  })

  it('says when a name matches more than one person', () => {
    const twins = [...people, { id: 13, name: 'Trung Le', email: 'trung.le@a.example', role: 'member' as const, access: true }]
    expect(parseLine('Ask @trung', labels, twins, friday).members).toEqual([10])
    expect(parseLine('Ask @trungle', labels, twins, friday).members).toEqual([13])
    const same = [...people, { id: 14, name: 'Mai Pham', email: 'pham@a.example', role: 'member' as const, access: true }]
    expect(parseLine('Ask @mai', labels, same, friday).unknown).toEqual([{ kind: 'ambiguous', text: 'mai' }])
  })

  it('understands the usual date words', () => {
    expect(parseDate('today', friday)).toBe(noon(2026, 11, 27))
    expect(parseDate('fri', friday)).toBe(noon(2026, 11, 27))
    expect(parseDate('tmr', friday)).toBe(noon(2026, 11, 28))
    expect(parseDate('+2w', friday)).toBe(noon(2026, 12, 11))
    expect(parseDate('25/12', friday)).toBe(noon(2026, 12, 25))
    expect(parseDate('3/1', friday)).toBe(noon(2027, 1, 3))
    expect(parseDate('2027-02-30', friday)).toBeNull()
    expect(parseDate('31/4', friday)).toBeNull()
    expect(parseDate('monday', friday)).toBe(noon(2026, 11, 30))
    expect(parseDate('month', friday)).toBeNull()
    expect(parseDate('satellite', friday)).toBeNull()
    expect(parseDate('0050-01-01', friday)).toBeNull()
    expect(parseDate('29/2', new Date(2028, 2, 5))).toBeNull()
  })

  it('suggests what matches the word being typed', () => {
    expect(suggest('Fix #b', labels, people).map((s) => s.label)).toEqual(['bug'])
    expect(suggest('Fix @t', labels, people).map((s) => s.insert)).toEqual(['@trung'])
    expect(suggest('Fix @o', labels, people)).toEqual([])
    expect(suggest('Fix ^to', labels, people).map((s) => s.insert)).toEqual(['^today', '^tomorrow'])
    expect(applySuggestion('Fix @t', { insert: '@trung', label: 'Trung' })).toBe('Fix @trung ')
  })
})
