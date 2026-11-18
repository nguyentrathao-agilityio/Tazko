import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import type { Person } from '@/projects/types'

export function Stack({ people }: { people: Person[] }) {
  const shown = people.slice(0, 5)
  return (
    <span className="stack">
      {shown.map((p) => (
        <span key={p.id} title={displayName(p.name, p.email)}>
          <Avatar name={displayName(p.name, p.email)} />
        </span>
      ))}
      {people.length > 5 && <span className="av c4">+{people.length - 5}</span>}
    </span>
  )
}
