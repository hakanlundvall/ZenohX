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

import React from 'react';
import { Fingerprint, Shuffle, X } from 'lucide-react';
import { Label } from '../../ui/label';
import { Input } from '../../ui/input';
import { Button } from '../../ui/button';
import { generateRandomZid, isValidZid } from '../../../lib/tls';

export interface NodeIdentitySectionProps {
  zid: string;
  setZid: (val: string) => void;
  activeSessionZid?: string;
}

export const NodeIdentitySection: React.FC<NodeIdentitySectionProps> = ({
  zid,
  setZid,
  activeSessionZid,
}) => {
  const trimmed = zid.trim();
  const hasValue = Boolean(trimmed);
  const isValid = !hasValue || isValidZid(trimmed);
  const normalized = hasValue ? trimmed.replace(/-/g, '').toLowerCase() : '';

  const handleGenerateRandom = () => {
    setZid(generateRandomZid());
  };

  const handleClear = () => {
    setZid('');
  };

  return (
    <div className="space-y-3 p-3.5 rounded-lg border bg-muted/10">
      <div className="flex items-start justify-between gap-2">
        <div className="space-y-0.5">
          <Label className="text-xs font-semibold flex items-center gap-1.5">
            <Fingerprint className="w-3.5 h-3.5 text-primary" />
            Node Identity (Zenoh ID / ZID)
          </Label>
          <p className="text-[11px] text-muted-foreground">
            Configure an explicit 128-bit hexadecimal Zenoh ID (ZID). Leave blank to auto-generate.
          </p>
        </div>
      </div>

      <div className="space-y-1.5">
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <Input
              value={zid}
              onChange={(e) => setZid(e.target.value)}
              placeholder={
                activeSessionZid
                  ? `Active node: ${activeSessionZid}`
                  : 'Auto-generated (e.g. 0123456789abcdef...)'
              }
              className={`h-8 font-mono text-xs bg-background pr-7 ${
                !isValid ? 'border-destructive focus-visible:ring-destructive' : ''
              }`}
            />
            {hasValue && (
              <button
                type="button"
                onClick={handleClear}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                title="Clear custom ZID"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>

          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleGenerateRandom}
            className="h-8 px-2.5 text-xs gap-1.5 shrink-0"
            title="Generate a random 128-bit hexadecimal Zenoh ID"
          >
            <Shuffle className="w-3.5 h-3.5 text-muted-foreground" />
            Random
          </Button>
        </div>

        {hasValue && !isValid && (
          <p className="text-[11px] text-destructive">
            Invalid Zenoh ID. Must be 1 to 32 hexadecimal characters (0-9, a-f).
          </p>
        )}

        {hasValue && isValid && (
          <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground font-mono">
            <span className="text-muted-foreground/70">ZID Hex:</span>
            <span className="text-foreground/80 font-semibold">{normalized}</span>
            <span className="text-[9px] text-muted-foreground">({normalized.length}/32 hex chars)</span>
          </div>
        )}
      </div>
    </div>
  );
};
