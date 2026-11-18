import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { Button } from '@/components/ui/button'

afterEach(cleanup)

describe('Button', () => {
  it('defaults to a non-submitting button', () => {
    render(<Button>Save</Button>)
    expect(screen.getByRole('button', { name: 'Save' }).getAttribute('type')).toBe('button')
  })
})
