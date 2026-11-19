export function ErrorNote({ message }: { message: string | null | undefined }) {
  if (!message) return null
  return (
    <p className="error" role="alert">
      {message}
    </p>
  )
}
