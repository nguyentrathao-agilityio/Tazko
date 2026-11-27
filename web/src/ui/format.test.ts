import { describe, expect, it } from 'vitest'
import { expiresIn } from './format'

describe('expiresIn', () => {
  const now = new Date('2026-01-10T12:00:00Z')
  const at = (hours: number) => now.getTime() / 1000 + hours * 3600

  it('counts whole days left without rounding up', () => {
    expect(expiresIn(at(7 * 24), now)).toBe('expires in 7 days')
    expect(expiresIn(at(2 * 24 - 1), now)).toBe('expires in 1 day')
    expect(expiresIn(at(23), now)).toBe('expires within a day')
  })
})
