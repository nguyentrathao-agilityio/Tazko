import type { MessageKey } from '@/i18n'

export const OWNER = 'owner'
export const MEMBER = 'member'
export const OBSERVER = 'observer'

export const COMPANY_ROLES = [OWNER, MEMBER] as const
export const PROJECT_ROLES = [OWNER, MEMBER, OBSERVER] as const

export type CompanyRole = (typeof COMPANY_ROLES)[number]
export type ProjectRole = (typeof PROJECT_ROLES)[number]

export const isOwner = (role: string): boolean => role === OWNER
export const isObserver = (role: string): boolean => role === OBSERVER

export const companyRoleLabel = (role: string): MessageKey => (isOwner(role) ? 'role.owner' : 'role.member')
