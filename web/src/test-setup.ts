import { configure } from '@testing-library/react'

const BUSY_MACHINE_WAIT_MS = 5000

configure({ asyncUtilTimeout: BUSY_MACHINE_WAIT_MS })

class QuietSocket {
  static readonly OPEN = 1
  readyState = 0
  onmessage: ((e: { data: string }) => void) | null = null
  onclose: (() => void) | null = null
  onopen: (() => void) | null = null
  send() {}
  close() {}
}

Object.defineProperty(globalThis, 'WebSocket', { value: QuietSocket, writable: true, configurable: true })
