const API = '/api/v1'

export const ME_API = `${API}/me`
export const ADMIN_API = `${API}/admin`

export const companyApi = (slug: string): string => `${API}/c/${slug}`
export const projectApi = (slug: string, project: number): string => `${companyApi(slug)}/projects/${project}`
export const boardApiOf = (slug: string, board: number): string => `${companyApi(slug)}/boards/${board}`

export const companyRoute = (slug: string): string => `/c/${slug}`
export const projectRoute = (slug: string, project: number): string => `${companyRoute(slug)}/p/${project}`
export const boardRoute = (slug: string, board: number): string => `${companyRoute(slug)}/b/${board}`
export const cardRouteUnder = (boardPath: string, card: number): string => `${boardPath}/c/${card}`
export const cardRoute = (slug: string, board: number, card: number): string => cardRouteUnder(boardRoute(slug, board), card)
