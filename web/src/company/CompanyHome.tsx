import type { Company } from '@/api'
import { t } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'

export function NoAccess({ company }: { company: Company }) {
  return <EmptyState title={t('noAccess.title')} body={t('noAccess.body', { company: company.name })} />
}
