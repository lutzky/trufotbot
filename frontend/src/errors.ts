// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

interface BackendError {
  body: {
    message: string
  }
}

function isBackendError(error: unknown): error is BackendError {
  if (typeof error !== 'object' || error === null) {
    return false
  }
  if (!('body' in error)) {
    return false
  }
  const body = (error as { body: unknown }).body
  if (typeof body !== 'object' || body === null) {
    return false
  }
  if (!('message' in body)) {
    return false
  }
  return typeof (body as { message: unknown }).message === 'string'
}

/**
 * Extracts a user-friendly error message from an error object.
 * It checks for a backend-provided message and falls back to a generic one.
 * It also logs the full error to the console for debugging.
 *
 * @param error The error object, typically from a catch block.
 * @returns A string containing the error message.
 */
export function getErrorMessage(error: unknown): string {
  console.error('An error occurred:', error)

  // Check if the error object has a specific structure from our backend API client
  if (isBackendError(error)) {
    return error.body.message
  }

  // Handle standard Error objects
  if (error instanceof Error) {
    // Provide a more user-friendly message for common network errors
    if (error.message.toLowerCase().includes('failed to fetch')) {
      return 'Could not connect to the server. Please check your network connection.'
    }
    return error.message
  }

  // Fallback for other types of errors (e.g., strings thrown)
  if (typeof error === 'string' && error.trim().length > 0) {
    return error
  }

  // Generic fallback message
  return 'An unexpected error occurred. Please try again later.'
}
