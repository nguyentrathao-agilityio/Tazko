const KEY = 'tazko.lastCompany'

export function rememberCompany(slug: string) {
  try {
    window.localStorage.setItem(KEY, slug)
  } catch {
    return
  }
}

export function lastCompany(): string | null {
  try {
    return window.localStorage.getItem(KEY)
  } catch {
    return null
  }
}
