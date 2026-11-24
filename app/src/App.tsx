import { OpenVault } from "@/features/session/OpenVault";
import { SessionGate } from "@/features/session/SessionGate";
import { ThemeSwitch } from "@/features/theme/ThemeSwitch";
import { useTheme } from "@/features/theme/useTheme";

export function App() {
  const { theme, setTheme } = useTheme();
  return (
    <div className="mx-auto flex min-h-screen w-full max-w-2xl flex-col gap-4 p-4">
      <header className="flex items-center justify-between gap-2">
        <h1 className="text-xl font-semibold">MFA Provider</h1>
        <ThemeSwitch theme={theme} onChange={setTheme} />
      </header>
      <main className="flex-1">
        <SessionGate>{({ lock }) => <OpenVault onLock={() => void lock()} />}</SessionGate>
      </main>
    </div>
  );
}
