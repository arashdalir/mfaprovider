import { useId } from "react";

/**
 * The user name a password manager files the form's password under. Shown rather than hidden:
 * managers may skip hidden fields, and the user needs the name to find the entry again.
 */
export function CredentialName({ name }: { name: string }) {
  const id = useId();
  return (
    <div className="flex flex-col text-xs text-muted-foreground">
      <label htmlFor={id}>Name in password managers</label>
      <input
        id={id}
        name="username"
        autoComplete="username"
        readOnly
        value={name}
        className="w-full bg-transparent font-mono outline-none focus-visible:ring-1 focus-visible:ring-ring"
      />
    </div>
  );
}
