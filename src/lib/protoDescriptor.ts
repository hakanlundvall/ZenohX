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
 * Helpers for serialized google.protobuf.FileDescriptorSet schemas: compiling them
 * into a protobuf.Root and rendering them as readable .proto text.
 */

import protobuf from 'protobufjs';
import descriptor from 'protobufjs/ext/descriptor.js';
import { extractMessageTypes } from './protobufEngine';

/** Builds a Root from a serialized google.protobuf.FileDescriptorSet. */
export function rootFromDescriptorSet(bytes: Uint8Array): protobuf.Root {
  // keepCase keeps proto field names (snake_case), like the rest of ZenohX.
  const root = protobuf.Root.fromDescriptor(bytes, { keepCase: true });
  root.resolveAll();
  return root;
}

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export interface DescriptorSetSummary {
  root: protobuf.Root;
  /** File names in the set, dependencies first. */
  files: string[];
  /** The file that declares `typeName`, else the last file of the set. */
  mainFile?: string;
  package?: string;
  syntax: 'proto2' | 'proto3';
  messageTypes: string[];
  protoText: string;
}

/** Compiles a descriptor set and renders it as .proto text for display. */
export function describeDescriptorSet(bytes: Uint8Array, typeName?: string): DescriptorSetSummary {
  const root = rootFromDescriptorSet(bytes);
  const set = descriptor.FileDescriptorSet.toObject(descriptor.FileDescriptorSet.decode(bytes), {
    enums: String,
    longs: String,
    defaults: false,
  }) as { file?: FileDesc[] };
  const files = set.file ?? [];

  const cleanType = typeName?.replace(/^\./, '');
  const main =
    (cleanType && files.find((f) => declaresType(f, cleanType))) || files[files.length - 1];

  return {
    root,
    files: files.map((f) => f.name ?? ''),
    mainFile: main?.name,
    package: main?.package || undefined,
    syntax: main?.syntax === 'proto3' ? 'proto3' : 'proto2',
    messageTypes: extractMessageTypes(root),
    protoText: files.map(printFile).join('\n'),
  };
}

// ---------------------------------------------------------------------------
// .proto rendering (display only; decoding always uses the descriptor itself)
// ---------------------------------------------------------------------------

interface FieldDesc {
  name?: string;
  number?: number;
  label?: string;
  type?: string;
  typeName?: string;
  defaultValue?: string;
  oneofIndex?: number;
  proto3Optional?: boolean;
}

interface MessageDesc {
  name?: string;
  field?: FieldDesc[];
  nestedType?: MessageDesc[];
  enumType?: EnumDesc[];
  oneofDecl?: Array<{ name?: string }>;
  options?: { mapEntry?: boolean };
}

interface EnumDesc {
  name?: string;
  value?: Array<{ name?: string; number?: number }>;
}

interface ServiceDesc {
  name?: string;
  method?: Array<{
    name?: string;
    inputType?: string;
    outputType?: string;
    clientStreaming?: boolean;
    serverStreaming?: boolean;
  }>;
}

interface FileDesc {
  name?: string;
  package?: string;
  dependency?: string[];
  messageType?: MessageDesc[];
  enumType?: EnumDesc[];
  service?: ServiceDesc[];
  syntax?: string;
}

function declaresType(file: FileDesc, fullName: string): boolean {
  const prefix = file.package ? `${file.package}.` : '';
  const walk = (msgs: MessageDesc[] | undefined, scope: string): boolean =>
    (msgs ?? []).some((m) => {
      const name = `${scope}${m.name}`;
      return name === fullName || walk(m.nestedType, `${name}.`);
    });
  return walk(file.messageType, prefix);
}

const SCALARS: Record<string, string> = {
  TYPE_DOUBLE: 'double',
  TYPE_FLOAT: 'float',
  TYPE_INT64: 'int64',
  TYPE_UINT64: 'uint64',
  TYPE_INT32: 'int32',
  TYPE_FIXED64: 'fixed64',
  TYPE_FIXED32: 'fixed32',
  TYPE_BOOL: 'bool',
  TYPE_STRING: 'string',
  TYPE_BYTES: 'bytes',
  TYPE_UINT32: 'uint32',
  TYPE_SFIXED32: 'sfixed32',
  TYPE_SFIXED64: 'sfixed64',
  TYPE_SINT32: 'sint32',
  TYPE_SINT64: 'sint64',
};

