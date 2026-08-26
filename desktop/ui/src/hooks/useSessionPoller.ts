import { useEffect } from 'react'
import { useAppStore } from '../store'

export function useSessionPoller(intervalMs: number) {
  const refreshSessions = useAppStore((s) => s.refreshSessions)

  useEffect(() => {
    void refreshSessions().then(() => {
      const lastId = localStorage.getItem('orbit-last-session')
      if (!lastId) return
      const { sessions, openSession } = useAppStore.getState()
      const target = sessions.find((s) => !s.is_history && s.id === lastId)
      if (target) void openSession(target)
    })
    const id = setInterval(() => void refreshSessions(), intervalMs)
    return () => clearInterval(id)
    // refreshSessions is a stable Zustand action — intentional empty dep array
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
}
