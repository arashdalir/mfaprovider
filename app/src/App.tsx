import { OpenVault } from "@/features/session/OpenVault";
import { SessionGate } from "@/features/session/SessionGate";
import { useTheme } from "@/features/theme/useTheme";

export function App() {
  const { theme, setTheme } = useTheme();
  return (
    <div className="mx-auto flex min-h-screen w-full max-w-2xl flex-col gap-4 p-4">
      <header>
        <h1 className="text-xl font-semibold">MFA Provider</h1>
      </header>
      <main className="flex-1">
        <SessionGate>
          {({ lock }) => (
            <OpenVault onLock={() => void lock()} theme={theme} onThemeChange={setTheme} />
          )}
        </SessionGate>
      </main>
    </div>
  );
}
