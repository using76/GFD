/**
 * Consent gating for AI-agent-initiated commands.
 *
 * Human commands always pass. Agent commands are checked against a policy by
 * capability. `ask` decisions are routed to an injectable approver (the UI
 * renders a prompt; headless tests auto-answer). Every decision is observable
 * so the app can show an audit trail and a "stop agent" control.
 */

import type { Capability, CommandSource } from './types';

export type ConsentDecision = 'allow' | 'ask' | 'deny';

export type ConsentMode = 'autonomous' | 'confirm-mutations' | 'confirm-destructive' | 'read-only';

export interface ConsentPolicy {
  mode: ConsentMode;
  /** Optional per-capability overrides that win over the mode default. */
  perCapability?: Partial<Record<Capability, ConsentDecision>>;
}

export interface ApprovalRequest {
  commandId: string;
  capability: Capability;
  agentSessionId?: string;
}

/** Resolves an `ask` decision to a yes/no. Injected by the host (UI or test). */
export type Approver = (req: ApprovalRequest) => Promise<boolean>;

const READ_ONLY_CAPS: ReadonlySet<Capability> = new Set<Capability>(['read', 'view-only']);

function defaultDecision(mode: ConsentMode, capability: Capability): ConsentDecision {
  if (READ_ONLY_CAPS.has(capability)) return 'allow';
  switch (mode) {
    case 'read-only':
      return 'deny';
    case 'autonomous':
      return 'allow';
    case 'confirm-mutations':
      return 'ask';
    case 'confirm-destructive':
      return capability === 'destructive' ? 'ask' : 'allow';
  }
}

export class ConsentController {
  private policy: ConsentPolicy;
  private approver: Approver;

  constructor(policy: ConsentPolicy = { mode: 'autonomous' }, approver?: Approver) {
    this.policy = policy;
    // Default approver denies — a host that wants `ask` must wire a real UI.
    this.approver = approver ?? (async () => false);
  }

  setPolicy(policy: ConsentPolicy): void {
    this.policy = policy;
  }

  getPolicy(): Readonly<ConsentPolicy> {
    return this.policy;
  }

  setApprover(approver: Approver): void {
    this.approver = approver;
  }

  /** Returns true if the command may proceed. Human commands always may. */
  async authorize(source: CommandSource, req: ApprovalRequest): Promise<boolean> {
    if (source !== 'agent') return true;
    const override = this.policy.perCapability?.[req.capability];
    const decision = override ?? defaultDecision(this.policy.mode, req.capability);
    if (decision === 'allow') return true;
    if (decision === 'deny') return false;
    return this.approver(req);
  }
}
