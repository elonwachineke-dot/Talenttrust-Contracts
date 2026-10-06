//! Amount validation and input sanitization tests.
//!
//! These tests exercise the pure validation helpers directly so they remain
//! independent from the full contract client stack.

use crate::{
    accumulate_amounts, safe_add_amounts, safe_subtract_amounts, validate_deposit_amount,
    validate_milestone_amounts, validate_single_amount, AmountBoundary, EscrowError,
    MAX_SINGLE_AMOUNT_STROOPS, MAX_TOTAL_ESCROW_STROOPS,
};

#[test]
fn classify_amount_handles_non_positive_and_boundary_values() {
    assert_eq!(crate::classify_amount(0), AmountBoundary::NonPositive);
    assert_eq!(crate::classify_amount(-1), AmountBoundary::NonPositive);
    assert_eq!(crate::classify_amount(1), AmountBoundary::WithinBounds);
    assert_eq!(crate::classify_amount(MAX_SINGLE_AMOUNT_STROOPS), AmountBoundary::WithinBounds);
    assert_eq!(crate::classify_amount(MAX_SINGLE_AMOUNT_STROOPS + 1), AmountBoundary::AboveMaximum);
}

#[test]
fn validate_single_amount_rejects_invalid_values() {
    assert!(validate_single_amount(0).is_err());
    assert!(validate_single_amount(-1).is_err());
    assert!(validate_single_amount(MAX_SINGLE_AMOUNT_STROOPS).is_ok());
    assert!(validate_single_amount(MAX_SINGLE_AMOUNT_STROOPS + 1).is_err());
}

#[test]
fn validate_milestone_amounts_requires_positive_total_and_cap() {
    let amounts = [10_000_000_i128, 20_000_000_i128];
    assert!(validate_milestone_amounts(&amounts, 100_000_000).is_ok());
    assert!(validate_milestone_amounts(&amounts, 20_000_000).is_err());
    assert!(validate_milestone_amounts(&[-1_i128], 1_000_000).is_err());
}

#[test]
fn validate_deposit_amount_catches_overflows_and_limits() {
    assert!(validate_deposit_amount(100, 0, 1_000).is_ok());
    assert!(validate_deposit_amount(0, 0, 1_000).is_err());
    assert!(validate_deposit_amount(100, 1_000_000, 1_000).is_err());
    assert!(validate_deposit_amount(i128::MAX, 1, 1_000).is_err());
}

#[test]
fn safe_arithmetic_uses_checked_operations() {
    assert_eq!(safe_add_amounts(10, 5), Some(15));
    assert_eq!(safe_add_amounts(i128::MAX, 1), None);
    assert_eq!(safe_subtract_amounts(10, 5), Some(5));
    assert_eq!(safe_subtract_amounts(5, 10), None);
    assert_eq!(accumulate_amounts([1, 2, 3]), Ok(6));
    assert!(accumulate_amounts([1, i128::MAX]).is_err());
}
// -----------------------------------------------------------------------------

#[test]
fn test_single_amount_validation() {
    // Valid amounts
    assert!(validate_single_amount(1).is_ok()); // Minimum positive
    assert!(validate_single_amount(100_0000000).is_ok()); // 1 token
    assert!(validate_single_amount(1_000_000_0000000).is_ok()); // Max single amount

    // Invalid amounts
    assert_eq!(
        validate_single_amount(0),
        Err(EscrowError::AmountMustBePositive)
    );
    assert_eq!(
        validate_single_amount(-1),
        Err(EscrowError::AmountMustBePositive)
    );
    assert_eq!(
        validate_single_amount(-100_0000000),
        Err(EscrowError::AmountMustBePositive)
    );
    assert_eq!(
        validate_single_amount(1_000_000_0000001),
        Err(EscrowError::InvalidMilestoneAmount)
    );
}

#[test]
fn test_milestone_amounts_validation() {
    let max_total = MAX_TOTAL_ESCROW_STROOPS;

    // Valid milestone arrays
    let milestones1 = [100_0000000, 200_0000000, 300_0000000];
    assert!(validate_milestone_amounts(&milestones1, max_total).is_ok());
    assert_eq!(
        validate_milestone_amounts(&milestones1, max_total).unwrap(),
        600_0000000
    );

    // Single milestone at maximum
    let milestones2 = [max_total];
    assert!(validate_milestone_amounts(&milestones2, max_total).is_ok());

    // Multiple milestones within bounds
    let milestones3 = [500_000_0000000, 500_000_0000000];
    assert!(validate_milestone_amounts(&milestones3, max_total).is_ok());

    // Invalid arrays
    let milestones4 = [100_0000000, 0, 300_0000000]; // Contains zero
    assert_eq!(
        validate_milestone_amounts(&milestones4, max_total),
        Err(EscrowError::AmountMustBePositive)
    );

    let milestones5 = [100_0000000, -50_0000000, 300_0000000]; // Contains negative
    assert_eq!(
        validate_milestone_amounts(&milestones5, max_total),
        Err(EscrowError::AmountMustBePositive)
    );

    let milestones6 = [600_000_0000000, 500_000_0000000]; // Exceeds contract max
    assert_eq!(
        validate_milestone_amounts(&milestones6, max_total),
        Err(EscrowError::InvalidMilestoneAmount)
    );
}

