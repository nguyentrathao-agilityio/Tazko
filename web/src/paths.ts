const API = '/api/v1'

export const ME_API = `${API}/me`
export const ADMIN_API = `${API}/admin`

export const companyApi = (slug: string): string => `${API}/c/${slug}`
export const projectApi = (slug: string, project: number): string => `${companyApi(slug)}/projects/${project}`
export const boardApiOf = (slug: string, board: number): string => `${companyApi(slug)}/boards/${board}`

export const companyRoute = (slug: string): string => `/c/${slug}`
