import en from './en.json'

export type MessageKey = keyof typeof en

const messages: Record<string, string> = en

export function t(key: MessageKey, vars: Record<string, string | number> = {}): string {
  const text = messages[key] ?? key
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in vars ? String(vars[name]) : match,
  )
}
