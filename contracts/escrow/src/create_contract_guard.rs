use crate::types::AdditionalDataKey;
use crate::Error;
use soroban_sdk::Env;

pub(crate) fn begin(env: &Env) -> Result<(), Error> {
    let sequence = env.ledger().sequence();
    let previous: Option<u32> = env
        .storage()
        .instance()
        .get(&AdditionalDataKey::ContractCreationGuard);

    if previous == Some(sequence) {
        env.panic_with_error(Error::ContractCreationInProgress);
    }

    env.storage()
        .instance()
        .set(&AdditionalDataKey::ContractCreationGuard, &sequence);
    Ok(())
}

pub(crate) fn end(_env: &Env) {}
