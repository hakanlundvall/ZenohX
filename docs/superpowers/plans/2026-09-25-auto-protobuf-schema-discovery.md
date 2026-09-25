# Automatic Protobuf Schema Discovery via `<key>/@schema`

> **Status:** Implemented. Mirrors the reference Python tool `tools/zenoh_tap.py`
> (`SchemaResolver`) from the publisher side.

**Goal:** Samples whose encoding is `application/protobuf` decode to JSON without the user
registering any `.proto` files. On first sight of a key, ZenohX follows the schema side
channels the publisher serves, builds the message type at runtime and decodes every
following sample on that key.

## Protocol

```
<topic>/@schema   -> JSON { type_name, schema_digest, schema_keyexpr?, ... }
schemas/<digest>  -> serialized google.protobuf.FileDescriptorSet
```

- `schema_digest` may carry an algorithm prefix (`sha256:abcd…`); everything after the
  first `:` is the digest.
- `schema_keyexpr` is optional and defaults to `schemas/<digest>`.
- Only the MIME part of the sample encoding counts, so
  `application/protobuf;pkg.Msg` is protobuf too.

## Design

| Concern | Where |
|---|---|
| Resolver, caches, status per key | `src/stores/schemaDiscoveryStore.ts` |
| Trigger on incoming samples | `src/stores/messageStore.ts` (batched sample listener) |
| Decoder lookup for a key (manual mapping, else discovered) | `resolveProtoDecoderForKey` in `src/lib/formatters.ts` |
| Suffixed encodings (`application/protobuf;…`) | `normalizeEncoding` in `src/lib/formatters.ts` |
| Feed previews | `src/components/pubsub/MessageList.tsx` |
| Inspector: type picker, status, Retry | `src/components/viewer/PayloadViewer.tsx` |

Behaviour, matching the Python `SchemaResolver`:

- **Descriptor → types:** `protobuf.Root.fromDescriptor(bytes, { keepCase: true })` from
  `protobufjs/ext/descriptor`. `keepCase` keeps proto field names, like
  `preserving_proto_field_name=True`. Decoding keeps ZenohX's existing options: defaults
  are included, enums are shown as names, and 64-bit integers as strings.
- **One root per digest:** different revisions of the same message name don't collide.
  Several keys that share a digest fetch the descriptor set only once, and concurrent
  fetches of the same digest are deduplicated.
- **Per-key cache, negative results included:** each key's status is `pending`,
  `resolved`, `not_found` or `error`. While a lookup is pending or after it resolved, later
  samples don't query again. A failed lookup is retried only on a sample arriving
  ≥ 30 s later, or through **Retry** in the payload inspector. (The Python tool caches
  failures forever. A GUI session lives longer, and publishers may come up after the
  subscriber.)
- **Doesn't block ingestion:** lookups run asynchronously, at most 4 at once, so a `**`
  subscription over many keys doesn't flood the network. Already-received messages
  re-render decoded once the schema resolves.
- **First OK reply wins:** error replies are skipped. Queries use `best_matching` and a
  2 s timeout, the Python default.
- **Manual mappings win:** a topic mapping configured in the Schema Manager takes
  precedence over a discovered schema.
- Discovered schemas stay in memory and are rediscovered each session. They aren't
  added to the persisted Schema Manager.

## Tests

- `tests/frontend/schemaDiscovery.test.ts`: meta-data parsing and MIME handling. It
  decodes a `FileDescriptorSet` produced by Python `protobuf`, with a
  `google/protobuf/timestamp.proto` import, together with a Python-serialized message. It
  also covers the custom `schema_keyexpr`, the single fetch per digest, request
  deduplication, the negative cache and retry, a missing descriptor set, an unknown type,
  error replies, and manual-mapping precedence.
- `tests/frontend/pubsub.test.ts`: end to end through the batched sample listener.

## Possible follow-ups

- Apply the same discovery to query replies (`queryStore`) and to MCP tool output.
- A settings toggle and a configurable timeout.
- "Save to Schema Manager" for a discovered schema. That needs `.proto` text, or storing
  descriptor sets in the registry.
