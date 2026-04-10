import { create } from 'zustand'

interface UserState {
  username: string | null
  token: string | null
  setUsername: (username: string | null) => void
  setToken: (token: string | null) => void
  logout: () => void
}

export const useUserStore = create<UserState>((set) => ({
  username: null,
  token: null,
  setUsername: (username) => set({ username }),
  setToken: (token) => set({ token }),
  logout: () => set({ username: null, token: null }),
}))
