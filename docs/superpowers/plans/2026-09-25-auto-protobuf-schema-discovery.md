# Automatic Protobuf Schema Discovery via `<key>/@schema` — Implementation Plan

> **Status:** Draft. The format of the JSON object returned by `<key>/@schema` is still
> **TBD** (to be provided). Every place that depends on it is marked **[FORMAT-TBD]**.

**Goal:** When a subscription receives a sample whose encoding is Protobuf and no
topic→schema mapping exists for its key, ZenohX automatically issues a Zenoh `get` on
`<sample key expression>/@schema`, reads the returned JSON descriptor, fetches the
referenced `.proto` definition, registers it in the Protobuf schema registry, and creates a
topic mapping so the sample (and every later sample on that key) decodes to JSON.

**Architecture:** Entirely in the frontend (TypeScript/Zustand). A new
`schemaDiscoveryStore` sits between sample ingestion (`messageStore`) and the schema
registry (`protoStore`). It reuses the existing `runQuery` Tauri command (`query_get`) and
the existing `addSchema` / `addMapping` APIs. No Rust changes are required unless the
descriptor points at a location the webview cannot reach (see Task 4).

**Tech Stack:** TypeScript, React, Zustand, protobufjs, Tauri `invoke`, `node:test`.

---

## Current state (relevant code)

| Concern | Location |
|---|---|
| Batched sample ingestion (every incoming sample passes here) | `src/stores/messageStore.ts:264-333` (`onZenohSamplesBatched`) |
| Raw encoding string → `EncodingType` | `src/lib/formatters.ts:596` (`normalizeEncoding`) |
| Raw encoding string from Zenoh | `src-tauri/src/zenoh/pubsub.rs:103` (`sample.encoding().to_string()`) |
| Zenoh `get` from the frontend | `src/lib/tauri.ts:228` (`runQuery`) |
| Schema registry + mappings (persisted) | `src/stores/protoStore.ts`, `src/types/proto.ts` |
| Proto parse/compile | `src/lib/protobufEngine.ts` (`parseProtoSchema`) |
| Decode on display | `src/lib/formatters.ts:255` (`tryFormatProtobuf`), `src/components/viewer/PayloadViewer.tsx:381` |

Observations that shape the plan:

1. Zenoh 1.x encodings may carry a schema suffix, e.g. `application/protobuf;my.pkg.Pose`.
   `normalizeEncoding` currently only matches the exact string, so such samples are **not**
   recognised as Protobuf today. This must be fixed first.
2. Decoding is lazy and driven by `findMappingForKey`. Once a mapping exists, all existing
   viewers pick it up automatically (they subscribe to `mappings` in the store), so already
   received messages re-render decoded without extra work.
3. `runQuery` returns `ReplySample[]` with raw payload bytes — the `@schema` reply is JSON
   text that we `TextDecoder`-decode and `JSON.parse`.

---

## Design

### Trigger condition

For each incoming sample in the batch handler:

```
isProtobuf(rawEncoding) && !protoStore.findMappingForKey(sample.key_expr)
  → schemaDiscovery.request(sessionId, sample.key_expr, encodingSchemaSuffix)
```

The check is O(1)-ish and must never block ingestion; `request()` is fire-and-forget.

### Which key to query

