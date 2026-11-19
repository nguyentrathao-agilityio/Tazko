import { useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react'
import { t } from '@/i18n'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { matches, mentionQuery, type Person } from '@/card/mention'

export function MentionBox({
  value,
  people,
  label,
  placeholder,
  onChange,
  onSubmit,
}: {
  value: string
  people: Person[]
  label: string
  placeholder: string
  onChange: (v: string) => void
  onSubmit: () => void
}) {
  const box = useRef<HTMLTextAreaElement>(null)
  const pending = useRef<number | null>(null)
  const [caret, setCaret] = useState(0)
  const [active, setActive] = useState(0)
  const [dismissed, setDismissed] = useState<number | null>(null)
  const q = mentionQuery(value, caret)
  const list = q && dismissed !== q.start ? matches(people, q.query) : []

  useLayoutEffect(() => {
    const at = pending.current
    if (at === null || !box.current) return
    pending.current = null
    box.current.focus()
    box.current.setSelectionRange(at, at)
  }, [value])

  const pick = (p: Person) => {
    if (!q) return
    const handle = p.name.trim() || p.email
    const next = `${value.slice(0, q.start)}@${handle} ${value.slice(caret)}`
    const at = q.start + handle.length + 2
    pending.current = at
    onChange(next)
    setCaret(at)
    setActive(0)
  }

  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (list.length) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        setActive((a) => (a + (e.key === 'ArrowDown' ? 1 : list.length - 1)) % list.length)
        return
      }

      if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault()
        pick(list[Math.min(active, list.length - 1)])
        return
      }

      if (e.key === 'Escape' && q) {
        e.preventDefault()
        e.stopPropagation()
        setDismissed(q.start)
        return
      }
    }

    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      onSubmit()
    }
  }

  return (
    <>
      <textarea
        ref={box}
        className="ta-s"
        value={value}
        placeholder={placeholder}
        aria-label={label}
        aria-autocomplete="list"
        onChange={(e) => {
          onChange(e.target.value)
          setCaret(e.target.selectionStart)
          setActive(0)
        }}
        onSelect={(e) => setCaret(e.currentTarget.selectionStart)}
        onKeyDown={onKey}
      />
      {list.length > 0 && (
        <div className="sugg" role="listbox" aria-label={t('notify.suggestions')}>
          {list.map((p, i) => {
            const name = displayName(p.name, p.email)
            return (
              <button
                type="button"
                key={p.id}
                className="mr"
                role="option"
                aria-selected={i === active}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => pick(p)}
              >
                <Avatar name={name} />
                <span>{name}</span>
              </button>
            )
          })}
        </div>
      )}
    </>
  )
}
