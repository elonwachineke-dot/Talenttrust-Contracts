//! Two-step approval workflow for high-impact governance overrides (#1221).
//!
//! High-impact protocol-configuration changes (`set_protocol_fee_bps`,
//! `set_governed_params`, `set_fee_withdrawal_cap`, `set_fee_withdrawal_cooldown`,
//! `set_max_milestones`) must pass through a two-step request → approve/reject →
//! apply state machine before they take effect.  This prevents a single
//! unreviewed request from unilaterally changing sensitive parameters.
//!
//! ## State machine
//!
//! ```text
//! [admin]  request_governance_proposal(kind)  →  Pending
//! [approver ≠ requester]  approve_governance_proposal(id)  →  Approved
//! [approver ≠ requester]  reject_governance_proposal(id)  →  Rejected  (terminal)
//! [admin]  apply_governance_proposal(id)  →  Applied  (terminal; side-effects executed)
//!
//! Any step fails with GovernanceProposalExpired if ledger.sequence() > expires_at_ledger.
//! ```
//!
//! ## Security properties
//!
//! * **Separate approver identity** — `approve_governance_proposal` rejects the
//!   requester's own address with `GovernanceSelfApproval`.
//! * **Short expiry window** — proposals expire after
//!   [`GOVERNANCE_PROPOSAL_TTL_LEDGERS`] (~3 days). Stale proposals cannot be
//!   applied after circumstances change.
//! * **Idempotency guard** — `apply_governance_proposal` can only be called once
//!   per proposal; subsequent calls fail with `GovernanceProposalInvalidState`.
//! * **Audit trail** — every state transition emits a structured Soroban event
//!   with proposal ID, kind, parties, and timestamp.
//! * **Rejection is terminal** — a rejected proposal cannot be re-approved or
//!   applied; the admin must open a fresh proposal.

pub use crate::Escrow;
use crate::storage_validation;
use crate::ttl::{set_governance_proposal_ttl, GOVERNANCE_PROPOSAL_TTL_LEDGERS};
use crate::{
    DataKey, Error, EscrowArgs, EscrowClient, GovernanceProposal, GovernanceProposalKind,
    GovernanceProposalState, GovernedParameters, MAX_FEE_BPS,
};
use soroban_sdk::{symbol_short, Address, Env, Symbol};

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Allocate a new monotonically-increasing proposal ID.
pub(crate) fn next_proposal_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::NextGovernanceProposalId)
        .unwrap_or(0u64);
    let next = current.saturating_add(1);
    env.storage()
        .persistent()
        .set(&DataKey::NextGovernanceProposalId, &next);
    next
}

/// Load a governance proposal from persistent storage, returning an error if not found.
///
/// Does **not** check expiry — callers must do that themselves so they can
/// distinguish "not found" from "expired-but-still-in-storage".
pub(crate) fn load_proposal(env: &Env, proposal_id: u64) -> GovernanceProposal {
    env.storage()
        .persistent()
        .get(&DataKey::GovernanceProposal(proposal_id))
        .unwrap_or_else(|| env.panic_with_error(Error::GovernanceProposalNotFound))
}

/// Persist a governance proposal and renew its TTL.
pub(crate) fn save_proposal(env: &Env, proposal: &GovernanceProposal) {
    env.storage()
        .persistent()
        .set(&DataKey::GovernanceProposal(proposal.proposal_id), proposal);
    set_governance_proposal_ttl(env, proposal.proposal_id);
}

/// Assert the proposal has not yet passed its expiry ledger.
pub(crate) fn require_not_expired(env: &Env, proposal: &GovernanceProposal) {
    if env.ledger().sequence() > proposal.expires_at_ledger {
        env.panic_with_error(Error::GovernanceProposalExpired);
    }
}

/// Assert the proposal is in the expected state, panicking with
/// `GovernanceProposalInvalidState` otherwise.
///
/// Centralising this check keeps every transition's state invariant
/// explicit and prevents accidental drift between entry points.
pub(crate) fn require_state(env: &Env, proposal: &GovernanceProposal, expected: GovernanceProposalState) {
    if proposal.state != expected {
        env.panic_with_error(Error::GovernanceProposalInvalidState);
    }
}

