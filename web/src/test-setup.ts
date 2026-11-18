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