#[test]
fn test_deposit_amount_validation() {
    let max_total = MAX_TOTAL_ESCROW_STROOPS;

    // Valid deposits
    assert!(validate_deposit_amount(100_0000000, 0, max_total).is_ok());
    assert!(validate_deposit_amount(100_0000000, 500_0000000, max_total).is_ok());
    assert!(validate_deposit_amount(max_total, 0, max_total).is_ok());

    // Invalid deposits
    assert_eq!(
        validate_deposit_amount(0, 0, max_total),
        Err(EscrowError::AmountMustBePositive)
    );
    assert_eq!(
        validate_deposit_amount(-1, 0, max_total),
        Err(EscrowError::AmountMustBePositive)
    );

    // Would exceed maximum
    assert_eq!(
        validate_deposit_amount(600_000_0000000, 500_000_0000000, max_total),
        Err(EscrowError::InvalidMilestoneAmount)
    );

    // Single amount exceeds maximum
    assert_eq!(
        validate_deposit_amount(1_000_000_0000001, 0, max_total),
        Err(EscrowError::InvalidMilestoneAmount)
    );
}

#[test]
fn test_safe_arithmetic_operations() {
    // Safe addition
    assert_eq!(safe_add_amounts(100, 200), Some(300));
    assert_eq!(safe_add_amounts(0, 0), Some(0));
    assert_eq!(safe_add_amounts(i128::MAX, 1), None);
    assert_eq!(safe_add_amounts(i128::MIN, -1), None);

    // Safe subtraction
    assert_eq!(safe_subtract_amounts(300, 100), Some(200));
    assert_eq!(safe_subtract_amounts(100, 100), Some(0));
    assert_eq!(safe_subtract_amounts(0, 1), Some(-1));
    assert_eq!(safe_subtract_amounts(i128::MIN, 1), None);
}

#[test]
fn test_edge_cases() {
    let max_total = MAX_TOTAL_ESCROW_STROOPS;

    // Test minimum positive amounts
    assert!(validate_single_amount(1).is_ok());
    let small_milestones = [1, 1, 1];
    assert!(validate_milestone_amounts(&small_milestones, max_total).is_ok());

    // Test boundary values
    assert!(validate_single_amount(1_000_000_0000000).is_ok()); // Max single amount
    assert_eq!(
        validate_single_amount(1_000_000_0000001),
        Err(EscrowError::InvalidMilestoneAmount)
    );

    // Test contract boundary
    let boundary_milestones = [MAX_TOTAL_ESCROW_STROOPS];
    assert!(validate_milestone_amounts(&boundary_milestones, max_total).is_ok());

    let over_boundary_milestones = [MAX_TOTAL_ESCROW_STROOPS + 1];
    assert_eq!(
        validate_milestone_amounts(&over_boundary_milestones, max_total),
        Err(EscrowError::InvalidMilestoneAmount)
    );
}

#[test]
fn test_stroop_precision() {
    // All i128 values are valid stroop amounts since stroop is the smallest unit
    // This test documents the precision requirements
    let valid_stroop_amounts = [
        1,           // 1 stroop
        100,         // 100 stroops
        1_0000000,   // 1 token
        123_4567890, // 123.4567890 tokens
    ];

    for amount in valid_stroop_amounts {
        assert!(validate_single_amount(amount).is_ok());
    }
}

#[test]
fn test_large_amount_arrays() {
    let max_total = MAX_TOTAL_ESCROW_STROOPS;

    // Test with maximum number of milestones (10)
    let many_milestones = [100_0000000; 10]; // 1 token each
    assert!(validate_milestone_amounts(&many_milestones, max_total).is_ok());
    assert_eq!(
        validate_milestone_amounts(&many_milestones, max_total).unwrap(),
        1_000_000_000
    );

    // Array with one too many milestones must be rejected by the milestone count bound.
    let too_many_milestones = [100_0000000; 11];
    assert_eq!(
        validate_milestone_amounts(&too_many_milestones, max_total),
        Err(EscrowError::InvalidMilestoneAmount)
    );
}

// -----------------------------------------------------------------------------
// Duplicate submission and concurrency-safety tests
// -----------------------------------------------------------------------------

#[test]
fn test_duplicate_milestone_amounts_are_accepted_and_summed_once() {
    // Duplicate values in the milestone array are not duplicate submissions;
    // each entry is a distinct milestone and must be summed exactly once.
    let max_total = MAX_TOTAL_ESCROW_STROOPS;
    let milestones = [100_0000000, 100_0000000, 100_0000000];
    assert_eq!(
        validate_milestone_amounts(&milestones, max_total).unwrap(),
        300_0000000
    );
}

#[test]
fn test_duplicate_deposits_accumulate_deterministically() {
    // Repeated deposits are not rejected as duplicates; they accumulate and
    // must never exceed the contract maximum. This documents the invariant
    // that deposit validation is based on the current balance, not on a
    // per-caller duplicate tracker.
    let max_total = MAX_TOTAL_ESCROW_STROOPS;
    assert!(validate_deposit_amount(100_0000000, 0, max_total).is_ok());
    assert!(validate_deposit_amount(100_0000000, 100_0000000, max_total).is_ok());
    assert!(validate_deposit_amount(100_0000000, 200_0000000, max_total).is_ok());
    // Once the accumulated balance reaches the maximum, any further deposit
    // must be rejected deterministically.
    assert_eq!(
        validate_deposit_amount(1, 1000_000_0000000, max_total),
        Err(EscrowError::InvalidMilestoneAmount)
    );
}

#[test]
fn test_concurrent_deposit_validation_is_deterministic() {
    // Validation is a pure function of (amount, current_balance, max_total).
    // Concurrent callers observe the same result for the same inputs, and the
    // contract enforces the invariant on each state transition.
    let max_total = MAX_TOTAL_ESCROW_STROOPS;
    for _ in 0..10 {
        assert!(validate_deposit_amount(100_0000000, 0, max_total).is_ok());
        assert_eq!(
            validate_deposit_amount(0, 0, max_total),
            Err(EscrowError::AmountMustBePositive)
        );
    }
}
