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

import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import { installWebBridgeIfNeeded } from "./lib/webBridge";

// In web access mode (served by `zenohx --web`) the Tauri IPC is bridged over a
// WebSocket; it must be in place before the app's modules can call into Tauri.
installWebBridgeIfNeeded()
  .then(() => import("./App"))
  .then(({ default: App }) => {
    ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>
    );
  });
