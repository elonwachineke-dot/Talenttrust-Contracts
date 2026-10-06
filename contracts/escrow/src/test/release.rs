use super::{assert_contract_error, EscrowFixture, MILESTONE_ONE};
use crate::{ContractStatus, EscrowError};

#[test]
fn release_funded_milestones_completes_contract() {
    let fixture = EscrowFixture::builder().funded().build();
    let escrow = fixture.escrow();

    for index in 0..3_u32 {
        assert!(escrow.approve_milestone_release(&fixture.escrow_id, &fixture.client, &index));
        assert!(escrow.release_milestone(&fixture.escrow_id, &fixture.client, &index));
    }

    let contract = escrow.get_contract(&fixture.escrow_id);
    assert_eq!(contract.status, ContractStatus::Completed);
    assert_eq!(contract.released_amount, fixture.total_amount());
}

#[test]
fn release_rejects_an_already_released_milestone() {
    let fixture = EscrowFixture::builder().funded().build();
    let escrow = fixture.escrow();
    assert!(escrow.approve_milestone_release(&fixture.escrow_id, &fixture.client, &0_u32));
    assert!(escrow.release_milestone(&fixture.escrow_id, &fixture.client, &0_u32));

    assert!(escrow.approve_milestone_release(&fixture.escrow_id, &fixture.client, &0_u32));
    assert_contract_error(
        escrow.try_release_milestone(&fixture.escrow_id, &fixture.client, &0_u32),
        EscrowError::MilestoneAlreadyReleased,
    );
    assert_eq!(
        escrow.get_contract(&fixture.escrow_id).released_amount,
        MILESTONE_ONE
    );
}
