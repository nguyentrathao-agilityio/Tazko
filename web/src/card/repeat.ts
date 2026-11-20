import { useCallback } from 'react'
import { useCardToggle } from '@/card/toggle'
import { api } from '@/http'
import { t, type MessageKey } from '@/i18n'
import { shortDate } from '@/card/look'
import { useToast } from '@/ui/toast'

export const RULES = ['daily', 'weekdays', 'weekly', 'biweekly', 'monthly'] as const

export type Rule = (typeof RULES)[number]

export type Repeated = { rule: Rule; from: number; to: number }

export const ruleText = (rule: string): string => t(`repeat.${rule}` as MessageKey)

function doneBody(done: boolean) {
  return done ? { done: true, tz_offset: -new Date().getTimezoneOffset() } : { done: false }
}

export function useToggleDone(run: (fn: () => Promise<unknown>) => Promise<boolean>) {
  const toast = useToast()
  const toggle = useCardToggle()
  return useCallback(
    (boardApi: string, card: { id: number; done?: boolean }) =>
      toggle(card.id, Boolean(card.done), (done) =>
        run(async () => {
          const res = await api<{ repeated?: Repeated } | null>('PATCH', `${boardApi}/cards/${card.id}`, doneBody(done))
          if (res?.repeated) {
            toast(t('repeat.toast', { rule: ruleText(res.repeated.rule), date: shortDate(res.repeated.to) }), () => run(() => api('POST', `${boardApi}/cards/${card.id}/repeat/undo`)))
          }
        }),
      ),
    [run, toast, toggle],
  )
}
