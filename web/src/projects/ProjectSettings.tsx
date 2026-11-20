import { useState } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { backgroundNames, background, colors } from '@/projects/look'
import type { ProjectView } from '@/projects/types'
import { useRun } from '@/projects/useRun'
import { isOwner } from '@/roles'
import { ErrorNote } from '@/ui/ErrorNote'
import { SwitchRow } from '@/ui/SwitchRow'
import { projectApi } from '@/paths'

export function ProjectSettings({ company, project, onSaved }: { company: Company; project: ProjectView; onSaved: () => void }) {
  const path = projectApi(company.slug, project.id)
  const [name, setName] = useState(project.name)
  const [description, setDescription] = useState(project.description)
  const [color, setColor] = useState(project.color)
  const [bg, setBg] = useState(project.background)
  const { error, runAndToast } = useRun(onSaved)

  const patch = (change: Record<string, unknown>) => void runAndToast(() => api('PATCH', path, change), t('toast.saved'))

  const save = () => patch({ name, description, color, background: bg })

  const flip = (key: 'members_create_boards' | 'members_invite' | 'voting', value: boolean) => patch({ [key]: value })

  return (
    <>
      <ErrorNote message={error} />
      <section className="box">
        <h2>{t('project.general')}</h2>
        <div className="field">
          <label htmlFor="ps-name">{t('project.name')}</label>
          <input type="text" id="ps-name" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="ps-desc">{t('project.description')}</label>
          <input type="text" id="ps-desc" value={description} onChange={(e) => setDescription(e.target.value)} />
        </div>
        <div className="field">
          <label id="ps-color">{t('project.color')}</label>
          <div className="swatches" role="group" aria-labelledby="ps-color">
            {colors.map((c) => (
              <button key={c} type="button" className={`sw av ${c}`} aria-pressed={color === c} aria-label={t('project.colorOption', { name: c })} onClick={() => setColor(c)} />
            ))}
          </div>
        </div>
        <div className="field">
          <label id="ps-bg">{t('project.background')}</label>
          <div className="swatches" role="group" aria-labelledby="ps-bg">
            {backgroundNames.map((k) => (
              <button
                key={k}
                type="button"
                className="sw"
                style={{ background: background(k) }}
                aria-pressed={bg === k}
                aria-label={t('project.backgroundOption', { name: k })}
                onClick={() => setBg(k)}
              />
            ))}
          </div>
        </div>
        <button type="button" className="btn primary" onClick={save}>
          {t('common.save')}
        </button>
      </section>
      <section className="box">
        <h2>{t('project.permissions')}</h2>
        <SwitchRow
          label={t('project.membersCreateBoards')}
          hint={t('project.membersCreateBoardsHint')}
          on={project.members_create_boards}
          onChange={(v) => flip('members_create_boards', v)}
        />
        <SwitchRow label={t('project.membersInvite')} hint={t('project.membersInviteHint')} on={project.members_invite} onChange={(v) => flip('members_invite', v)} />
      </section>
      <section className="box">
        <h2>{t('project.cards')}</h2>
        <SwitchRow label={t('project.voting')} hint={t('project.votingHint')} on={project.voting} onChange={(v) => flip('voting', v)} />
      </section>
      <section className="box danger-zone">
        <h2>{t('project.archiveOrDelete')}</h2>
        <p className="muted">
          {isOwner(company.role) ? t('project.archiveOwner') : t('project.archiveAsk', { company: company.name })}
        </p>
      </section>
    </>
  )
}
