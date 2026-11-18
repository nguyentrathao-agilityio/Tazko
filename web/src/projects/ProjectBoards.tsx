import type { Company } from '@/api'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { Go } from '@/ui/Link'
import type { ProjectView } from '@/projects/types'

export function ProjectBoards({ project }: { company: Company; project: ProjectView; go: Go }) {
  return (
    <>
      <div className="sechd">
        <Icon name="board" />
        {t('project.allBoards')}
      </div>
      <p className="muted">{t('project.noBoards')}</p>
      {!project.can.create_board && <p className="hint">{t('project.boardsOwnersOnly')}</p>}
    </>
  )
}
