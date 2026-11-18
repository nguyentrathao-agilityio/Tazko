import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { t } from '@/i18n'
import { ToastContext, type Undo } from '@/ui/toast'

type Shown = { id: number; message: string; undo?: Undo }

export function Toaster({ children }: { children: ReactNode }) {
  const [shown, setShown] = useState<Shown | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const next = useRef(0)

  const toast = useCallback((message: string, undo?: Undo) => {
    clearTimeout(timer.current)
    next.current += 1
    const id = next.current
    setShown({ id, message, undo })
    timer.current = setTimeout(() => setShown((s) => (s?.id === id ? null : s)), undo ? 6000 : 2800)
  }, [])

  useEffect(() => () => clearTimeout(timer.current), [])

  const onUndo = async () => {
    const undo = shown?.undo
    clearTimeout(timer.current)
    setShown(null)
    if (undo) await undo()
  }

  return (
    <ToastContext.Provider value={toast}>
      {children}
      <div
        role="status"
        aria-live="polite"
        className="pointer-events-none fixed inset-x-0 bottom-[calc(20px+env(safe-area-inset-bottom,0px))] z-[90] flex justify-center px-3"
      >
        {shown && (
          <div className="pointer-events-auto flex max-w-full items-center gap-3.5 rounded-[10px] bg-ink px-3.5 py-2.5 font-medium text-bg shadow-raised">
            <span>{shown.message}</span>
            {shown.undo && (
              <button
                type="button"
                className="cursor-pointer border-0 bg-transparent font-bold text-accent brightness-[1.4]"
                onClick={() => void onUndo()}
              >
                {t('toast.undo')}
              </button>
            )}
          </div>
        )}
      </div>
    </ToastContext.Provider>
  )
}
