// Copyright 2026 ZenohX Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/**
 * Automatic Protobuf schema discovery.
 *
 * Publishers that advertise `application/protobuf` serve their schema on two
 * side channels:
 *
 *   <topic>/@schema   -> JSON { type_name, schema_digest, schema_keyexpr?, ... }
 *   schemas/<digest>  -> serialized google.protobuf.FileDescriptorSet
 *
 * On first sight of a Protobuf key this store follows those links, builds a
 * protobuf.Root from the descriptor set and remembers the message type for the
 * key. No .proto files are needed. Each descriptor set is registered in the
 * Protobuf Schema Manager (useProtoStore) as a discovered schema, so it is
 * reused across restarts instead of being fetched again.
 */

import { create } from 'zustand';
import type protobuf from 'protobufjs';
import { runQuery } from '../lib/tauri';
import { rootFromDescriptorSet } from '../lib/protoDescriptor';
import { useProtoStore } from './protoStore';
import type { ReplySample } from '../types/zenoh';

export { rootFromDescriptorSet };

export const PROTOBUF_MIME = 'application/protobuf';
export const SCHEMA_META_SUFFIX = '/@schema';
export const DEFAULT_SCHEMA_QUERY_TIMEOUT_MS = 2000;
/** Failed lookups are retried on a later sample once this long has passed. */
export const SCHEMA_RETRY_AFTER_MS = 30_000;
/** Upper bound on concurrent `@schema` lookups, so `**` subscriptions don't flood the network. */
const MAX_CONCURRENT_LOOKUPS = 4;

export type SchemaDiscoveryStatus = 'pending' | 'resolved' | 'not_found' | 'error';

export interface SchemaDiscoveryEntry {
  keyExpr: string;
  status: SchemaDiscoveryStatus;
  typeName?: string;
  digest?: string;
  schemaKeyExpr?: string;
  error?: string;
  updatedAt: number;
}

export interface SchemaMeta {
  typeName: string;
  digest: string;
  schemaKeyExpr: string;
}

interface SchemaDiscoveryState {
  entries: Record<string, SchemaDiscoveryEntry>;
  queryTimeoutMs: number;

  /** Starts a lookup for `keyExpr` unless one is cached, in flight, or recently failed. */
  request: (sessionId: string, keyExpr: string) => void;
  /** Forces a new lookup for `keyExpr`, ignoring the cached result. */
  retry: (sessionId: string, keyExpr: string) => Promise<void>;
  /** Resolves `keyExpr` and waits for the result. */
  resolve: (sessionId: string, keyExpr: string) => Promise<SchemaDiscoveryEntry>;
  getDecoder: (keyExpr: string) => { root: protobuf.Root; typeName: string } | null;
  clear: () => void;
}

// Compiled roots per schema digest: different revisions of the same message
// name live in separate roots so they cannot collide.
const rootsByDigest = new Map<string, protobuf.Root>();
const pendingRoots = new Map<string, Promise<protobuf.Root | null>>();
const inFlight = new Map<string, Promise<SchemaDiscoveryEntry>>();

let activeLookups = 0;
const lookupQueue: Array<() => void> = [];

async function withLookupSlot<T>(fn: () => Promise<T>): Promise<T> {
  if (activeLookups >= MAX_CONCURRENT_LOOKUPS) {
    // The finishing lookup hands its slot over directly (see below).
    await new Promise<void>((resolve) => lookupQueue.push(resolve));
  } else {
    activeLookups++;
  }
  try {
    return await fn();
  } finally {
    const next = lookupQueue.shift();
    if (next) next();
    else activeLookups--;
  }
}

/** Encoding without its schema suffix, e.g. 'application/protobuf'. */
export function mimeOf(encoding: string | null | undefined): string {
  return String(encoding ?? '').split(';', 1)[0].trim().toLowerCase();
}

export function isProtobufEncoding(encoding: string | null | undefined): boolean {
  return mimeOf(encoding) === PROTOBUF_MIME;
}

