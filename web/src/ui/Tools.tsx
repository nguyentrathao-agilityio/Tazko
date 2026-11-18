import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { useMe } from '@/ui/meContext'
import { SearchButton } from '@/search/Search'

export function Tools() {
  const me = useMe()
  const name = me ? displayName(me.name, me.email) : ''
  return (
    <div className="tools">
      <SearchButton />
      {me && (
        <span title={name}>
          <Avatar name={name} />
        </span>
      )}
    </div>
  )
}