The concrete sample key (not the subscription's wildcard), i.e.
`robot/1/pose` → `get("robot/1/pose/@schema")`. The `@` chunk is a verbatim chunk in
Zenoh 1.x, so it is not matched by `**` subscriptions and will not echo back into feeds.

### De-duplication, caching and back-off

`schemaDiscoveryStore` keeps a per-`(sessionId, keyExpr)` state map:

```ts
type DiscoveryStatus = 'pending' | 'resolved' | 'not_found' | 'error';
interface DiscoveryEntry {
  keyExpr: string;
  sessionId: string;
  status: DiscoveryStatus;
  descriptor?: SchemaDescriptor;   // [FORMAT-TBD]
  protoId?: string;
  messageTypeName?: string;
  error?: string;
  lastAttempt: number;
  attempts: number;
}
```

- `pending` → further samples on the key are ignored (no query storm at high rates).
- `not_found` / `error` → retry only after back-off (e.g. 30 s, doubling, capped at 10 min).
- `resolved` → nothing to do; the mapping now exists so the trigger no longer fires.
- Global concurrency limit (e.g. 4 in-flight `@schema` gets) with a small queue, so a
  `**` subscription over hundreds of keys does not flood the network.
- Descriptor-level cache: if two keys resolve to the same schema source (same URL/id —
  **[FORMAT-TBD]**), fetch and compile the `.proto` once and only add a second mapping.

### Resolving the descriptor → `.proto` source

`@schema` returns JSON describing *where* to get the definition. Pipeline:

1. `runQuery(sessionId, `${key}/@schema`, 'best_matching', timeout)`; take the first `ok`
   reply; decode UTF-8; `JSON.parse`; validate with `parseSchemaDescriptor()` **[FORMAT-TBD]**.
2. `loadProtoSource(descriptor)` returns `{ files: Array<{name, content}>, messageType }`.
   Planned source kinds (final set depends on the format):
   - **inline** — proto text embedded in the JSON;
   - **zenoh key** — another key expression to `get` (e.g. a schema-registry queryable);
   - **http(s) URL** — fetched via Rust (Task 4) to avoid webview CORS/CSP limits;
   - **imports** — multiple files / dependencies resolved recursively with a depth limit.
3. Register: `protoStore.addSchema(name, content)` (or reuse an existing schema whose
   `rawContent` is identical, to avoid duplicates across restarts).
4. Determine message type: from the descriptor, else from the encoding suffix
   (`application/protobuf;<type>`), else — if the file defines exactly one message — that one.
   Otherwise mark `error: "ambiguous message type"` and surface it in the UI.
5. Map: `protoStore.addMapping(<keyExpr>, protoId, messageType)`.

### Mapping granularity & provenance

Mappings are created for the **exact sample key** (safest; different keys under one
wildcard may use different types). Extend `ProtoTopicMapping` / `ProtoDefinition` with an
optional provenance field so auto-discovered entries are distinguishable and refreshable:

```ts
source?: 'manual' | 'auto';
origin?: { keyExpr: string; descriptor: unknown; fetchedAt: number }; // on ProtoDefinition
```

Manual mappings always win: discovery never runs if any mapping (manual or auto) already
matches, and never overwrites a manual schema.

### Settings

Add to `settingsStore`:
- `autoSchemaDiscovery: boolean` (default `true`)
- `schemaDiscoveryTimeoutMs: number` (default `2000`)
- (if URL sources are supported) `allowRemoteSchemaUrls: boolean` — a security toggle,
  since fetching a URL advertised by an arbitrary network peer is an outbound request.

---

## Tasks

### Task 1: Robust Protobuf encoding detection
- Modify `src/lib/formatters.ts` `normalizeEncoding` to split on `;` and treat
  `application/protobuf;*`, `application/x-protobuf;*`, `protobuf;*` as `'protobuf'`.
- Add `parseEncoding(raw): { mime: string; schema?: string }` helper.
- Keep the raw encoding string on `MessageItem` (new optional `rawEncoding`) so the
  schema suffix is available downstream.
- Tests: `tests/…formatters…` cases for suffixed/unsuffixed/case variants.

### Task 2: Schema descriptor parsing — **[FORMAT-TBD]**
- New `src/lib/schemaDescriptor.ts`: `SchemaDescriptor` type + `parseSchemaDescriptor(json)`
  that validates and normalises into a discriminated union of source kinds.
- Tests with fixture JSON objects (valid, missing fields, unknown kind).

### Task 3: Discovery store
- New `src/stores/schemaDiscoveryStore.ts` implementing: `request()`, state map,
  de-dup, concurrency queue, back-off, descriptor cache, `retry(key)`, `clear()`.
- Uses `runQuery` for `@schema` and for zenoh-key sources.
- On success calls `protoStore.addSchema` / `addMapping` (with `source: 'auto'`).
- Not persisted except the resulting schemas/mappings (already persisted by `protoStore`).
- Unit tests with `runQuery` mocked: single fetch for N samples, back-off, ambiguous type,
  manual mapping precedence, same-source dedupe.

### Task 4: Remote source fetching (only if the format allows URLs)
- New Tauri command `fetch_schema_source(url)` in `src-tauri/src/commands/` using
  `reqwest` (http/https only, size cap e.g. 1 MiB, timeout), gated by
  `allowRemoteSchemaUrls`. Wrapper in `src/lib/tauri.ts`.

### Task 5: Wire into ingestion
- In `src/stores/messageStore.ts` batch handler, after building each `MessageItem`,
  call the trigger check (guarded by `autoSchemaDiscovery`). Also apply to query
  replies in `queryStore` for consistency (optional, same helper).

### Task 6: UI feedback
- `PayloadViewer`: when a Protobuf sample has no mapping, show status from the discovery
  store — "Fetching schema…", "Schema not published (`<key>/@schema`)", error + Retry.
- Proto Schema Manager: badge auto-discovered schemas/mappings ("auto"), show origin key
  and fetch time, allow "Refresh" and converting to manual.
- Settings panel toggles from the Settings section.

### Task 7: MCP parity (optional)
- Expose discovery status in MCP subscription/message tools so agents get decoded JSON
  and can trigger `retry` for a key.

### Task 8: Docs & changelog
- README feature bullet, `CHANGELOG.md` entry, short section in the Protobuf docs
  describing the `@schema` convention and the JSON format.

---

## Open questions (to settle with the `@schema` format)

1. Exact JSON shape and which source kinds exist (inline / zenoh key / URL / other).
2. Does the descriptor name the message type, or must it come from the encoding suffix?
3. Can one descriptor reference multiple `.proto` files (imports)? How are import paths
   resolved?
4. Is there a version/hash field usable for cache invalidation and change detection?
5. Should the `@schema` query target the exact key, or also fall back to parent keys
   (e.g. `robot/1/@schema`, `robot/@schema`) when the exact key has no answer?
