import { ref, watch } from 'vue'

const username = ref<string | null>(localStorage.getItem('username'))

export function useUsername() {
  return username
}

export function setUsername(newUsername: string) {
  username.value = newUsername
}

watch(username, (newValue) => {
  if (newValue) {
    localStorage.setItem('username', newValue)
  } else {
    localStorage.removeItem('username')
  }
})
