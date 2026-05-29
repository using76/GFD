/**
 * Pluggable physics model manifest (Phase 7).
 *
 * A physics model is a manifest of governing equations whose terms are either
 * built-in solver kernels or runtime EXPRESSIONS (gfd-expression / GMN strings,
 * e.g. "$rho * ddt($U) + div($rho*$U*$U) - laplacian($mu,$U)"). Constitutive
 * relations (viscosity, density, ...) are likewise constant or expression-based.
 *
 * This GUI-side contract serializes into the solver config (passed to
 * solve.start as `physics_manifest`). The backend reuses gfd-expression to
 * parse/linearize/evaluate expression terms; see the plan's Phase 7 backend
 * contract (physics.* RPCs + ExpressionSourceTerm).
 */

export type TermRole = 'transient' | 'convection' | 'diffusion' | 'source' | 'pressure_coupling';

export type TermImpl =
  | { kind: 'builtin'; name: string; params?: Record<string, number> }
  | { kind: 'expression'; expr: string; linearizeOver?: string };

export type ZoneSelector =
  | { by: 'all' }
  | { by: 'name'; name: string }
  | { by: 'expression'; expr: string };

export interface PhysicsTerm {
  id: string;
  role: TermRole;
  impl: TermImpl;
  zone: ZoneSelector;
  enabled: boolean;
}

export interface GoverningEquation {
  /** Local id, e.g. "momentum_x". */
  id: string;
  /** Maps to the backend EquationId (MomentumX, Energy, Continuity, ...). */
  equationId: string;
  /** Field being solved (U, p, T, k, ...). */
  field: string;
  terms: PhysicsTerm[];
}

export type ConstitutiveImpl = { kind: 'constant'; value: number } | { kind: 'expression'; expr: string };

export interface ConstitutiveRelation {
  property: 'viscosity' | 'density' | 'conductivity' | 'specific_heat' | string;
  impl: ConstitutiveImpl;
}

export interface PhysicsManifest {
  id: string;
  name: string;
  nameKo?: string;
  equations: GoverningEquation[];
  constitutive: ConstitutiveRelation[];
  coupling: { pressureVelocity: 'SIMPLE' | 'PISO' | 'SIMPLEC'; relaxation: Record<string, number> };
}

function builtinMomentum(axis: 'x' | 'y' | 'z'): GoverningEquation {
  const field = `U_${axis}`;
  const mk = (role: TermRole, name: string): PhysicsTerm => ({
    id: `momentum_${axis}_${role}`,
    role,
    impl: { kind: 'builtin', name },
    zone: { by: 'all' },
    enabled: true,
  });
  return {
    id: `momentum_${axis}`,
    equationId: `Momentum${axis.toUpperCase()}`,
    field,
    terms: [
      mk('transient', 'ddt'),
      mk('convection', 'div_uu'),
      mk('diffusion', 'laplacian_mu'),
      mk('pressure_coupling', 'grad_p'),
    ],
  };
}

/** The default incompressible Navier–Stokes model — all built-in. */
export function buildDefaultManifest(): PhysicsManifest {
  return {
    id: 'incompressible_ns',
    name: 'Incompressible Navier–Stokes',
    nameKo: '비압축성 나비에-스토크스',
    equations: [
      builtinMomentum('x'),
      builtinMomentum('y'),
      builtinMomentum('z'),
      {
        id: 'continuity',
        equationId: 'Continuity',
        field: 'p',
        terms: [
          { id: 'continuity_pc', role: 'pressure_coupling', impl: { kind: 'builtin', name: 'pressure_correction' }, zone: { by: 'all' }, enabled: true },
        ],
      },
    ],
    constitutive: [
      { property: 'density', impl: { kind: 'constant', value: 1.0 } },
      { property: 'viscosity', impl: { kind: 'constant', value: 0.01 } },
    ],
    coupling: { pressureVelocity: 'SIMPLE', relaxation: { velocity: 0.5, pressure: 0.3 } },
  };
}
