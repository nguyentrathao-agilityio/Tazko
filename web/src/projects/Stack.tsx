import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import type { Person } from '@/projects/types'

export function Stack({ people }: { people: Person[] }) {
  const shown = people.slice(0, 5)
  return (
    <span className="stack">
      {shown.map((p) => {
        const name = displayName(p.name, p.email)
        return (
          <span key={p.id} title={name}>
            <Avatar name={name} />
          </span>
        )
      })}
      {people.length > 5 && <span className="av c4">+{people.length - 5}</span>}
    </span>
  )
}
