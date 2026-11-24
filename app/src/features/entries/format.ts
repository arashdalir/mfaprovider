/** "482913" → "482 913" and "48291357" → "4829 1357" for easier reading; other codes as they are. */
export function formatCode(code: string): string {
  if (!/^\d+$/.test(code) || code.length % 2 !== 0) {
    return code;
  }
  const half = code.length / 2;
  return `${code.slice(0, half)} ${code.slice(half)}`;
}
