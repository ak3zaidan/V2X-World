import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App.js";
import { applyThemeToDocument } from "./lib/theme.js";
import { SettingsStandalone } from "./settings/Standalone.js";
import { isDetachedSettings } from "./state/settings.js";
import { readTheme } from "./state/store.js";
import "uplot/dist/uPlot.min.css";
import "./styles/app.css";

applyThemeToDocument(readTheme());

const host = document.getElementById("root");
if (!host) throw new Error("#root is missing from index.html");

createRoot(host).render(<StrictMode>{isDetachedSettings() ? <SettingsStandalone /> : <App />}</StrictMode>);