/** Parses the JSON served on `<topic>/@schema`. */
export function parseSchemaMeta(json: unknown): SchemaMeta {
  if (!json || typeof json !== 'object') {
    throw new Error('schema meta data is not a JSON object');
  }
  const meta = json as Record<string, unknown>;
  if (typeof meta.type_name !== 'string' || !meta.type_name) {
    throw new Error("schema meta data has no 'type_name'");
  }
  if (typeof meta.schema_digest !== 'string' || !meta.schema_digest) {
    throw new Error("schema meta data has no 'schema_digest'");
  }
  // "sha256:abcd…" -> "abcd…"
  const rawDigest = meta.schema_digest;
  const colon = rawDigest.indexOf(':');
  const digest = colon >= 0 ? rawDigest.slice(colon + 1) : rawDigest;
  const schemaKeyExpr =
    typeof meta.schema_keyexpr === 'string' && meta.schema_keyexpr
      ? meta.schema_keyexpr
      : `schemas/${digest}`;
  return { typeName: meta.type_name, digest, schemaKeyExpr };
}

function firstOk(replies: ReplySample[]): ReplySample | null {
  return replies.find((r) => !r.is_err) ?? null;
}

async function getFirstOk(sessionId: string, selector: string, timeoutMs: number) {
  const replies = await runQuery(sessionId, selector, 'best_matching', timeoutMs);
  return firstOk(replies ?? []);
}

/**
 * Returns the compiled root for a digest: from memory, from a schema already
 * registered in the Schema Manager, or by fetching `schemaKeyExpr` (which then
 * registers it there).
 */
async function fetchRoot(
  sessionId: string,
  meta: SchemaMeta,
  topic: string,
  timeoutMs: number
): Promise<protobuf.Root | null> {
  const { digest, schemaKeyExpr, typeName } = meta;
  const cached = rootsByDigest.get(digest);
  if (cached) return cached;

  let pending = pendingRoots.get(digest);
  if (!pending) {
    pending = (async () => {
      const proto = useProtoStore.getState();
      const registered = proto.findSchemaByDigest(digest);
      let root = registered ? proto.getCompiledRoot(registered.id) : null;
      if (!root) {
        const sample = await getFirstOk(sessionId, schemaKeyExpr, timeoutMs);
        if (!sample) return null;
        const bytes = new Uint8Array(sample.payload);
        root = rootFromDescriptorSet(bytes);
        // Validate before registering so a broken set never reaches the manager.
        root.lookupType(typeName.replace(/^\./, ''));
        proto.upsertDiscoveredSchema({ digest, schemaKeyExpr, topic, typeName, descriptorSet: bytes });
      }
      rootsByDigest.set(digest, root);
      return root;
    })().finally(() => pendingRoots.delete(digest));
    pendingRoots.set(digest, pending);
  }
  return pending;
}

async function lookup(sessionId: string, keyExpr: string, timeoutMs: number): Promise<SchemaDiscoveryEntry> {
  const now = () => Date.now();
  const metaKey = keyExpr + SCHEMA_META_SUFFIX;

  const metaSample = await getFirstOk(sessionId, metaKey, timeoutMs);
  if (!metaSample) {
    return { keyExpr, status: 'not_found', error: `No schema meta data on '${metaKey}'`, updatedAt: now() };
  }

  const text = new TextDecoder('utf-8').decode(new Uint8Array(metaSample.payload));
  const meta = parseSchemaMeta(JSON.parse(text));
  const base = { keyExpr, typeName: meta.typeName, digest: meta.digest, schemaKeyExpr: meta.schemaKeyExpr };

  const root = await fetchRoot(sessionId, meta, keyExpr, timeoutMs);
  if (!root) {
    return { ...base, status: 'not_found', error: `No schema served on '${meta.schemaKeyExpr}'`, updatedAt: now() };
  }

  // Fail early if the advertised type isn't in the descriptor set.
  root.lookupType(meta.typeName.replace(/^\./, ''));
  // Record this topic (and type) on the schema shown in the Schema Manager.
  useProtoStore.getState().upsertDiscoveredSchema({
    digest: meta.digest,
    schemaKeyExpr: meta.schemaKeyExpr,
    topic: keyExpr,
    typeName: meta.typeName,
  });
  return { ...base, status: 'resolved', updatedAt: now() };
}

