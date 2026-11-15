import { listen } from "@tauri-apps/api/event";
import { isTauri } from "./env";
import {
  EVENTS,
  type ConnectionStatus,
  type LatencyUpdate,
  type LogLine,
  type Notice,
  type Snapshot,
  type TrafficTick,
} from "./types";

export interface EventMap {
  snapshot: Snapshot;
  status: ConnectionStatus;
  traffic: TrafficTick;
  log: LogLine;
  latency: LatencyUpdate;
  notice: Notice;
}
export type EventName = keyof EventMap;
export type Unlisten = () => void;

type AnyHandler = (payload: unknown) => void;
const mockHandlers = new Map<EventName, Set<AnyHandler>>();

/** Used by the mock backend to emit events with the same shape as Tauri's. */
export function emitMock<K extends EventName>(name: K, payload: EventMap[K]): void {
  mockHandlers.get(name)?.forEach((h) => h(payload));
}

/** Subscribe to a backend event. Works identically against Tauri and the mock. */
export async function on<K extends EventName>(
  name: K,
  handler: (payload: EventMap[K]) => void,
): Promise<Unlisten> {
  if (isTauri) {
    return listen<EventMap[K]>(EVENTS[name], (e) => handler(e.payload));
  }
  let set = mockHandlers.get(name);
  if (!set) {
    set = new Set();
    mockHandlers.set(name, set);
  }
  const h = handler as AnyHandler;
  set.add(h);
  return () => {
    set.delete(h);
  };
}
