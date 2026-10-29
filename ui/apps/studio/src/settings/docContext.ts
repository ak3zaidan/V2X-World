/**
 * The settings draft, for a control whose choices depend on another field of the document (the
 * providers a failure can name, 08-settings-ux-and-gaps §4.2 item 2).
 */

import { createContext } from "react";

export const SettingsDocContext = createContext<unknown>(null);
