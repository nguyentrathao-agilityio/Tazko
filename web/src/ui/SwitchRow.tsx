export function SwitchRow({ label, hint, on, onChange }: { label: string; hint: string; on: boolean; onChange: (on: boolean) => void }) {
  return (
    <div className="switchrow">
      <div>
        <b>{label}</b>
        <small>{hint}</small>
      </div>
      <button type="button" className="switchbtn" role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />
    </div>
  )
}
