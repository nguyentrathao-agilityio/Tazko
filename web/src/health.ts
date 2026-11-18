export type Health = { status: string; version: string }

export async function fetchHealth(signal?: AbortSignal): Promise<Health> {
  const res = await fetch('/health', { signal, headers: { accept: 'application/json' } })
  if (!res.ok) throw new Error(`health ${res.status}`)
  return (await res.json()) as Health
}
