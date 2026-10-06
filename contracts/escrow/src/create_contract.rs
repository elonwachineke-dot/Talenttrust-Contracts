use crate::{
    amount_validation, keys, token_scale, ttl, Contract, ContractStatus, DataKey, Error,
    Escrow, EscrowClient, EscrowError, GovernedParameters, Milestone, ReleaseAuthorization,
    MAX_MAX_MILESTONES, MAX_MILESTONES, MIN_MAX_MILESTONES,
};
use soroban_sdk::{symbol_short, Address, Env, Vec};

impl Escrow {
    /// Atomically reserves and returns the next available contract ID.
    ///
    /// This is the concurrency-safe counterpart to [`Self::next_contract_id`].
    /// It performs a single read-modify-write of `DataKey::NextContractId`:
    /// the incremented value is persisted *before* the id is returned to the
    /// caller, so a concurrent or re-entrant invocation that reaches this
    /// point cannot observe the same id. The collision check is retained as a
    /// defensive invariant: if the reserved slot is already occupied the
    /// reservation is aborted with `ContractIdCollision` rather than silently
    /// overwriting existing state.
    ///
    /// # Errors
    /// * `ContractIdOverflow`  - If the next id would exceed `u32::MAX`
    /// * `ContractIdCollision` - If the reserved id slot is already occupied
    pub(crate) fn reserve_contract_id(env: &Env) -> u32 {
        let id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::NextContractId)
            .unwrap_or(1);

        if env
            .storage()
            .persistent()
            .get::<_, Contract>(&DataKey::Contract(id))
            .is_some()
        {
            env.panic_with_error(Error::ContractIdCollision);
        }

        let next_id = id
            .checked_add(1)
            .unwrap_or_else(|| env.panic_with_error(Error::ContractIdOverflow));
        env.storage()
            .persistent()
            .set(&DataKey::NextContractId, &next_id);

        id
    }

    /// Returns the next available contract ID and asserts it is not already occupied.
    ///
    /// # Errors
    /// * `ContractIdCollision` - If either key for the allocated ID is occupied
    pub(crate) fn next_contract_id(env: &Env) -> u32 {
        let id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::NextContractId)
            .unwrap_or(1);

        if id == 0 {
            env.panic_with_error(Error::ContractIdOverflow);
        }

        if env
            .storage()
            .persistent()
            .get::<_, Contract>(&DataKey::Contract(id))
            .is_some()
        {
            env.panic_with_error(Error::ContractIdCollision);
        }

        id
    }
}
