import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface SettingsState {
  theme: 'light' | 'dark'
  refreshInterval: number
  setTheme: (theme: 'light' | 'dark') => void
  setRefreshInterval: (interval: number) => void
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: 'light',
      refreshInterval: 0,
      setTheme: (theme) => set({ theme }),
      setRefreshInterval: (interval) => set({ refreshInterval: interval }),
    }),
    {
      name: 'chronodb-settings',
    }
  )
)
