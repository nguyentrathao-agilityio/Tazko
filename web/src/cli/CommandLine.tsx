import { t } from '@/i18n'
import { CLI_AUTHORIZE } from '@/paths'
import { Terminal } from '@/tokens/Terminal'
import { Icon } from '@/ui/Icon'

const COMMANDS: [string, string, string][] = [
  ['tazko auth', 'login · logout · status · token', 'cli.cmd.auth'],
  ['tazko project', 'list · members', 'cli.cmd.project'],
  ['tazko board', 'list · view', 'cli.cmd.board'],
  ['tazko card', 'list · view · create · move · done · comment · archive', 'cli.cmd.card'],
  ['tazko my', 'cards', 'cli.cmd.my'],
  ['tazko search', '"query"', 'cli.cmd.search'],
  ['tazko api', 'METHOD /path', 'cli.cmd.api'],
  ['tazko config', 'get · set', 'cli.cmd.config'],
]

export function CommandLine() {
  const host = window.location.origin
  return (
    <>
      <section className="box">
        <h2>{t('cli.install')}</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          {t('cli.installLead')}
        </p>
        <Terminal text={`curl -fsSL ${host}/cli/install.sh | sh`} label={t('cli.install')} />
        <p className="hint">
          {t('cli.installHint')} <a href="/cli/download">{t('cli.download')}</a>
        </p>
      </section>
      <section className="box">
        <h2>{t('cli.signIn')}</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          {t('cli.signInLead')}
        </p>
        <Terminal text={`tazko auth login --host ${host}`} label={t('cli.signIn')} />
        <div className="cli-preview">
          <a className="btn" href={CLI_AUTHORIZE} target="_blank" rel="noopener noreferrer">
            <Icon name="globe" />
            {t('cli.previewApprove')}
          </a>
          <p className="hint">{t('cli.signInHint')}</p>
        </div>
      </section>
      <section className="box">
        <h2>{t('cli.try')}</h2>
        <Terminal
          text={[
            'tazko my cards --due overdue',
            'tazko board list --project "Website relaunch"',
            'tazko card create --board "Sprint 14" --list Backlog --title "Fix footer links" --label Bug',
            'tazko card move 3/142 --to "In review"',
            'tazko card comment 3/142 --body "Ready for review"',
            'tazko search "is:overdue @me" --json id,title,due_at',
            "tazko api GET /me --jq .email",
          ].join('\n')}
          label={t('cli.try')}
        />
        <p className="hint">{t('cli.tryHint')}</p>
      </section>
      <section className="box">
        <h2>{t('cli.commands')}</h2>
        <div className="tw">
          <table>
            <thead>
              <tr>
                <th>{t('cli.col.command')}</th>
                <th>{t('cli.col.sub')}</th>
                <th>{t('cli.col.does')}</th>
              </tr>
            </thead>
            <tbody>
              {COMMANDS.map(([c, sub, key]) => (
                <tr key={c}>
                  <td className="mono">{c}</td>
                  <td className="muted">{sub}</td>
                  <td>{t(key as Parameters<typeof t>[0])}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="hint" style={{ marginTop: 10 }}>
          {t('cli.commandsHint')}
        </p>
      </section>
    </>
  )
}
