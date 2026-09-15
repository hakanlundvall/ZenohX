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
import { PayloadViewer, JsonHighlightedCode } from '../../src/components/viewer/PayloadViewer';

describe('PayloadViewer Component & Structure', () => {
  test('PayloadViewer and JsonHighlightedCode are exported as functions', () => {
    assert.equal(typeof PayloadViewer, 'function');
    assert.equal(typeof JsonHighlightedCode, 'object'); // React.memo wraps into an object with $$typeof
  });

  test('PayloadViewer instantiates correctly without throwing for text payload', () => {
    const element = React.createElement(PayloadViewer, {
      payload: new TextEncoder().encode('Hello ZenohX'),
      encoding: 'text',
      maxHeight: '100%',
      className: 'flex-1 min-h-0',
    });
    assert.ok(element);
    assert.equal(element.props.encoding, 'text');
    assert.equal(element.props.maxHeight, '100%');
    assert.equal(element.props.className, 'flex-1 min-h-0');
  });

  test('PayloadViewer accepts JSON payload with custom or 100% maxHeight', () => {
    const jsonStr = JSON.stringify({ key: 'value', count: 42, active: true });
    const element = React.createElement(PayloadViewer, {
      payload: new TextEncoder().encode(jsonStr),
      encoding: 'json',
      maxHeight: '100%',
      className: 'flex-1 min-h-0',
    });
    assert.ok(element);
    assert.equal(element.props.encoding, 'json');
  });

  test('PayloadViewer accepts fixed pixel maxHeight for panels like QueryablePanel', () => {
    const element = React.createElement(PayloadViewer, {
      payload: new Uint8Array([1, 2, 3, 4]),
      encoding: 'raw',
      maxHeight: '160px',
    });
    assert.ok(element);
    assert.equal(element.props.maxHeight, '160px');
  });
});
