import { t } from '@/i18n'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import type { LiveStatus, Viewer } from '@/live/useLive'

export function LivePill({ status, viewers, onClick }: { status: LiveStatus; viewers: Viewer[]; onClick: (el: HTMLElement) => void }) {
  if (status === 'closed') return null
  const live = status === 'live'
  return (
    <button type="button" className={`live${live ? '' : ' off'}`} title={t('live.title')} onClick={(e) => onClick(e.currentTarget)}>
      <i aria-hidden="true" />
      {live ? t('live.here', { n: viewers.length }) : t('live.reconnecting')}
    </button>
  )
}

export function LiveList({ viewers, me }: { viewers: Viewer[]; me: number }) {
  return (
    <>
      <p className="hint" style={{ margin: '0 0 8px', padding: 0, border: 0 }}>
        {t('live.hint')}
      </p>
      {viewers.map((v) => {
        const name = displayName(v.name, v.email)
        return (
          <div key={v.id} className="mr" style={{ cursor: 'default' }}>
            <Avatar name={name} />
            <span>
              {name}
              {v.id === me ? ` ${t('project.you')}` : ''}
            </span>
            <span className="pill ok" style={{ marginLeft: 'auto' }}>
              {t('live.viewing')}
            </span>
          </div>
        )
      })}
    </>
  )
}
