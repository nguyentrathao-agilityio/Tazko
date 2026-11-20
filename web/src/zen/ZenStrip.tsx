import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { Settings } from '@/settings/store'

export function ZenStrip({
  name,
  mine,
  hideDone,
  density,
  onMine,
  onHideDone,
  onDensity,
  onSearch,
  onPalette,
  onExit,
}: {
  name: string
  mine: boolean
  hideDone: boolean
  density: Settings['zen_density']
  onMine: () => void
  onHideDone: () => void
  onDensity: (d: Settings['zen_density']) => void
  onSearch: () => void
  onPalette: () => void
  onExit: () => void
}) {
  const options: [Settings['zen_density'], string][] = [
    ['comfortable', t('zen.comfortable')],
    ['compact', t('zen.compact')],
    ['minimal', t('zen.minimal')],
  ]
  return (
    <div className="zs-w">
      <div className="zs-h" aria-hidden="true" />
      <div className="zs" role="toolbar" aria-label={t('zen.controls')}>
        <b className="zn">{name}</b>
        <span className="zsep" />
        <button type="button" className={`zc${mine ? ' on' : ''}`} aria-pressed={mine} title="Q" onClick={onMine}>
          {t('zen.mine')}
        </button>
        <button type="button" className={`zc${hideDone ? ' on' : ''}`} aria-pressed={hideDone} onClick={onHideDone}>
          {t('zen.hideDone')}
        </button>
        <span className="zsep" />
        <div className="seg" role="radiogroup" aria-label={t('zen.density')}>
          {options.map(([k, label]) => (
            <button key={k} type="button" role="radio" aria-checked={density === k} onClick={() => onDensity(k)}>
              {label}
            </button>
          ))}
        </div>
        <span className="sp" />
        <button type="button" className="ibtn" aria-label={t('palette.title')} title={t('palette.button')} onClick={onPalette}>
          <Icon name="cmd" />
        </button>
        <button type="button" className="ibtn" aria-label={t('search.title')} title={t('zen.searchHint')} onClick={onSearch}>
          <Icon name="search" />
        </button>
        <button type="button" className="btn sm" onClick={onExit}>
          {t('zen.exit')} <span className="kbd">Z</span>
        </button>
      </div>
    </div>
  )
}
