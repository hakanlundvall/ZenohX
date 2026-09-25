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

import { test, describe, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

let mockInvokeHandler: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> = async () => [];

// @ts-expect-error Mocking global window
globalThis.window = globalThis;
// @ts-expect-error Mocking tauri internals
globalThis.window.__TAURI_INTERNALS__ = {
  invoke: async (cmd: string, args?: Record<string, unknown>) => mockInvokeHandler(cmd, args),
  transformCallback: (cb: unknown) => cb,
};

import {
  useSchemaDiscoveryStore,
  mimeOf,
  isProtobufEncoding,
  parseSchemaMeta,
  rootFromDescriptorSet,
} from '../../src/stores/schemaDiscoveryStore';
import { useProtoStore } from '../../src/stores/protoStore';
import { getPayloadSnippet, normalizeEncoding, tryFormatProtobuf } from '../../src/lib/formatters';
import type { ReplySample } from '../../src/types/zenoh';

// google.protobuf.FileDescriptorSet serialized by Python protobuf: rcsio/pose.proto
// (package rcs.io, message Pose, enum Kind) importing google/protobuf/timestamp.proto.
const POSE_DESCRIPTOR_SET = Buffer.from(
  'Cv8BCh9nb29nbGUvcHJvdG9idWYvdGltZXN0YW1wLnByb3RvEg9nb29nbGUucHJvdG9idWYiOwoJVGltZXN0YW1wEhgKB3NlY29uZHMYASABKANSB3NlY29uZHMSFAoFbmFub3MYAiABKAVSBW5hbm9zQoUBChNjb20uZ29vZ2xlLnByb3RvYnVmQg5UaW1lc3RhbXBQcm90b1ABWjJnb29nbGUuZ29sYW5nLm9yZy9wcm90b2J1Zi90eXBlcy9rbm93bi90aW1lc3RhbXBwYvgBAaICA0dQQqoCHkdvb2dsZS5Qcm90b2J1Zi5XZWxsS25vd25UeXBlc2IGcHJvdG8zCoACChByY3Npby9wb3NlLnByb3RvEgZyY3MuaW8aH2dvb2dsZS9wcm90b2J1Zi90aW1lc3RhbXAucHJvdG8imgEKBFBvc2USEwoFcG9zX3gYASABKAFSBHBvc1gSGQoIZnJhbWVfaWQYAiABKAlSB2ZyYW1lSWQSGgoEa2luZBgDIAEoDjIMLnJjcy5pby5LaW5kEikKBXN0YW1wGAQgASgLMhouZ29vZ2xlLnByb3RvYnVmLlRpbWVzdGFtcBINCgVjb3VudBgFIAEoAxIMCgR2YWxzGAYgAygCKh4KBEtpbmQSCgoGS0lORF9BEAASCgoGS0lORF9CEAFiBnByb3RvMw==',
  'base64'
);
// Pose(pos_x=1.5, frame_id="map", kind=KIND_B, stamp={seconds: 5}, count=123, vals=[1, 2]) serialized by Python.
const POSE_BYTES = Array.from(Buffer.from('CQAAAAAAAPg/EgNtYXAYASICCAUoezIIAACAPwAAAEA=', 'base64'));

const DIGEST = 'a1b2c3d4e5f60718293a4b5c6d7e8f90';
const SESSION = 'sess-1';

function reply(keyExpr: string, payload: Uint8Array | number[], isErr = false): ReplySample {
  return {
    session_id: SESSION,
    key_expr: keyExpr,
    payload: Array.from(payload),
    encoding: 'application/json',
    latency_ms: 1,
    timestamp: Date.now(),
    is_err: isErr,
  };
}

function json(obj: unknown): number[] {
  return Array.from(new TextEncoder().encode(JSON.stringify(obj)));
}

/** Serves `@schema` meta data and descriptor sets, recording every selector queried. */
function serve(meta: Record<string, unknown>, sets: Record<string, Uint8Array>): string[] {
  const queried: string[] = [];
  mockInvokeHandler = async (cmd, args) => {
    assert.equal(cmd, 'query_get');
    const selector = String(args?.selector);
    queried.push(selector);
    if (selector.endsWith('/@schema')) {
      const topic = selector.slice(0, -'/@schema'.length);
      return meta[topic] ? [reply(selector, json(meta[topic]))] : [];
    }
    return sets[selector] ? [reply(selector, sets[selector])] : [];
  };
  return queried;
}

const POSE_META = { type_name: 'rcs.io.Pose', schema_digest: `sha256:${DIGEST}` };

describe('schema discovery helpers', () => {
  test('mimeOf strips the schema suffix and normalises case', () => {
    assert.equal(mimeOf('application/protobuf;rcs.io.Pose'), 'application/protobuf');
    assert.equal(mimeOf(' Application/Protobuf '), 'application/protobuf');
    assert.equal(mimeOf(undefined), '');
    assert.equal(isProtobufEncoding('application/protobuf;x'), true);
    assert.equal(isProtobufEncoding('application/json'), false);
  });

  test('normalizeEncoding recognises suffixed protobuf encodings', () => {
    assert.equal(normalizeEncoding('application/protobuf;rcs.io.Pose'), 'protobuf');
    assert.equal(normalizeEncoding('application/json;foo'), 'json');
  });

  test('parseSchemaMeta strips the digest algorithm and defaults the schema key', () => {
    assert.deepEqual(parseSchemaMeta(POSE_META), {
      typeName: 'rcs.io.Pose',
      digest: DIGEST,
      schemaKeyExpr: `schemas/${DIGEST}`,
    });
    assert.equal(parseSchemaMeta({ ...POSE_META, schema_keyexpr: 'registry/x' }).schemaKeyExpr, 'registry/x');
    assert.equal(parseSchemaMeta({ type_name: 'a.B', schema_digest: 'abc' }).digest, 'abc');
  });

  test('parseSchemaMeta rejects incomplete meta data', () => {
    assert.throws(() => parseSchemaMeta(null), /not a JSON object/);
    assert.throws(() => parseSchemaMeta({ schema_digest: 'x' }), /type_name/);
    assert.throws(() => parseSchemaMeta({ type_name: 'a.B' }), /schema_digest/);
  });

  test('rootFromDescriptorSet keeps proto field names and resolves imports', () => {
    const type = rootFromDescriptorSet(POSE_DESCRIPTOR_SET).lookupType('rcs.io.Pose');
    assert.deepEqual(Object.keys(type.fields), ['pos_x', 'frame_id', 'kind', 'stamp', 'count', 'vals']);
  });
});

describe('schema discovery store', () => {
  beforeEach(() => {
    useSchemaDiscoveryStore.getState().clear();
    useProtoStore.getState().clearAll();
  });

  test('follows @schema to the descriptor set and decodes samples by key', async () => {
    const queried = serve({ 'rcsio/pose': POSE_META }, { [`schemas/${DIGEST}`]: POSE_DESCRIPTOR_SET });

    const entry = await useSchemaDiscoveryStore.getState().resolve(SESSION, 'rcsio/pose');
    assert.equal(entry.status, 'resolved', entry.error);
    assert.equal(entry.typeName, 'rcs.io.Pose');
    assert.deepEqual(queried, ['rcsio/pose/@schema', `schemas/${DIGEST}`]);

    const res = tryFormatProtobuf(POSE_BYTES, { keyExpr: 'rcsio/pose' });
    assert.equal(res.success, true, res.error);
    assert.deepEqual(res.data, {
      pos_x: 1.5,
      frame_id: 'map',
      kind: 'KIND_B',
      stamp: { seconds: '5', nanos: 0 },
      count: '123',
      vals: [1, 2],
    });
    assert.match(getPayloadSnippet(POSE_BYTES, 'protobuf', 120, { keyExpr: 'rcsio/pose' }), /"frame_id":"map"/);
    assert.equal(normalizeEncoding('zenoh/bytes', POSE_BYTES, 'rcsio/pose'), 'protobuf');
  });

  test('uses schema_keyexpr from the meta data when given', async () => {
    const queried = serve(
      { 'rcsio/pose': { ...POSE_META, schema_keyexpr: 'registry/pose' } },
      { 'registry/pose': POSE_DESCRIPTOR_SET }
    );
    const entry = await useSchemaDiscoveryStore.getState().resolve(SESSION, 'rcsio/pose');
    assert.equal(entry.status, 'resolved', entry.error);
    assert.deepEqual(queried, ['rcsio/pose/@schema', 'registry/pose']);
  });

  test('fetches a descriptor set only once per digest', async () => {
    const queried = serve(
      { 'robot/1/pose': POSE_META, 'robot/2/pose': POSE_META },
      { [`schemas/${DIGEST}`]: POSE_DESCRIPTOR_SET }
    );
    const store = useSchemaDiscoveryStore.getState();
    const [a, b] = await Promise.all([store.resolve(SESSION, 'robot/1/pose'), store.resolve(SESSION, 'robot/2/pose')]);
    assert.equal(a.status, 'resolved');
    assert.equal(b.status, 'resolved');
    assert.equal(queried.filter((q) => q === `schemas/${DIGEST}`).length, 1);
  });

  test('request() issues one lookup per key while pending and after success', async () => {
    const queried = serve({ 'rcsio/pose': POSE_META }, { [`schemas/${DIGEST}`]: POSE_DESCRIPTOR_SET });
    const store = useSchemaDiscoveryStore.getState();
    store.request(SESSION, 'rcsio/pose');
    store.request(SESSION, 'rcsio/pose');
    await store.resolve(SESSION, 'rcsio/pose');
    store.request(SESSION, 'rcsio/pose');
    assert.equal(queried.filter((q) => q.endsWith('/@schema')).length, 1);
  });

  test('caches missing meta data and only re-queries on retry', async () => {
    const queried = serve({}, {});
    const store = useSchemaDiscoveryStore.getState();

    const entry = await store.resolve(SESSION, 'plain/topic');
    assert.equal(entry.status, 'not_found');
    assert.match(entry.error ?? '', /plain\/topic\/@schema/);

    store.request(SESSION, 'plain/topic');
    assert.equal(queried.length, 1);

    await store.retry(SESSION, 'plain/topic');
    assert.equal(queried.length, 2);
    assert.equal(tryFormatProtobuf(POSE_BYTES, { keyExpr: 'plain/topic' }).success, false);
  });

  test('reports a missing descriptor set', async () => {
    serve({ 'rcsio/pose': POSE_META }, {});
    const entry = await useSchemaDiscoveryStore.getState().resolve(SESSION, 'rcsio/pose');
    assert.equal(entry.status, 'not_found');
    assert.match(entry.error ?? '', new RegExp(`schemas/${DIGEST}`));
  });

  test('reports an advertised type missing from the descriptor set', async () => {
    serve(
      { 'rcsio/pose': { ...POSE_META, type_name: 'rcs.io.Missing' } },
      { [`schemas/${DIGEST}`]: POSE_DESCRIPTOR_SET }
    );
    const entry = await useSchemaDiscoveryStore.getState().resolve(SESSION, 'rcsio/pose');
    assert.equal(entry.status, 'error');
    assert.equal(useSchemaDiscoveryStore.getState().getDecoder('rcsio/pose'), null);
  });

  test('skips error replies and uses the first successful one', async () => {
    mockInvokeHandler = async (_cmd, args) => {
      const selector = String(args?.selector);
      if (selector.endsWith('/@schema')) {
        return [reply(selector, [], true), reply(selector, json(POSE_META))];
      }
      return [reply(selector, POSE_DESCRIPTOR_SET)];
    };
    const entry = await useSchemaDiscoveryStore.getState().resolve(SESSION, 'rcsio/pose');
    assert.equal(entry.status, 'resolved', entry.error);
  });

  test('a manual topic mapping takes precedence over a discovered schema', async () => {
    serve({ 'rcsio/pose': POSE_META }, { [`schemas/${DIGEST}`]: POSE_DESCRIPTOR_SET });
    await useSchemaDiscoveryStore.getState().resolve(SESSION, 'rcsio/pose');

    const proto = useProtoStore.getState();
    const added = proto.addSchema('manual.proto', 'syntax = "proto3"; package manual; message Other { double pos_x = 1; }');
    assert.equal(added.success, true);
    proto.addMapping('rcsio/**', added.id!, 'manual.Other');

    const res = tryFormatProtobuf(POSE_BYTES, { keyExpr: 'rcsio/pose' });
    assert.equal(res.success, true, res.error);
    assert.deepEqual(res.data, { pos_x: 1.5 });
  });
});

describe('discovered schemas in the Schema Manager', () => {
  beforeEach(() => {
    useSchemaDiscoveryStore.getState().clear();
    useProtoStore.getState().clearAll();
  });

  const serveSchemas = () =>
    serve(
      { 'robot/1/pose': POSE_META, 'robot/2/pose': POSE_META },
      { [`schemas/${DIGEST}`]: POSE_DESCRIPTOR_SET }
    );

  test('registers a discovered schema once per digest with its topics', async () => {
    serveSchemas();
    const store = useSchemaDiscoveryStore.getState();
    await store.resolve(SESSION, 'robot/1/pose');
    await store.resolve(SESSION, 'robot/2/pose');

    const schemas = useProtoStore.getState().schemas;
    assert.equal(schemas.length, 1);
    const [schema] = schemas;
    assert.equal(schema.source, 'discovered');
    assert.equal(schema.name, 'rcsio/pose.proto');
    assert.equal(schema.package, 'rcs.io');
    assert.equal(schema.syntax, 'proto3');
    assert.ok(schema.messageTypes.includes('rcs.io.Pose'));
    assert.equal(schema.discovery?.digest, DIGEST);
    assert.equal(schema.discovery?.schemaKeyExpr, `schemas/${DIGEST}`);
    assert.deepEqual(schema.discovery?.files, ['google/protobuf/timestamp.proto', 'rcsio/pose.proto']);
    assert.deepEqual(schema.discovery?.advertisedTypes, ['rcs.io.Pose']);
    assert.deepEqual(schema.discovery?.topics, ['robot/2/pose', 'robot/1/pose']);
    assert.match(schema.rawContent, /import "google\/protobuf\/timestamp\.proto";/);
    assert.match(schema.rawContent, /message Pose \{/);
    assert.match(schema.rawContent, /google\.protobuf\.Timestamp stamp = 4;/);
    assert.match(schema.rawContent, /repeated float vals = 6;/);
  });

  test('reuses a registered schema instead of fetching the descriptor set again', async () => {
    serveSchemas();
    await useSchemaDiscoveryStore.getState().resolve(SESSION, 'robot/1/pose');

    // Simulate a restart: discovery state is gone, the registered schema is persisted.
    useSchemaDiscoveryStore.getState().clear();
    const queried = serveSchemas();
    const entry = await useSchemaDiscoveryStore.getState().resolve(SESSION, 'robot/1/pose');
    assert.equal(entry.status, 'resolved', entry.error);
    assert.deepEqual(queried, ['robot/1/pose/@schema']);
    assert.deepEqual(tryFormatProtobuf(POSE_BYTES, { keyExpr: 'robot/1/pose' }).data?.frame_id, 'map');
  });

  test('deleting a discovered schema forgets it until it is discovered again', async () => {
    serveSchemas();
    await useSchemaDiscoveryStore.getState().resolve(SESSION, 'robot/1/pose');
    const [schema] = useProtoStore.getState().schemas;

    useProtoStore.getState().removeSchema(schema.id);
    assert.equal(useSchemaDiscoveryStore.getState().entries['robot/1/pose'], undefined);
    assert.equal(tryFormatProtobuf(POSE_BYTES, { keyExpr: 'robot/1/pose' }).success, false);

    const queried = serveSchemas();
    await useSchemaDiscoveryStore.getState().resolve(SESSION, 'robot/1/pose');
    assert.deepEqual(queried, ['robot/1/pose/@schema', `schemas/${DIGEST}`]);
    assert.equal(useProtoStore.getState().schemas.length, 1);
  });

  test('discovered schemas are read-only but usable in manual mappings', async () => {
    serveSchemas();
    await useSchemaDiscoveryStore.getState().resolve(SESSION, 'robot/1/pose');
    const proto = useProtoStore.getState();
    const [schema] = proto.schemas;

    const update = proto.updateSchema(schema.id, 'syntax = "proto3"; message A {}');
    assert.equal(update.success, false);
    assert.match(update.error ?? '', /read-only/);

    assert.ok(proto.getCompiledRoot(schema.id)?.lookupType('rcs.io.Pose'));
    assert.ok(
      proto.getAllMessageTypes().some((t) => t.protoId === schema.id && t.typeName === 'rcs.io.Pose')
    );
    proto.addMapping('fleet/**', schema.id, 'rcs.io.Pose');
    const res = tryFormatProtobuf(POSE_BYTES, { keyExpr: 'fleet/a/pose' });
    assert.equal(res.success, true, res.error);
    assert.equal((res.data as Record<string, unknown>).pos_x, 1.5);
  });

  test('schemas added in the manager record how they were added', () => {
    const proto = useProtoStore.getState();
    const src = 'syntax = "proto3"; package p; message M { int32 a = 1; }';
    const file = proto.addSchema('m.proto', src, 'file');
    const legacy = proto.addSchema('legacy.proto', src);
    const byId = (id?: string) => useProtoStore.getState().schemas.find((s) => s.id === id);
    assert.equal(byId(file.id)?.source, 'file');
    assert.equal(byId(legacy.id)?.source, undefined);
    assert.equal(byId(file.id)?.discovery, undefined);
  });
});

describe('describeDescriptorSet', () => {
  // demo/x.proto: message X { map<string,int32> tags; oneof payload { a, b }; optional int32 opt; } service Svc
  const MAP_ONEOF_SET = Buffer.from(
    'CtEBCgxkZW1vL3gucHJvdG8SBGRlbW8ikAEKAVgSHwoEdGFncxgBIAMoCzIRLmRlbW8uWC5UYWdzRW50cnkSCwoBYRgCIAEoCUgAEgsKAWIYAyABKAVIABIQCgNvcHQYBCABKAVIAYgBARorCglUYWdzRW50cnkSCwoDa2V5GAEgASgJEg0KBXZhbHVlGAIgASgFOgI4AUIJCgdwYXlsb2FkQgYKBF9vcHQyIAoDU3ZjEhkKA0dldBIHLmRlbW8uWBoHLmRlbW8uWDABYgZwcm90bzM=',
    'base64'
  );

  test('renders maps, oneofs, proto3 optional and services as .proto text', async () => {
    const { describeDescriptorSet } = await import('../../src/lib/protoDescriptor');
    const summary = describeDescriptorSet(MAP_ONEOF_SET);
    assert.equal(summary.mainFile, 'demo/x.proto');
    assert.deepEqual(summary.messageTypes, ['demo.X']);
    const text = summary.protoText;
    assert.match(text, /map<string, int32> tags = 1;/);
    assert.match(text, /oneof payload \{\n    string a = 2;\n    int32 b = 3;\n  \}/);
    assert.match(text, /optional int32 opt = 4;/);
    assert.doesNotMatch(text, /TagsEntry/);
    assert.match(text, /rpc Get \(X\) returns \(stream X\);/);
  });
});