export const useSchemaDiscoveryStore = create<SchemaDiscoveryState>()((set, get) => ({
  entries: {},
  queryTimeoutMs: DEFAULT_SCHEMA_QUERY_TIMEOUT_MS,

  request: (sessionId, keyExpr) => {
    const entry = get().entries[keyExpr];
    if (entry) {
      if (entry.status === 'pending' || entry.status === 'resolved') return;
      // Negative results are cached too, otherwise every sample of an
      // undescribed key would trigger a new pair of queries.
      if (Date.now() - entry.updatedAt < SCHEMA_RETRY_AFTER_MS) return;
    }
    void get().resolve(sessionId, keyExpr);
  },

  retry: async (sessionId, keyExpr) => {
    set((s) => {
      const entries = { ...s.entries };
      delete entries[keyExpr];
      return { entries };
    });
    await get().resolve(sessionId, keyExpr);
  },

  resolve: (sessionId, keyExpr) => {
    watchRemovedSchemas();
    const existing = inFlight.get(keyExpr);
    if (existing) return existing;

    set((s) => ({
      entries: { ...s.entries, [keyExpr]: { keyExpr, status: 'pending', updatedAt: Date.now() } },
    }));

    const promise = withLookupSlot(() => lookup(sessionId, keyExpr, get().queryTimeoutMs))
      .catch(
        (err: unknown): SchemaDiscoveryEntry => ({
          keyExpr,
          status: 'error',
          error: err instanceof Error ? err.message : String(err),
          updatedAt: Date.now(),
        })
      )
      .then((entry) => {
        if (entry.status === 'resolved') {
          console.info(
            `[schema] resolved '${keyExpr}': type=${entry.typeName} digest=${entry.digest?.slice(0, 12)}… via ${entry.schemaKeyExpr}`
          );
        } else {
          console.warn(`[schema] lookup failed for '${keyExpr}': ${entry.error}`);
        }
        set((s) => ({ entries: { ...s.entries, [keyExpr]: entry } }));
        return entry;
      })
      .finally(() => inFlight.delete(keyExpr));

    inFlight.set(keyExpr, promise);
    return promise;
  },

  getDecoder: (keyExpr) => {
    const entry = get().entries[keyExpr];
    if (entry?.status !== 'resolved' || !entry.digest || !entry.typeName) return null;
    const root = rootsByDigest.get(entry.digest);
    return root ? { root, typeName: entry.typeName } : null;
  },

  clear: () => {
    rootsByDigest.clear();
    pendingRoots.clear();
    inFlight.clear();
    set({ entries: {} });
  },
}));

// Deleting a discovered schema in the Schema Manager forgets it here too, so the
// next sample on its topics fetches and registers it again. Subscribed lazily:
// protoStore and this module import each other (via formatters), so useProtoStore
// may not be initialised yet while this module is evaluated.
let unsubscribeProtoStore: (() => void) | null = null;
function watchRemovedSchemas() {
  if (unsubscribeProtoStore) return;
  unsubscribeProtoStore = useProtoStore.subscribe((state, prev) => {
    if (state.schemas === prev.schemas) return;
    const present = new Set(state.schemas.map((s) => s.discovery?.digest).filter(Boolean));
    const removed = prev.schemas
      .map((s) => s.discovery?.digest)
      .filter((d): d is string => !!d && !present.has(d));
    if (removed.length === 0) return;

    for (const digest of removed) rootsByDigest.delete(digest);
    useSchemaDiscoveryStore.setState((s) => ({
      entries: Object.fromEntries(
        Object.entries(s.entries).filter(([, e]) => !e.digest || !removed.includes(e.digest))
      ),
    }));
  });
}
