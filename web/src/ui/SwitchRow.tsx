export function Switch({ label, on, disabled, onChange }: { label: string; on: boolean; disabled?: boolean; onChange: (on: boolean) => void }) {
  return <button type="button" className="switchbtn" role="switch" aria-checked={on} aria-label={label} disabled={disabled} onClick={() => onChange(!on)} />
}

export function SwitchRow({ label, hint, on, onChange }: { label: string; hint: string; on: boolean; onChange: (on: boolean) => void }) {
  return (
    <div className="switchrow">
      <div>
        <b>{label}</b>
        <small>{hint}</small>
      </div>
      <Switch label={label} on={on} onChange={onChange} />
    </div>
  )
}
