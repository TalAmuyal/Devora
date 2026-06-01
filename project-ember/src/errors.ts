const errors: string[] = [];

/** Record an error. Called by error-handling code throughout the app. */
export function recordError(message: string): void {
  errors.push(message);
}

/** Return all errors recorded since the last scrape, then clear the list. */
export function scrapeErrors(): string[] {
  return errors.splice(0);
}
