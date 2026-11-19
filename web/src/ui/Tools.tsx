import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { useMe } from '@/ui/meContext'
import { SearchButton } from '@/search/Search'
import { Bell } from '@/notify/Bell'
import { companyOf } from '@/notify/notes'

export function Tools() {
  const me = useMe()
  const name = me ? displayName(me.name, me.email) : ''
  const slug = companyOf(window.location.pathname)
  return (
    <div className="tools">
      <SearchButton />
      {me && slug && <Bell key={slug} slug={slug} />}
      {me && (
        <span title={name}>
          <Avatar name={name} />
        </span>
      )}
    </div>
  )
}
