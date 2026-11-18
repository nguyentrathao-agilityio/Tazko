import { describe, expect, it } from 'vitest'
import { t } from '@/i18n'

describe('t', () => {
  it('returns the English string', () => {
    expect(t('app.name')).toBe('Tazko')
  })

  it('fills in variables', () => {
    expect(t('app.status.ok', { version: '1.2.3' })).toBe('Server is running, version 1.2.3.')
  })

  it('leaves unknown variables in place', () => {
    expect(t('app.status.ok')).toContain('{version}')
  })
})
