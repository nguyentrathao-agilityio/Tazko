import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { CommentText } from '@/card/Rich'
import { usage, sizeShort } from '@/card/files'

afterEach(cleanup)

describe('comment text', () => {
  it('shows mentions of people on the board as chips', () => {
    const { container } = render(<CommentText text={'Option B. @Trung Ngo can you check? cc @mai, not mail@mai.example or @Nobody'} names={['Trung Ngo', 'trung@a.example', 'Mai Tran', 'mai']} />)
    const chips = [...container.querySelectorAll('.men')].map((m) => m.textContent)
    expect(chips).toEqual(['@Trung Ngo', '@mai'])
    expect(container.textContent).toBe('Option B. @Trung Ngo can you check? cc @mai, not mail@mai.example or @Nobody')
  })

  it('keeps links and bold inside mentioned text', () => {
    const { container } = render(<CommentText text={'**Done** @Jo Lee see https://example.com/x'} names={['Jo Lee']} />)
    expect(container.querySelector('strong')?.textContent).toBe('Done')
    expect(container.querySelector('.men')?.textContent).toBe('@Jo Lee')
    expect(container.querySelector('a')?.getAttribute('href')).toBe('https://example.com/x')
  })
})

describe('storage wording', () => {
  it('reads like the prototype', () => {
    expect(sizeShort(10 * 1024 * 1024)).toBe('10 MB')
    expect(usage(1.8 * 1024 ** 3, 5 * 1024 ** 3)).toBe('1.8 of 5 GB')
    expect(usage(921 * 1024, 5 * 1024 ** 3)).toBe('921 KB of 5 GB')
  })
})
