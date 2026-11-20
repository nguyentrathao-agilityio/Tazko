import { useEffect, useRef, useState } from 'react'
import { boardApiOf } from '@/paths'

export type Viewer = { id: number; name: string; email: string }
export type LiveEvent = { id: number; type: string; actor: { id: number | null; name: string }; card: number | null; created_at: number }
export type LiveStatus = 'connecting' | 'live' | 'offline' | 'closed'

type Msg =
  | { type: 'hello'; last: number; presence: Viewer[] }
  | { type: 'presence'; presence: Viewer[] }
  | { type: 'events'; events: LiveEvent[]; catch_up?: boolean }
  | { type: 'reset'; last: number }
  | { type: 'closed'; reason: string }
  | { type: 'pong' }

const delays = [500, 1000, 2000, 4000, 5000]

export function socketUrl(path: string): string {
  const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws'
  return `${scheme}://${window.location.host}${path}`
}

export function keepAlive(ws: WebSocket): number {
  return window.setInterval(() => ws.readyState === WebSocket.OPEN && ws.send('ping'), 30_000)
}

export function liveUrl(slug: string, board: number, after: number | null): string {
  const q = after === null ? '' : `?after=${after}`
  return socketUrl(`${boardApiOf(slug, board)}/live${q}`)
}

export function useLive(slug: string, board: number, start: number | null, onChange: (events: LiveEvent[] | null) => void) {
  const [status, setStatus] = useState<LiveStatus>('connecting')
  const [viewers, setViewers] = useState<Viewer[]>([])
  const handler = useRef(onChange)
  const last = useRef<number | null>(start)

  useEffect(() => {
    handler.current = onChange
  })

  useEffect(() => {
    if (last.current === null && start !== null) last.current = start
  }, [start])

  const ready = start !== null

  useEffect(() => {
    if (!ready || typeof WebSocket === 'undefined') return
    let socket: WebSocket | null = null
    let attempt = 0
    let timer: number | undefined
    let ping: number | undefined
    let stopped = false

    const connect = () => {
      if (stopped) return
      setStatus('connecting')
      const ws = new WebSocket(liveUrl(slug, board, last.current))
      socket = ws
      ws.onmessage = (e) => {
        let msg: Msg
        try {
          msg = JSON.parse(String(e.data)) as Msg
        } catch {
          return
        }

        if (msg.type === 'hello') {
          attempt = 0
          setStatus('live')
          setViewers(msg.presence)
          if (last.current === null) last.current = msg.last
        } else if (msg.type === 'presence') {
          setViewers(msg.presence)
        } else if (msg.type === 'events' && msg.events.length) {
          last.current = Math.max(last.current ?? 0, ...msg.events.map((x) => x.id))
          handler.current(msg.events)
        } else if (msg.type === 'reset') {
          last.current = msg.last
          handler.current(null)
        } else if (msg.type === 'closed') {
          stopped = true
          setStatus('closed')
        }
      }
      ws.onclose = () => {
        if (socket !== ws) return
        socket = null
        window.clearInterval(ping)
        if (stopped) return
        setStatus('offline')
        timer = window.setTimeout(connect, delays[Math.min(attempt++, delays.length - 1)])
      }
      ws.onopen = () => {
        ping = keepAlive(ws)
      }
    }

    const online = () => {
      if (socket || stopped) return
      window.clearTimeout(timer)
      attempt = 0
      connect()
    }

    connect()
    window.addEventListener('online', online)

    return () => {
      stopped = true
      window.removeEventListener('online', online)
      window.clearTimeout(timer)
      window.clearInterval(ping)
      const ws = socket
      socket = null
      ws?.close()
    }
  }, [slug, board, ready])

  return { status, viewers }
}
