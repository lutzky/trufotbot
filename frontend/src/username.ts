export function getUsername(): string | null {
  return localStorage.getItem('username') || null
}

export function setUsername(newUsername: string) {
  localStorage.setItem('username', newUsername)
}
