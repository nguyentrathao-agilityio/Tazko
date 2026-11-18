import { createContext, useContext } from 'react'

export type Undo = () => Promise<unknown> | void

export type ToastApi = (message: string, undo?: Undo) => void

export const ToastContext = createContext<ToastApi>(() => undefined)

export function useToast(): ToastApi {
  return useContext(ToastContext)
}
