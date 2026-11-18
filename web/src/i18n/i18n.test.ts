import { describe, expect, it } from 'vitest'
import { t } from '@/i18n'

describe('t', () => {
  it('returns the English string', () => {
    expect(t('app.name')).toBe('Tazko')
  })

  it('fills in variables', () => {
    expect(t('noAccess.body', { company: 'Acme' })).toBe('Ask an owner of Acme.')
  })

  it('leaves unknown variables in place', () => {
    expect(t('noAccess.body')).toContain('{company}')
  })
})
