/// <reference types="node" />
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { EmptyState } from '@/ui/EmptyState'
import { Toaster } from '@/ui/Toaster'
import { useToast } from '@/ui/toast'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

function Trigger({ undo }: { undo?: () => void }) {
  const toast = useToast()
  return (
    <button type="button" onClick={() => toast('Role changed', undo)}>
      go
    </button>
  )
}

describe('toasts', () => {
  it('shows a message and hides it after a short while', () => {
    vi.useFakeTimers()
    render(
      <Toaster>
        <Trigger />
      </Toaster>,
    )
    fireEvent.click(screen.getByText('go'))
    expect(screen.getByRole('status').textContent).toContain('Role changed')
    expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull()
    act(() => vi.advanceTimersByTime(3000))
    expect(screen.getByRole('status').textContent).toBe('')
  })

  it('offers Undo for six seconds and runs it once', () => {
    vi.useFakeTimers()
    const undo = vi.fn()
    render(
      <Toaster>
        <Trigger undo={undo} />
      </Toaster>,
    )
    fireEvent.click(screen.getByText('go'))
    act(() => vi.advanceTimersByTime(5000))
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    expect(undo).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull()
  })

  it('replaces an older toast with a newer one', () => {
    render(
      <Toaster>
        <Trigger />
      </Toaster>,
    )
    fireEvent.click(screen.getByText('go'))
    fireEvent.click(screen.getByText('go'))
    expect(screen.getAllByText('Role changed')).toHaveLength(1)
  })
})

describe('empty states', () => {
  it('shows a title, text and actions', () => {
    render(
      <EmptyState title="No projects yet" body="Owners create projects.">
        <button type="button">Do it</button>
      </EmptyState>,
    )
    expect(screen.getByRole('heading', { name: 'No projects yet' })).toBeTruthy()
    expect(screen.getByText('Owners create projects.')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Do it' })).toBeTruthy()
  })
})

describe('theme', () => {
  it('maps every colour, radius, size and font token from the design into Tailwind', () => {
    const tokens = readFileSync(resolve(process.cwd(), '../design/tokens.css'), 'utf8')
    const theme = readFileSync(resolve(process.cwd(), 'src/ui/theme.css'), 'utf8')
    const names = new Set([...tokens.matchAll(/--([a-z0-9-]+)\s*:/g)].map((m) => m[1]))
    const product = [...names].filter((n) => !n.startsWith('proto'))
    expect(product.length).toBeGreaterThan(30)
    for (const name of product) expect(theme, name).toContain(`var(--${name})`)
  })
})
