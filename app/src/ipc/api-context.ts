import { createContext, useContext } from "react";

import { ipc } from "./client";

/** What the UI may ask of the core. Tests provide a fake through `ApiContext.Provider`. */
export type VaultApi = typeof ipc;

export const ApiContext = createContext<VaultApi>(ipc);

export function useApi(): VaultApi {
  return useContext(ApiContext);
}
