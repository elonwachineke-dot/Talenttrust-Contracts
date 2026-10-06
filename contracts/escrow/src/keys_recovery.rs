use crate::types::AdditionalDataKey;
use soroban_sdk::{contracttype, Env, String};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyRecoveryStatus {
    InProgress,
    Recovered,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyRecoveryRecord {
    pub contract_id: u32,
    pub status: KeyRecoveryStatus,
    pub reason: String,
    pub started_at: u64,
    pub completed_at: Option<u64>,
}

pub(crate) fn load_recovery_record(env: &Env, contract_id: u32) -> Option<KeyRecoveryRecord> {
    env.storage()
        .persistent()
        .get(&AdditionalDataKey::KeyRecovery(contract_id))
}

pub(crate) fn store_recovery_record(env: &Env, record: &KeyRecoveryRecord) {
    let key = AdditionalDataKey::KeyRecovery(record.contract_id);
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, crate::ttl::PERSISTENT_BUMP_THRESHOLD, crate::ttl::PERSISTENT_TTL_LEDGERS);
}

pub(crate) fn clear_recovery_record(env: &Env, contract_id: u32) {
    env.storage()
        .persistent()
        .remove(&AdditionalDataKey::KeyRecovery(contract_id));
}
