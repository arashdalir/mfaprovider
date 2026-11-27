export const MIN_PASSWORD_LENGTH = 8;

/** The first problem with a new device password, or null if it is acceptable. */
export function newPasswordProblem(password: string, confirmation: string): string | null {
  if (password.length < MIN_PASSWORD_LENGTH) {
    return `Use at least ${MIN_PASSWORD_LENGTH} characters.`;
  }
  if (password !== confirmation) {
    return "The two passwords do not match.";
  }
  return null;
}
