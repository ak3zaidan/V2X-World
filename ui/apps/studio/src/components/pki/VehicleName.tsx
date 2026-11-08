/**
 * A vehicle's name as every certificate view writes it: "Vehicle 66" (its simulation node, the
 * number the map, the HUD and the camera chip show) with its SCMS device id as a small second part
 * ("device 1000066"), and a tooltip that ties the two together (`lib/pkiNames.ts`).
 */

import { deviceLine, vehicleName, vehicleTitle } from "../../lib/pkiNames.js";

export function VehicleName({ device, node, testId }: { readonly device: string; readonly node?: number | null; readonly testId?: string }): React.JSX.Element {
  const sub = deviceLine(device, node);
  return (
    <span className="pki-vname" title={vehicleTitle(device, node)} data-testid={testId} data-device={device}>
      <span className="pki-vname-main">{vehicleName(device, node)}</span>
      {sub ? <small className="pki-vname-sub faint mono">{sub}</small> : null}
    </span>
  );
}