/// Validate that the payload carried in `kind` satisfies the same bounds
/// enforced by the corresponding live setter.
pub(crate) fn validate_kind(env: &Env, kind: &GovernanceProposalKind) {
    match kind {
        GovernanceProposalKind::SetProtocolFeeBps(bps) => {
            if *bps > MAX_FEE_BPS {
                env.panic_with_error(Error::InvalidProtocolParameters);
            }
        }
        GovernanceProposalKind::SetGovernedParams(params) => {
            if params.protocol_fee_bps > MAX_FEE_BPS {
                env.panic_with_error(Error::InvalidProtocolParameters);
            }
            storage_validation::validate_escrow_total_cap(env, params.max_escrow_total_stroops);
            if params.max_escrow_total_stroops <= 0 {
                env.panic_with_error(Error::InvalidProtocolParameters);
            }
        }
        GovernanceProposalKind::SetFeeWithdrawalCap(cap_bps) => {
            if *cap_bps > 10_000 {
                env.panic_with_error(Error::InvalidProtocolParameters);
            }
        }
        GovernanceProposalKind::SetFeeWithdrawalCooldown(cooldown) => {
            if *cooldown > 2_592_000 {
                env.panic_with_error(Error::InvalidProtocolParameters);
            }
        }
        GovernanceProposalKind::SetMaxMilestones(max) => {
            if *max < crate::MIN_MAX_MILESTONES || *max > crate::MAX_MAX_MILESTONES {
                env.panic_with_error(Error::LimitOutOfRange);
            }
        }
    }
}

/// Apply the side-effects of an approved proposal.  All mutations follow
/// the same patterns as the existing single-step setters in `governance.rs`.
pub(crate) fn apply_kind(env: &Env, kind: &GovernanceProposalKind) {
    match kind {
        GovernanceProposalKind::SetProtocolFeeBps(new_bps) => {
            let old_bps: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::ProtocolFeeBps)
                .unwrap_or(0u32);
            env.storage()
                .persistent()
                .set(&DataKey::ProtocolFeeBps, new_bps);
            env.events().publish(
                (Symbol::new(env, "protocol_fee_bps"),),
                (old_bps, *new_bps, env.ledger().timestamp()),
            );
        }
        GovernanceProposalKind::SetGovernedParams(new_params) => {
            let old_params: Option<GovernedParameters> =
                env.storage().persistent().get(&DataKey::GovernedParameters);
            env.storage()
                .persistent()
                .set(&DataKey::GovernedParameters, new_params);
            crate::ttl::extend_governed_parameters_ttl(env);
            // Update readiness checklist
            let mut checklist: crate::ReadinessChecklist = env
                .storage()
                .persistent()
                .get(&DataKey::ReadinessChecklist)
                .unwrap_or_default();
            checklist.governed_params_set = true;
            env.storage()
                .persistent()
                .set(&DataKey::ReadinessChecklist, &checklist);
            env.events().publish(
                (Symbol::new(env, "governed_parameters"),),
                (old_params, new_params.clone(), env.ledger().timestamp()),
            );
        }
        GovernanceProposalKind::SetFeeWithdrawalCap(new_cap) => {
            let old_cap: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::FeeWithdrawalCap)
                .unwrap_or(5_000u32);
            env.storage()
                .persistent()
                .set(&DataKey::FeeWithdrawalCap, new_cap);
            env.events().publish(
                (Symbol::new(env, "fee_cap"),),
                (old_cap, *new_cap, env.ledger().timestamp()),
            );
        }
        GovernanceProposalKind::SetFeeWithdrawalCooldown(new_cooldown) => {
            let old_cooldown: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::FeeWithdrawalCooldownLedgers)
                .unwrap_or(17_280u32);
            env.storage()
                .persistent()
                .set(&DataKey::FeeWithdrawalCooldownLedgers, new_cooldown);
            env.events().publish(
                (Symbol::new(env, "fee_cooldown"),),
                (old_cooldown, *new_cooldown, env.ledger().timestamp()),
            );
        }
        GovernanceProposalKind::SetMaxMilestones(new_max) => {
            env.storage()
                .persistent()
                .set(&DataKey::MaxMilestones, new_max);
            env.events().publish(
                (Symbol::new(env, "max_milestones"),),
                (*new_max, env.ledger().timestamp()),
            );
        }
    }
}

// ── Public contract entrypoints ───────────────────────────────────────────────
