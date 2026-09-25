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

import type protobuf from 'protobufjs';

/** How a schema entered the registry. Schemas saved before this field existed have none. */
export type ProtoSchemaSource = 'file' | 'preset' | 'editor' | 'discovered';

/** Provenance of a schema fetched from a publisher via `<topic>/@schema`. */
export interface ProtoDiscoveryInfo {
  /** Schema digest without its algorithm prefix. */
  digest: string;
  /** Key the descriptor set was fetched from, e.g. `schemas/<digest>`. */
  schemaKeyExpr: string;
  /** Serialized google.protobuf.FileDescriptorSet, base64 encoded. */
  descriptorSet: string;
  /** Files in the descriptor set, dependencies first. */
  files: string[];
  /** Message types advertised by publishers for this schema. */
  advertisedTypes: string[];
  /** Topics seen using this schema, most recent first (capped). */
  topics: string[];
  lastSeenAt: number;
}

export interface ProtoDefinition {
  id: string;
  name: string;
  /** .proto text; generated from the descriptor set (read-only) for discovered schemas. */
  rawContent: string;
  syntax: 'proto2' | 'proto3';
  package?: string;
  messageTypes: string[];
  createdAt: number;
  updatedAt: number;
  source?: ProtoSchemaSource;
  discovery?: ProtoDiscoveryInfo;
}

export interface DiscoveredSchemaInput {
  digest: string;
  schemaKeyExpr: string;
  topic: string;
  typeName: string;
  /** Required when no schema with this digest is registered yet. */
  descriptorSet?: Uint8Array;
}

export interface ProtoTopicMapping {
  id: string;
  keyPattern: string;
  protoId: string;
  messageTypeName: string;
  createdAt: number;
}

export interface ProtoState {
  schemas: ProtoDefinition[];
  mappings: ProtoTopicMapping[];

  addSchema: (
    name: string,
    rawContent: string,
    source?: ProtoSchemaSource
  ) => { success: boolean; error?: string; id?: string };
  /** Registers a schema discovered via `<topic>/@schema`, or records another topic using it. */
  upsertDiscoveredSchema: (input: DiscoveredSchemaInput) => string;
  findSchemaByDigest: (digest: string) => ProtoDefinition | undefined;
  updateSchema: (id: string, rawContent: string, name?: string) => { success: boolean; error?: string };
  removeSchema: (id: string) => void;

  addMapping: (keyPattern: string, protoId: string, messageTypeName: string) => void;
  removeMapping: (mappingId: string) => void;

  findMappingForKey: (keyExpr: string) => ProtoTopicMapping | undefined;
  getAllMessageTypes: () => Array<{ protoId: string; protoName: string; typeName: string }>;
  getCompiledRoot: (protoId: string) => protobuf.Root | null;
  getGlobalRoot: () => protobuf.Root;
  clearAll: () => void;
}

