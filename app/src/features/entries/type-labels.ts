const LABELS: Record<string, string> = {
  totp: "Time-based (TOTP)",
  hotp: "Counter-based (HOTP)",
  steam: "Steam Guard",
};

export function typeLabel(id: string): string {
  return LABELS[id] ?? id;
}
