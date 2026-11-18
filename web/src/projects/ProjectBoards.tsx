import { useState, type FormEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { Go } from '@/ui/Link'
import { Modal } from '@/ui/Modal'
import { useLoad } from '@/ui/useLoad'
import { BoardTile } from '@/board/BoardTile'
import type { BoardTileData } from '@/board/types'
import { background, backgroundNames } from '@/projects/look'
import type { ProjectView } from '@/projects/types'
import { useRun } from '@/projects/useRun'

export function ProjectBoards({ company, project, go }: { company: Company; project: ProjectView; go: Go }) {
  const path = `/api/v1/c/${company.slug}/projects/${project.id}/boards`
  const { data, error: loadError, reload } = useLoad(() => api<{ boards: BoardTileData[] }>('GET', path), path)
  const [creating, setCreating] = useState(false)
  const [showClosed, setShowClosed] = useState(false)
  const { error, run } = useRun(reload)
  const boards = data?.boards ?? []
  const open = boards.filter((b) => !b.closed)
  const starred = open.filter((b) => b.starred)
  const closed = boards.filter((b) => b.closed)
  const openBoard = (id: number) => go(`/c/${company.slug}/b/${id}`)
  const star = (b: BoardTileData, on: boolean) => void run(() => api('PUT', `/api/v1/c/${company.slug}/boards/${b.id}/star`, { starred: on }))

  return (
    <>
      {(error || loadError) && (
        <p className="error" role="alert">
          {error ?? loadError}
        </p>
      )}
      {closed.length > 0 && (
        <div className="row" style={{ marginBottom: 14 }}>
          <button type="button" className="btn quiet" onClick={() => setShowClosed((s) => !s)}>
            {t(showClosed ? 'project.hideClosed' : 'project.showClosed', { n: closed.length })}
          </button>
        </div>
      )}
      {starred.length > 0 && (
        <>
          <div className="sechd">
            <Icon name="star" />
            {t('project.starred')}
          </div>
          <div className="pgrid">
            {starred.map((b) => (
              <BoardTile key={b.id} board={b} onOpen={() => openBoard(b.id)} onStar={(on) => star(b, on)} />
            ))}
          </div>
        </>
      )}
      <div className="sechd">
        <Icon name="board" />
        {t('project.allBoards')}
      </div>
      <div className="pgrid">
        {open.map((b) => (
          <BoardTile key={b.id} board={b} onOpen={() => openBoard(b.id)} onStar={(on) => star(b, on)} />
        ))}
        {project.can.create_board && (
          <button type="button" className="btile new" onClick={() => setCreating(true)}>
            <span className="row" style={{ flexWrap: 'nowrap', gap: 6 }}>
              <Icon name="plus" />
              {t('project.createBoard')}
            </span>
          </button>
        )}
      </div>
      {data && open.length === 0 && !project.can.create_board && <p className="muted">{t('project.noBoards')}</p>}
      {!project.can.create_board && <p className="hint">{t('project.boardsOwnersOnly')}</p>}
      {showClosed && closed.length > 0 && (
        <>
          <div className="sechd">
            <Icon name="archive" />
            {t('project.closedBoards')}
          </div>
          <div className="tw">
            <table>
              <tbody>
                {closed.map((b) => (
                  <tr key={b.id}>
                    <td>
                      <b>{b.name}</b>
                    </td>
                    <td className="num">
                      {project.can.manage && (
                        <button type="button" className="btn sm" onClick={() => void run(() => api('POST', `/api/v1/c/${company.slug}/boards/${b.id}/reopen`))}>
                          {t('project.reopen')}
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
      {creating && (
        <CreateBoard
          initial={project.background}
          onClose={() => setCreating(false)}
          onCreate={async (body) => {
            const res = await api<{ id: number }>('POST', path, body)
            setCreating(false)
            openBoard(res.id)
          }}
        />
      )}
    </>
  )
}

function CreateBoard({
  initial,
  onClose,
  onCreate,
}: {
  initial: string
  onClose: () => void
  onCreate: (body: { name: string; background: string; restricted: boolean }) => Promise<void>
}) {
  const [bg, setBg] = useState(initial)
  const [problem, setProblem] = useState<string | null>(null)

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    const name = String(f.get('name') ?? '').trim()
    if (!name) {
      setProblem(t('board.titleNeeded'))
      return
    }

    onCreate({ name, background: bg, restricted: f.get('access') === 'r' }).catch((err: unknown) => setProblem(err instanceof Error ? err.message : String(err)))
  }

  return (
    <Modal label={t('board.create')} onClose={onClose}>
      <h2>{t('board.create')}</h2>
      <form onSubmit={submit} noValidate>
        <div className="field">
          <label htmlFor="nb-n">{t('board.title')}</label>
          <input type="text" id="nb-n" name="name" placeholder={t('board.titlePlaceholder')} />
          {problem && (
            <p className="error" role="alert">
              {problem}
            </p>
          )}
        </div>
        <div className="field">
          <label id="nb-bg">{t('board.background')}</label>
          <div className="swatches" role="group" aria-labelledby="nb-bg">
            {backgroundNames.map((k) => (
              <button key={k} type="button" className="sw" style={{ background: background(k) }} aria-pressed={bg === k} aria-label={k} onClick={() => setBg(k)} />
            ))}
          </div>
        </div>
        <div className="field">
          <label htmlFor="nb-a">{t('board.access')}</label>
          <select id="nb-a" name="access" style={{ width: '100%' }}>
            <option value="p">{t('board.wholeProject')}</option>
            <option value="r">{t('board.restrictedChosen')}</option>
          </select>
        </div>
        <div className="acts">
          <button type="button" className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" type="submit">
            {t('board.create')}
          </button>
        </div>
      </form>
    </Modal>
  )
}
