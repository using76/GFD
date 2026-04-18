/**
 * Unit conversion helpers. Internal state is always SI; these helpers convert
 * only for display and for the Imperial input fields. All toX() take an SI value
 * and return the Imperial equivalent; fromX() does the reverse.
 */

export type UnitSystem = 'SI' | 'Imperial';

export interface UnitLabels {
  length: string;
  velocity: string;
  pressure: string;
  temperature: string;
  density: string;
  force: string;
  stress: string;
  massFlow: string;
}

export const SI_LABELS: UnitLabels = {
  length: 'm',
  velocity: 'm/s',
  pressure: 'Pa',
  temperature: 'K',
  density: 'kg/m³',
  force: 'N',
  stress: 'Pa',
  massFlow: 'kg/s',
};

export const IMPERIAL_LABELS: UnitLabels = {
  length: 'ft',
  velocity: 'ft/s',
  pressure: 'psi',
  temperature: '°F',
  density: 'lb/ft³',
  force: 'lbf',
  stress: 'psi',
  massFlow: 'lb/s',
};

export const labelsFor = (u: UnitSystem): UnitLabels => (u === 'Imperial' ? IMPERIAL_LABELS : SI_LABELS);

// --- Conversion factors ---
const M_TO_FT = 3.28084;
const PA_TO_PSI = 1.45038e-4;
const KG_TO_LB = 2.20462;

export const length = {
  toDisplay: (si: number, u: UnitSystem) => (u === 'Imperial' ? si * M_TO_FT : si),
  toSI: (disp: number, u: UnitSystem) => (u === 'Imperial' ? disp / M_TO_FT : disp),
};

export const velocity = {
  toDisplay: (si: number, u: UnitSystem) => (u === 'Imperial' ? si * M_TO_FT : si),
  toSI: (disp: number, u: UnitSystem) => (u === 'Imperial' ? disp / M_TO_FT : disp),
};

export const pressure = {
  toDisplay: (si: number, u: UnitSystem) => (u === 'Imperial' ? si * PA_TO_PSI : si),
  toSI: (disp: number, u: UnitSystem) => (u === 'Imperial' ? disp / PA_TO_PSI : disp),
};

export const temperature = {
  /** K → °F */
  toDisplay: (si: number, u: UnitSystem) => (u === 'Imperial' ? (si - 273.15) * 9 / 5 + 32 : si),
  toSI: (disp: number, u: UnitSystem) => (u === 'Imperial' ? (disp - 32) * 5 / 9 + 273.15 : disp),
};

export const density = {
  toDisplay: (si: number, u: UnitSystem) => (u === 'Imperial' ? si * KG_TO_LB / Math.pow(M_TO_FT, 3) : si),
  toSI: (disp: number, u: UnitSystem) => (u === 'Imperial' ? disp / KG_TO_LB * Math.pow(M_TO_FT, 3) : disp),
};

export const force = {
  /** N → lbf (1 lbf ≈ 4.44822 N) */
  toDisplay: (si: number, u: UnitSystem) => (u === 'Imperial' ? si / 4.44822 : si),
  toSI: (disp: number, u: UnitSystem) => (u === 'Imperial' ? disp * 4.44822 : disp),
};

/** Format a scalar with the unit-system label. `kind` selects the conversion. */
export function formatWithUnit(
  si: number,
  kind: keyof UnitLabels,
  u: UnitSystem,
  precision = 3,
): string {
  const labels = labelsFor(u);
  let val = si;
  switch (kind) {
    case 'length': val = length.toDisplay(si, u); break;
    case 'velocity': val = velocity.toDisplay(si, u); break;
    case 'pressure':
    case 'stress': val = pressure.toDisplay(si, u); break;
    case 'temperature': val = temperature.toDisplay(si, u); break;
    case 'density': val = density.toDisplay(si, u); break;
    case 'force': val = force.toDisplay(si, u); break;
    case 'massFlow': val = u === 'Imperial' ? si * KG_TO_LB : si; break;
    default: break;
  }
  const absVal = Math.abs(val);
  const formatted = absVal >= 1e4 || (absVal < 1e-3 && absVal > 0)
    ? val.toExponential(precision)
    : val.toFixed(precision);
  return `${formatted} ${labels[kind]}`;
}
