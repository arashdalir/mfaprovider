import { isTheme, type Theme } from "./theme";

const LABELS: Record<Theme, string> = { system: "System", light: "Light", dark: "Dark" };

export function ThemeSwitch({
  theme,
  onChange,
}: {
  theme: Theme;
  onChange: (theme: Theme) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm">
      Theme
      <select
        className="rounded-md border bg-background px-2 py-1"
        value={theme}
        onChange={(event) => {
          if (isTheme(event.target.value)) {
            onChange(event.target.value);
          }
        }}
      >
        {Object.entries(LABELS).map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </select>
    </label>
  );
}