function printFile(file: FileDesc): string {
  const pkg = file.package ?? '';
  const proto3 = file.syntax === 'proto3';
  const typeRef = (name: string | undefined) => {
    const clean = (name ?? '').replace(/^\./, '');
    return pkg && clean.startsWith(`${pkg}.`) ? clean.slice(pkg.length + 1) : clean;
  };

  const lines: string[] = [`// ${file.name ?? 'unnamed.proto'}`, `syntax = "${proto3 ? 'proto3' : 'proto2'}";`];
  if (pkg) lines.push('', `package ${pkg};`);
  if (file.dependency?.length) {
    lines.push('');
    for (const dep of file.dependency) lines.push(`import "${dep}";`);
  }

  const fieldType = (f: FieldDesc) => (f.type && SCALARS[f.type]) || typeRef(f.typeName);

  const printEnum = (e: EnumDesc, indent: string) => {
    lines.push('', `${indent}enum ${e.name} {`);
    for (const v of e.value ?? []) lines.push(`${indent}  ${v.name} = ${v.number ?? 0};`);
    lines.push(`${indent}}`);
  };

  const printMessage = (m: MessageDesc, indent: string, scope: string) => {
    const fullName = `${scope}${m.name}`;
    const mapEntries = new Map<string, MessageDesc>();
    for (const nested of m.nestedType ?? []) {
      if (nested.options?.mapEntry) mapEntries.set(`${fullName}.${nested.name}`, nested);
    }

    const printField = (f: FieldDesc, pad: string) => {
      const entry = f.type === 'TYPE_MESSAGE' ? mapEntries.get((f.typeName ?? '').replace(/^\./, '')) : undefined;
      if (entry) {
        const [key, value] = [entry.field?.find((x) => x.number === 1), entry.field?.find((x) => x.number === 2)];
        lines.push(`${pad}map<${key ? fieldType(key) : '?'}, ${value ? fieldType(value) : '?'}> ${f.name} = ${f.number};`);
        return;
      }
      let label = '';
      if (f.label === 'LABEL_REPEATED') label = 'repeated ';
      else if (f.label === 'LABEL_REQUIRED') label = 'required ';
      else if (!proto3 || f.proto3Optional) label = 'optional ';
      const def = f.defaultValue !== undefined && f.defaultValue !== '' ? ` [default = ${f.defaultValue}]` : '';
      lines.push(`${pad}${label}${fieldType(f)} ${f.name} = ${f.number}${def};`);
    };

    lines.push('', `${indent}message ${m.name} {`);
    const inner = `${indent}  `;
    const fields = m.field ?? [];
    const oneofFields = new Map<number, FieldDesc[]>();
    for (const f of fields) {
      if (f.oneofIndex !== undefined && f.oneofIndex !== null && !f.proto3Optional) {
        oneofFields.set(f.oneofIndex, [...(oneofFields.get(f.oneofIndex) ?? []), f]);
      }
    }
    const printedOneofs = new Set<number>();
    for (const f of fields) {
      const idx = f.oneofIndex;
      if (idx !== undefined && idx !== null && oneofFields.has(idx)) {
        if (printedOneofs.has(idx)) continue;
        printedOneofs.add(idx);
        lines.push(`${inner}oneof ${m.oneofDecl?.[idx]?.name ?? `oneof_${idx}`} {`);
        for (const member of oneofFields.get(idx)!) printField(member, `${inner}  `);
        lines.push(`${inner}}`);
      } else {
        printField(f, inner);
      }
    }
    for (const e of m.enumType ?? []) printEnum(e, inner);
    for (const nested of m.nestedType ?? []) {
      if (!nested.options?.mapEntry) printMessage(nested, inner, `${fullName}.`);
    }
    lines.push(`${indent}}`);
  };

  for (const e of file.enumType ?? []) printEnum(e, '');
  for (const m of file.messageType ?? []) printMessage(m, '', pkg ? `${pkg}.` : '');
  for (const s of file.service ?? []) {
    lines.push('', `service ${s.name} {`);
    for (const rpc of s.method ?? []) {
      const input = `${rpc.clientStreaming ? 'stream ' : ''}${typeRef(rpc.inputType)}`;
      const output = `${rpc.serverStreaming ? 'stream ' : ''}${typeRef(rpc.outputType)}`;
      lines.push(`  rpc ${rpc.name} (${input}) returns (${output});`);
    }
    lines.push('}');
  }
  return lines.join('\n') + '\n';
}
