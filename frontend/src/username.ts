export function getUsername(): string | null {
  return localStorage.getItem('username')
}

export function setUsername(newUsername: string) {
  localStorage.setItem('username', newUsername)
}
