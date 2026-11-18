import { describe, expect, it } from 'vitest'
import { t } from '@/i18n'

describe('t', () => {
  it('returns the English string', () => {
    expect(t('app.name')).toBe('Tazko')
  })

  it('fills in variables', () => {
    expect(t('home.signedInAs', { email: 'a@example.com' })).toBe('Signed in as a@example.com')
  })

  it('leaves unknown variables in place', () => {
    expect(t('home.signedInAs')).toContain('{email}')
  })
})
