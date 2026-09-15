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

import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import React from 'react';
import {
  MessageDetails,
  fromMessageItem,
  fromReplySample,
  type UnifiedMessageData,
} from '../../src/components/viewer/MessageDetails';
import type { MessageItem, ReplySample } from '../../src/types/zenoh';

describe('MessageDetails Component & Adapters', () => {
  test('fromMessageItem converts a Pub/Sub MessageItem into UnifiedMessageData', () => {
    const pubSubMsg: MessageItem = {
      id: 'msg-1',
      sessionId: 'session-123',
      profileId: 'profile-abc',
      direction: 'incoming',
      keyExpr: 'demo/telemetry/robot1',
      payload: Array.from(new TextEncoder().encode('{"status":"online"}')),
      encoding: 'json',
      kind: 'put',
      timestamp: 1700000000000,
      senderZid: 'zid-xyz',
    };

    const unified: UnifiedMessageData = fromMessageItem(pubSubMsg, {
      senderSubtitle: 'Remote Publisher',
      profileName: 'Production Router',
    });

    assert.equal(unified.keyExpr, 'demo/telemetry/robot1');
    assert.equal(unified.direction, 'incoming');
    assert.equal(unified.kind, 'put');
    assert.equal(unified.encoding, 'json');
    assert.equal(unified.timestamp, 1700000000000);
    assert.equal(unified.senderZid, 'zid-xyz');
    assert.equal(unified.sessionId, 'session-123');
    assert.equal(unified.profileName, 'Production Router');
  });

  test('fromMessageItem resolves full ZID for incoming message with sourceId', () => {
    const incomingMsg: MessageItem = {
      id: 'msg-2',
      sessionId: 'session-123',
      direction: 'incoming',
      keyExpr: 'demo/robot/telemetry',
      payload: [1, 2, 3],
      encoding: 'raw',
      kind: 'put',
      timestamp: 1700000001000,
      sourceId: 'c8ef34a1b2c3d4e5f67890abcdef1234',
    };

    const unified = fromMessageItem(incomingMsg);
    assert.equal(unified.senderZid, 'c8ef34a1b2c3d4e5f67890abcdef1234');
    assert.equal(unified.senderTitle, 'c8ef34a1b2c3d4e5f67890abcdef1234');
  });

  test('fromMessageItem falls back to Unknown / Anonymous when no sourceId or senderZid', () => {
    const anonMsg: MessageItem = {
      id: 'msg-3',
      sessionId: 'session-123',
      direction: 'incoming',
      keyExpr: 'demo/robot/telemetry',
      payload: [],
      encoding: 'text',
      kind: 'put',
      timestamp: 1700000002000,
    };

    const unified = fromMessageItem(anonMsg);
    assert.equal(unified.senderTitle, 'Unknown / Anonymous');
  });

  test('fromReplySample converts a Query ReplySample into UnifiedMessageData', () => {
    const reply: ReplySample = {
      session_id: 'session-456',
      key_expr: 'demo/telemetry/queryable',
      payload: Array.from(new TextEncoder().encode('{"temperature":23.5}')),
      encoding: 'json',
      replier_id: 'zid-replier-99',
      latency_ms: 18,
      timestamp: 1700000050000,
      is_err: false,
      error_message: null,
    };

    const unified: UnifiedMessageData = fromReplySample(reply);

    assert.equal(unified.keyExpr, 'demo/telemetry/queryable');
    assert.equal(unified.encoding, 'json');
    assert.equal(unified.timestamp, 1700000050000);
    assert.equal(unified.latencyMs, 18);
    assert.equal(unified.replierId, 'zid-replier-99');
    assert.equal(unified.isError, false);
    assert.equal(unified.errorMessage, null);
  });

  test('MessageDetails instantiates React element without throwing', () => {
    const reply: ReplySample = {
      session_id: 'session-456',
      key_expr: 'demo/telemetry/queryable',
      payload: Array.from(new TextEncoder().encode('{"temperature":23.5}')),
      encoding: 'json',
      replier_id: 'zid-replier-99',
      latency_ms: 18,
      timestamp: 1700000050000,
      is_err: false,
      error_message: null,
    };

    const unified = fromReplySample(reply);
    const element = React.createElement(MessageDetails, {
      data: unified,
      title: 'Reply Details',
      onClose: () => {},
    });

    assert.ok(element);
    assert.equal(element.props.title, 'Reply Details');
    assert.equal(element.props.data.keyExpr, 'demo/telemetry/queryable');
  });
});
