/**
 * Shared primitive types for the GFD command-core.
 *
 * This package is framework-agnostic: it MUST NOT import React, Three.js,
 * Electron, or any rendering/UI library. It is the single source of truth that
 * both the human UI and external AI agents drive through commands.
 */

export type Vec3 = [number, number, number];

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export type JsonObject = { [key: string]: JsonValue };

/** Command side-effect classification, used for AI consent gating. */
export type Capability =
  | 'read'
  | 'view-only'
  | 'mutate-scene'
  | 'mutate-physics'
  | 'run-solver'
  | 'destructive';

/** Who initiated a command. Recorded in the journal for audit. */
export type CommandSource = 'human' | 'agent' | 'replay';

/** The 12 command categories. Map 1:1 onto the 9 SpaceClaim ribbon tabs. */
export type CommandCategory =
  | 'geometry'
  | 'display'
  | 'measure'
  | 'repair'
  | 'prepare'
  | 'mesh'
  | 'setup'
  | 'calc'
  | 'results'
  | 'view'
  | 'selection'
  | 'physics'
  | 'system';
