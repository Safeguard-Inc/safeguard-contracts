#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Env};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> token::Client<'a> {
    let sac = e.register_stellar_asset_contract_v2(admin.clone());
    token::Client::new(e, &sac.address())
}

#[test]
fn test_initialize_and_config() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let escrow_period = 3600u64;
    let spend_cap = 1_000_0000000i128; // 1,000 units with 7 decimals

    client.initialize(&admin, &escrow_period, &spend_cap);

    let (cfg_admin, cfg_cap, cfg_period, is_paused, total_escrows) = client.get_config();
    assert_eq!(cfg_admin, admin);
    assert_eq!(cfg_cap, spend_cap);
    assert_eq!(cfg_period, escrow_period);
    assert!(!is_paused);
    assert_eq!(total_escrows, 0);
}

#[test]
fn test_direct_approved_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &1000_0000000);

    // Mint 500 units to sender
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Pay 100 units (under spend cap 1000)
    let receipt = client.pay(&sender, &recipient, &token_client.address, &100_0000000);

    assert_eq!(receipt.status, PaymentStatus::Approved);
    assert_eq!(receipt.amount, 100_0000000);
    assert_eq!(receipt.escrow_id, 0);
    assert_eq!(token_client.balance(&recipient), 100_0000000);
    assert_eq!(token_client.balance(&sender), 400_0000000);
}

#[test]
fn test_spend_cap_routes_to_escrow_and_admin_releases() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Spend cap is 100 units
    client.initialize(&admin, &3600, &100_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &1000_0000000);

    // Pay 500 units (exceeds 100 cap -> routed to escrow)
    let receipt = client.pay(&sender, &recipient, &token_client.address, &500_0000000);

    assert_eq!(receipt.status, PaymentStatus::Escrowed);
    assert_eq!(receipt.escrow_id, 1);
    assert_eq!(token_client.balance(&recipient), 0);
    assert_eq!(token_client.balance(&contract_id), 500_0000000);
    assert_eq!(token_client.balance(&sender), 500_0000000);

    // Admin releases escrow #1
    client.release_escrow(&1);

    assert_eq!(token_client.balance(&recipient), 500_0000000);
    assert_eq!(token_client.balance(&contract_id), 0);

    let escrow = client.get_escrow(&1);
    assert_eq!(escrow.status, EscrowStatus::Released);
}

#[test]
fn test_refund_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &50_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &200_0000000);

    // Exceeds cap -> escrow #1
    client.pay(&sender, &recipient, &token_client.address, &150_0000000);

    // Admin refunds
    client.refund_escrow(&admin, &1);

    assert_eq!(token_client.balance(&sender), 200_0000000);
    assert_eq!(token_client.balance(&contract_id), 0);

    let escrow = client.get_escrow(&1);
    assert_eq!(escrow.status, EscrowStatus::Refunded);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // RecipientDenylisted = 11
fn test_denylist_blocks_recipient() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let blocked_recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &1000_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Add recipient to denylist
    client.add_to_denylist(&blocked_recipient);
    assert!(client.is_denylisted(&blocked_recipient));

    // Must panic / revert
    client.pay(
        &sender,
        &blocked_recipient,
        &token_client.address,
        &50_0000000,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // ContractPaused = 4
fn test_paused_blocks_payments() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &1000_0000000);
    client.set_paused(&true);

    client.pay(&sender, &recipient, &token_client.address, &50_0000000);
}

#[test]
fn test_gas_and_cpu_benchmarks() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // 1. Benchmark initialize
    client.initialize(&admin, &3600, &1000_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &10_000_0000000);

    // 2. Benchmark direct approved payment
    let receipt1 = client.pay(&sender, &recipient, &token_client.address, &100_0000000);
    assert_eq!(receipt1.status, PaymentStatus::Approved);

    // 3. Benchmark spend cap escrow routing
    let receipt2 = client.pay(&sender, &recipient, &token_client.address, &5000_0000000);
    assert_eq!(receipt2.status, PaymentStatus::Escrowed);

    // 4. Benchmark release escrow
    client.release_escrow(&receipt2.escrow_id);

    // 5. Benchmark denylist lookup and insertion
    client.add_to_denylist(&recipient);
    assert!(client.is_denylisted(&recipient));
}

#[contract]
struct MockPolicy;

#[contractimpl]
impl MockPolicy {
    pub fn is_authorized(env: Env, account: Address, _token: Address) -> bool {
        let denied: Option<Address> = env.storage().instance().get(&symbol_short!("deny"));
        if let Some(bad_acct) = denied {
            if account == bad_acct {
                return false;
            }
        }
        true
    }

    pub fn set_deny(env: Env, account: Address) {
        env.storage()
            .instance()
            .set(&symbol_short!("deny"), &account);
    }
}

#[test]
fn test_policy_contract_registration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let policy_id = env.register(MockPolicy, ());

    client.initialize(&admin, &3600, &1000_0000000);
    assert_eq!(client.get_policy_contract(), None);

    client.set_policy_contract(&policy_id);
    assert_eq!(client.get_policy_contract(), Some(policy_id.clone()));

    client.remove_policy_contract();
    assert_eq!(client.get_policy_contract(), None);
}

#[test]
fn test_cross_contract_policy_approves_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &1000_0000000);

    // Register MockPolicy and wire it
    let policy_id = env.register(MockPolicy, ());
    client.set_policy_contract(&policy_id);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Both parties authorized -> direct pay approved
    let receipt = client.pay(&sender, &recipient, &token_client.address, &100_0000000);
    assert_eq!(receipt.status, PaymentStatus::Approved);
    assert_eq!(token_client.balance(&recipient), 100_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")] // PolicyDenied = 7
fn test_cross_contract_policy_denies_unauthorized_party() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &1000_0000000);

    // Register MockPolicy, deny recipient, and wire it
    let policy_id = env.register(MockPolicy, ());
    let mock_client = MockPolicyClient::new(&env, &policy_id);
    mock_client.set_deny(&recipient);

    client.set_policy_contract(&policy_id);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Should revert with PolicyDenied (code 7)
    client.pay(&sender, &recipient, &token_client.address, &100_0000000);
}

#[test]
fn test_batch_denylist_and_getters() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let spend_cap = 500_0000000i128;
    client.initialize(&admin, &7200, &spend_cap);

    assert_eq!(client.get_spend_cap(), spend_cap);
    assert_eq!(client.get_escrow_count(), 0);

    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);
    let mut addrs = soroban_sdk::Vec::new(&env);
    addrs.push_back(addr1.clone());
    addrs.push_back(addr2.clone());

    client.add_to_denylist_batch(&addrs);

    assert!(client.is_denylisted(&addr1));
    assert!(client.is_denylisted(&addr2));

    client.extend_ttl(&50_000, &150_000);
}

#[test]
fn test_two_step_admin_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin, &3600, &1000_0000000);

    // Initial state: pending admin is None
    assert_eq!(client.get_pending_admin(), None);

    // Propose new admin
    client.propose_admin(&new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));

    // Claim admin by new_admin
    client.claim_admin();

    // Verify admin is updated and pending is cleared
    let (current_admin, _, _, _, _) = client.get_config();
    assert_eq!(current_admin, new_admin);
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")] // InvalidEscrowPeriod = 14
fn test_zero_escrow_period_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &0, &1000_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")] // InvalidAmount = 5
fn test_negative_spend_cap_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &3600, &-100);
}

#[test]
fn test_storage_ttl_extension() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &1000_0000000);

    // Instance TTL extension succeeds
    client.extend_instance_ttl(&1000, &5000);

    // Non-existent escrow returns EscrowNotFound
    let res = client.try_extend_escrow_ttl(&999, &1000, &5000);
    assert_eq!(res, Err(Ok(PaymentError::EscrowNotFound)));
}

#[test]
fn test_get_escrows_batch() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &100_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &10_000_0000000);

    let receipt1 = client.pay(&sender, &recipient, &token_client.address, &200_0000000);
    let receipt2 = client.pay(&sender, &recipient, &token_client.address, &300_0000000);

    let mut query_ids = Vec::new(&env);
    query_ids.push_back(receipt1.escrow_id);
    query_ids.push_back(receipt2.escrow_id);
    query_ids.push_back(9999u64);

    let batch = client.get_escrows_batch(&query_ids);
    assert_eq!(batch.len(), 2);
    assert_eq!(batch.get(0).unwrap().id, receipt1.escrow_id);
    assert_eq!(batch.get(1).unwrap().id, receipt2.escrow_id);
}

#[test]
fn test_cumulative_volume_tracking() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &100_0000000);

    assert_eq!(client.get_total_volume(), 0);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &10_000_0000000);

    client.pay(&sender, &recipient, &token_client.address, &50_0000000);
    assert_eq!(client.get_total_volume(), 50_0000000);

    client.pay(&sender, &recipient, &token_client.address, &150_0000000);
    assert_eq!(client.get_total_volume(), 200_0000000);
}

#[test]
fn test_payment_sequence_numbers() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &100_0000000);

    assert_eq!(client.get_payment_sequence(), 0);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &10_000_0000000);

    client.pay(&sender, &recipient, &token_client.address, &10_0000000);
    assert_eq!(client.get_payment_sequence(), 1);

    client.pay(&sender, &recipient, &token_client.address, &500_0000000);
    assert_eq!(client.get_payment_sequence(), 2);
}

#[test]
fn test_min_payment_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &100_0000000);

    assert_eq!(client.get_min_payment_amount(), 1);

    client.set_min_payment_amount(&10_0000000);
    assert_eq!(client.get_min_payment_amount(), 10_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &100_0000000);

    let res = client.try_pay(&sender, &recipient, &token_client.address, &5_0000000);
    assert_eq!(res, Err(Ok(PaymentError::InvalidAmount)));

    let receipt = client.pay(&sender, &recipient, &token_client.address, &15_0000000);
    assert_eq!(receipt.status, PaymentStatus::Approved);
}

#[test]
fn test_is_active_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &50_0000000);

    // Non-existent escrow returns false
    assert!(!client.is_active_escrow(&1));
    assert!(!client.is_active_escrow(&999));

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &200_0000000);

    // Payment exceeding spend cap (75 > 50) routes to escrow id 1
    let receipt = client.pay(&sender, &recipient, &token_client.address, &75_0000000);
    assert_eq!(receipt.status, PaymentStatus::Escrowed);
    assert_eq!(receipt.escrow_id, 1);

    // Active escrow should now be true
    assert!(client.is_active_escrow(&1));

    // After release, escrow is no longer pending, so is_active_escrow returns false
    client.release_escrow(&1);
    assert!(!client.is_active_escrow(&1));
}

#[test]
fn test_get_escrows_by_sender() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender1 = Address::generate(&env);
    let sender2 = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &50_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender1, &200_0000000);
    token_admin_client.mint(&sender2, &200_0000000);

    // Initial check: no escrows
    assert_eq!(client.get_escrows_by_sender(&sender1).len(), 0);

    // Sender 1 creates two escrows (amounts 60 and 70 > 50 cap)
    client.pay(&sender1, &recipient, &token_client.address, &60_0000000);
    client.pay(&sender1, &recipient, &token_client.address, &70_0000000);

    // Sender 2 creates one escrow (amount 80 > 50 cap)
    client.pay(&sender2, &recipient, &token_client.address, &80_0000000);

    let sender1_escrows = client.get_escrows_by_sender(&sender1);
    assert_eq!(sender1_escrows.len(), 2);
    assert_eq!(sender1_escrows.get(0).unwrap().id, 1);
    assert_eq!(sender1_escrows.get(0).unwrap().amount, 60_0000000);
    assert_eq!(sender1_escrows.get(1).unwrap().id, 2);
    assert_eq!(sender1_escrows.get(1).unwrap().amount, 70_0000000);

    let sender2_escrows = client.get_escrows_by_sender(&sender2);
    assert_eq!(sender2_escrows.len(), 1);
    assert_eq!(sender2_escrows.get(0).unwrap().id, 3);
    assert_eq!(sender2_escrows.get(0).unwrap().amount, 80_0000000);

    let stranger = Address::generate(&env);
    assert_eq!(client.get_escrows_by_sender(&stranger).len(), 0);
}

#[test]
fn test_get_escrows_by_recipient() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient_a = Address::generate(&env);
    let recipient_b = Address::generate(&env);

    client.initialize(&admin, &3600, &50_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &300_0000000);

    // Initial check: no escrows
    assert_eq!(client.get_escrows_by_recipient(&recipient_a).len(), 0);

    // Sender sends 2 payments to recipient A (65 and 75 > 50 cap)
    client.pay(&sender, &recipient_a, &token_client.address, &65_0000000);
    client.pay(&sender, &recipient_a, &token_client.address, &75_0000000);

    // Sender sends 1 payment to recipient B (90 > 50 cap)
    client.pay(&sender, &recipient_b, &token_client.address, &90_0000000);

    let rec_a_escrows = client.get_escrows_by_recipient(&recipient_a);
    assert_eq!(rec_a_escrows.len(), 2);
    assert_eq!(rec_a_escrows.get(0).unwrap().id, 1);
    assert_eq!(rec_a_escrows.get(0).unwrap().amount, 65_0000000);
    assert_eq!(rec_a_escrows.get(1).unwrap().id, 2);
    assert_eq!(rec_a_escrows.get(1).unwrap().amount, 75_0000000);

    let rec_b_escrows = client.get_escrows_by_recipient(&recipient_b);
    assert_eq!(rec_b_escrows.len(), 1);
    assert_eq!(rec_b_escrows.get(0).unwrap().id, 3);
    assert_eq!(rec_b_escrows.get(0).unwrap().amount, 90_0000000);

    let stranger = Address::generate(&env);
    assert_eq!(client.get_escrows_by_recipient(&stranger).len(), 0);
}

#[test]
fn test_dispute_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.initialize(&admin, &3600, &50_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &200_0000000);

    // Escrow id 1 created
    let receipt = client.pay(&sender, &recipient, &token_client.address, &75_0000000);
    assert_eq!(receipt.status, PaymentStatus::Escrowed);
    assert_eq!(receipt.escrow_id, 1);

    // Stranger cannot dispute
    let res = client.try_dispute_escrow(&stranger, &1);
    assert_eq!(res, Err(Ok(PaymentError::InvalidDisputeParty)));

    // Recipient disputes
    client.dispute_escrow(&recipient, &1);

    let mut query_ids = Vec::new(&env);
    query_ids.push_back(1);
    let batch = client.get_escrows_batch(&query_ids);
    let record = batch.get(0).unwrap();
    assert_eq!(record.status, EscrowStatus::Disputed);

    // Cannot dispute again
    let res2 = client.try_dispute_escrow(&sender, &1);
    assert_eq!(res2, Err(Ok(PaymentError::EscrowAlreadySettled)));
}

#[test]
fn test_arbitrate_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin, &3600, &50_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Escrow 1: route to recipient
    let r1 = client.pay(&sender, &recipient, &token_client.address, &100_0000000);
    assert_eq!(r1.escrow_id, 1);
    client.dispute_escrow(&sender, &1);

    client.arbitrate_escrow(&1, &true);
    assert_eq!(token_client.balance(&recipient), 100_0000000);

    let mut q1 = Vec::new(&env);
    q1.push_back(1);
    assert_eq!(
        client.get_escrows_batch(&q1).get(0).unwrap().status,
        EscrowStatus::Released
    );

    // Cannot arbitrate already settled escrow
    let err = client.try_arbitrate_escrow(&1, &false);
    assert_eq!(err, Err(Ok(PaymentError::EscrowAlreadySettled)));

    // Escrow 2: route refund to sender
    let r2 = client.pay(&sender, &recipient, &token_client.address, &150_0000000);
    assert_eq!(r2.escrow_id, 2);
    assert_eq!(token_client.balance(&sender), 250_0000000);

    client.dispute_escrow(&recipient, &2);
    client.arbitrate_escrow(&2, &false);
    assert_eq!(token_client.balance(&sender), 400_0000000);

    let mut q2 = Vec::new(&env);
    q2.push_back(2);
    assert_eq!(
        client.get_escrows_batch(&q2).get(0).unwrap().status,
        EscrowStatus::Refunded
    );
}

#[test]
fn test_get_disputed_escrows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let sender = Address::generate(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipient3 = Address::generate(&env);

    client.initialize(&admin, &3600, &50_0000000);

    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &1000_0000000);

    client.pay(&sender, &recipient1, &token_client.address, &100_0000000); // id 1: pending
    client.pay(&sender, &recipient2, &token_client.address, &100_0000000); // id 2: disputed
    client.pay(&sender, &recipient3, &token_client.address, &100_0000000); // id 3: disputed

    client.dispute_escrow(&sender, &2);
    client.dispute_escrow(&recipient3, &3);

    let disputed = client.get_disputed_escrows();
    assert_eq!(disputed.len(), 2);
    assert_eq!(disputed.get(0).unwrap().id, 2);
    assert_eq!(disputed.get(1).unwrap().id, 3);

    // After arbitrating id 2, only id 3 remains disputed
    client.arbitrate_escrow(&2, &true);
    let disputed_after = client.get_disputed_escrows();
    assert_eq!(disputed_after.len(), 1);
    assert_eq!(disputed_after.get(0).unwrap().id, 3);
}

#[test]
fn test_remove_batch_from_denylist() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &100_0000000);

    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);
    let addr3 = Address::generate(&env);

    let mut add_list = soroban_sdk::Vec::new(&env);
    add_list.push_back(addr1.clone());
    add_list.push_back(addr2.clone());
    add_list.push_back(addr3.clone());

    client.add_to_denylist_batch(&add_list);
    assert!(client.is_denylisted(&addr1));
    assert!(client.is_denylisted(&addr2));
    assert!(client.is_denylisted(&addr3));

    let mut remove_list = soroban_sdk::Vec::new(&env);
    remove_list.push_back(addr1.clone());
    remove_list.push_back(addr2.clone());

    client.remove_from_denylist_batch(&remove_list);
    assert!(!client.is_denylisted(&addr1));
    assert!(!client.is_denylisted(&addr2));
    assert!(client.is_denylisted(&addr3));
}

#[test]
fn test_get_denylist_batch() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &100_0000000);

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let a4 = Address::generate(&env);

    let mut to_block = soroban_sdk::Vec::new(&env);
    to_block.push_back(a2.clone());
    to_block.push_back(a4.clone());
    client.add_to_denylist_batch(&to_block);

    let mut query = soroban_sdk::Vec::new(&env);
    query.push_back(a1.clone());
    query.push_back(a2.clone());
    query.push_back(a3.clone());
    query.push_back(a4.clone());

    let results = client.get_denylist_batch(&query);
    assert_eq!(results.len(), 4);
    assert_eq!(results.get(0).unwrap(), false);
    assert_eq!(results.get(1).unwrap(), true);
    assert_eq!(results.get(2).unwrap(), false);
    assert_eq!(results.get(3).unwrap(), true);
}

#[test]
fn test_cancel_admin_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &100_0000000);

    let candidate = Address::generate(&env);
    client.propose_admin(&candidate);
    assert_eq!(client.get_pending_admin(), Some(candidate.clone()));

    // Cancel proposal
    client.cancel_admin_transfer();
    assert_eq!(client.get_pending_admin(), None);

    // Candidate cannot claim after cancellation
    let res = client.try_claim_admin();
    assert_eq!(res, Err(Ok(PaymentError::PendingAdminNotFound)));

    // Cancelling when none pending fails
    let res2 = client.try_cancel_admin_transfer();
    assert_eq!(res2, Err(Ok(PaymentError::PendingAdminNotFound)));
}

#[test]
fn test_simulate_payment_batch() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);
    let mallory = Address::generate(&env);
    let dave = Address::generate(&env);
    let token = Address::generate(&env);

    client.add_to_denylist(&mallory);

    let mut payments = soroban_sdk::Vec::new(&env);
    payments.push_back(PaymentCheckParam {
        recipient: bob.clone(),
        token: token.clone(),
        amount: 10_0000000,
    });
    payments.push_back(PaymentCheckParam {
        recipient: charlie.clone(),
        token: token.clone(),
        amount: 100_0000000,
    });
    payments.push_back(PaymentCheckParam {
        recipient: mallory.clone(),
        token: token.clone(),
        amount: 5_0000000,
    });
    payments.push_back(PaymentCheckParam {
        recipient: dave.clone(),
        token: token.clone(),
        amount: 0,
    });

    let verdicts = client.simulate_payment_batch(&sender, &payments);
    assert_eq!(verdicts.len(), 4);

    // Bob: Approved
    assert_eq!(verdicts.get(0).unwrap().status, PaymentStatus::Approved);
    assert_eq!(verdicts.get(0).unwrap().reason_code, 200);

    // Charlie: Escrowed
    assert_eq!(verdicts.get(1).unwrap().status, PaymentStatus::Escrowed);
    assert_eq!(verdicts.get(1).unwrap().reason_code, 201);

    // Mallory: Denylisted
    assert_eq!(verdicts.get(2).unwrap().status, PaymentStatus::Blocked);
    assert_eq!(verdicts.get(2).unwrap().reason_code, 111);

    // Dave: Zero amount
    assert_eq!(verdicts.get(3).unwrap().status, PaymentStatus::Blocked);
    assert_eq!(verdicts.get(3).unwrap().reason_code, 105);
}
#[test]
fn test_guardian_pause_circuit_breaker() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &100_0000000);

    let guardian = Address::generate(&env);
    let stranger = Address::generate(&env);

    // Initial guardian is none
    assert_eq!(client.get_guardian(), None);

    // Admin sets guardian
    client.set_guardian(&guardian);
    assert_eq!(client.get_guardian(), Some(guardian.clone()));

    // Stranger cannot trigger guardian pause
    let unauthorized_res = client.try_pause_by_guardian(&stranger);
    assert_eq!(unauthorized_res, Err(Ok(PaymentError::Unauthorized)));
    assert!(!client.is_paused());

    // Guardian successfully pauses
    let pause_res = client.pause_by_guardian(&guardian);
    assert_eq!(pause_res, ());
    assert!(client.is_paused());

    // Admin unpauses
    client.set_paused(&false);
    assert!(!client.is_paused());

    // Admin can also trigger pause_by_guardian
    client.pause_by_guardian(&admin);
    assert!(client.is_paused());
}
#[test]
fn test_per_token_spend_cap_routing() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let token_a = Address::generate(&env); // uses global 50
    let token_b = Address::generate(&env); // custom high cap 200
    let token_c = Address::generate(&env); // custom low cap 5

    // Default fallback to global spend cap
    assert_eq!(client.get_effective_spend_cap(&token_a), 50_0000000);
    assert_eq!(client.get_token_spend_cap(&token_a), None);

    // Admin sets per-token spend caps
    client.set_token_spend_cap(&token_b, &200_0000000);
    client.set_token_spend_cap(&token_c, &5_0000000);

    assert_eq!(client.get_token_spend_cap(&token_b), Some(200_0000000));
    assert_eq!(client.get_effective_spend_cap(&token_b), 200_0000000);
    assert_eq!(client.get_effective_spend_cap(&token_c), 5_0000000);

    // Test simulation routing with token caps
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let mut batch = soroban_sdk::Vec::new(&env);
    // Token B with 100 is below 200 -> Approved
    batch.push_back(PaymentCheckParam {
        recipient: recipient.clone(),
        token: token_b.clone(),
        amount: 100_0000000,
    });
    // Token C with 10 is above 5 -> Escrowed
    batch.push_back(PaymentCheckParam {
        recipient: recipient.clone(),
        token: token_c.clone(),
        amount: 10_0000000,
    });

    let verdicts = client.simulate_payment_batch(&sender, &batch);
    assert_eq!(verdicts.get(0).unwrap().status, PaymentStatus::Approved);
    assert_eq!(verdicts.get(1).unwrap().status, PaymentStatus::Escrowed);

    // Remove token cap restores fallback
    client.remove_token_spend_cap(&token_b);
    assert_eq!(client.get_token_spend_cap(&token_b), None);
    assert_eq!(client.get_effective_spend_cap(&token_b), 50_0000000);
}
#[test]
fn test_get_allowlist_batch() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &100_0000000);

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let a4 = Address::generate(&env);

    client.add_to_allowlist(&a1);
    client.add_to_allowlist(&a3);

    assert!(client.is_allowlisted(&a1));
    assert!(!client.is_allowlisted(&a2));
    assert!(client.is_allowlisted(&a3));
    assert!(!client.is_allowlisted(&a4));

    let mut query = soroban_sdk::Vec::new(&env);
    query.push_back(a1.clone());
    query.push_back(a2.clone());
    query.push_back(a3.clone());
    query.push_back(a4.clone());

    let results = client.get_allowlist_batch(&query);
    assert_eq!(results.len(), 4);
    assert_eq!(results.get(0).unwrap(), true);
    assert_eq!(results.get(1).unwrap(), false);
    assert_eq!(results.get(2).unwrap(), true);
    assert_eq!(results.get(3).unwrap(), false);

    // Remove a1
    client.remove_from_allowlist(&a1);
    assert!(!client.is_allowlisted(&a1));

    let mut query2 = soroban_sdk::Vec::new(&env);
    query2.push_back(a1.clone());
    query2.push_back(a3.clone());
    let results2 = client.get_allowlist_batch(&query2);
    assert_eq!(results2.get(0).unwrap(), false);
    assert_eq!(results2.get(1).unwrap(), true);
}
#[test]
fn test_release_escrow_partial() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &200_0000000);

    // Pay 100 -> Routed to escrow ID 1
    let receipt = client.pay(&sender, &recipient, &token_client.address, &100_0000000);
    assert_eq!(receipt.status, PaymentStatus::Escrowed);
    assert_eq!(receipt.escrow_id, 1);

    let initial_escrow = client.get_escrow(&1);
    assert_eq!(initial_escrow.amount, 100_0000000);
    assert_eq!(initial_escrow.status, EscrowStatus::Pending);

    // 1. Partial release of 40
    client.release_escrow_partial(&1, &40_0000000);
    assert_eq!(token_client.balance(&recipient), 40_0000000);

    let intermediate_escrow = client.get_escrow(&1);
    assert_eq!(intermediate_escrow.amount, 60_0000000);
    assert_eq!(intermediate_escrow.status, EscrowStatus::Pending);

    // 2. Reject over-release (requesting 70 when only 60 remain)
    let over_res = client.try_release_escrow_partial(&1, &70_0000000);
    assert_eq!(over_res, Err(Ok(PaymentError::InvalidAmount)));

    // 3. Final release of remaining 60
    client.release_escrow_partial(&1, &60_0000000);
    assert_eq!(token_client.balance(&recipient), 100_0000000);

    let final_escrow = client.get_escrow(&1);
    assert_eq!(final_escrow.amount, 0);
    assert_eq!(final_escrow.status, EscrowStatus::Released);

    // 4. Settled escrow cannot be released again
    let settled_res = client.try_release_escrow_partial(&1, &10_0000000);
    assert_eq!(settled_res, Err(Ok(PaymentError::EscrowAlreadySettled)));
}

#[test]
fn test_extend_dispute_period() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &200_0000000);

    // Escrow payment of 100
    let receipt = client.pay(&sender, &recipient, &token_client.address, &100_0000000);
    assert_eq!(receipt.status, PaymentStatus::Escrowed);
    let initial_escrow = client.get_escrow(&receipt.escrow_id);
    let initial_release_after = initial_escrow.release_after;

    // 1. Extend while Pending
    let new_release = client.extend_dispute_period(&receipt.escrow_id, &1800);
    assert_eq!(new_release, initial_release_after + 1800);
    assert_eq!(
        client.get_escrow(&receipt.escrow_id).release_after,
        initial_release_after + 1800
    );

    // 2. Put into Disputed status
    client.dispute_escrow(&sender, &receipt.escrow_id);
    assert_eq!(
        client.get_escrow(&receipt.escrow_id).status,
        EscrowStatus::Disputed
    );

    // 3. Extend while Disputed
    let disputed_release = client.extend_dispute_period(&receipt.escrow_id, &3600);
    assert_eq!(disputed_release, initial_release_after + 1800 + 3600);

    // 4. Reject extension with 0 duration
    let zero_res = client.try_extend_dispute_period(&receipt.escrow_id, &0);
    assert_eq!(zero_res, Err(Ok(PaymentError::InvalidEscrowPeriod)));

    // 5. Arbitrate and resolve
    client.arbitrate_escrow(&receipt.escrow_id, &true);
    assert_eq!(
        client.get_escrow(&receipt.escrow_id).status,
        EscrowStatus::Released
    );

    // 6. Cannot extend settled escrow
    let settled_res = client.try_extend_dispute_period(&receipt.escrow_id, &1000);
    assert_eq!(settled_res, Err(Ok(PaymentError::EscrowAlreadySettled)));
}

#[test]
fn test_batch_extend_escrows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Create 2 escrows
    let r1 = client.pay(&sender, &recipient, &token_client.address, &100_0000000);
    let r2 = client.pay(&sender, &recipient, &token_client.address, &150_0000000);

    let init1 = client.get_escrow(&r1.escrow_id).release_after;
    let init2 = client.get_escrow(&r2.escrow_id).release_after;

    // Batch extend escrows 1 and 2 by 2400 seconds
    let mut ids = Vec::new(&env);
    ids.push_back(r1.escrow_id);
    ids.push_back(r2.escrow_id);
    let extended_count = client.batch_extend_escrows(&ids, &2400);

    assert_eq!(extended_count, 2);
    assert_eq!(client.get_escrow(&r1.escrow_id).release_after, init1 + 2400);
    assert_eq!(client.get_escrow(&r2.escrow_id).release_after, init2 + 2400);

    // Reject 0 duration
    let zero_res = client.try_batch_extend_escrows(&ids, &0);
    assert_eq!(zero_res, Err(Ok(PaymentError::InvalidEscrowPeriod)));
}

#[test]
fn test_batch_release_escrows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let rec1 = Address::generate(&env);
    let rec2 = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Create 2 escrows
    let r1 = client.pay(&sender, &rec1, &token_client.address, &100_0000000);
    let r2 = client.pay(&sender, &rec2, &token_client.address, &150_0000000);

    assert_eq!(token_client.balance(&rec1), 0);
    assert_eq!(token_client.balance(&rec2), 0);

    let mut ids = Vec::new(&env);
    ids.push_back(r1.escrow_id);
    ids.push_back(r2.escrow_id);

    // Batch release escrows 1 and 2
    let released_count = client.batch_release_escrows(&ids);
    assert_eq!(released_count, 2);

    assert_eq!(token_client.balance(&rec1), 100_0000000);
    assert_eq!(token_client.balance(&rec2), 150_0000000);

    assert_eq!(
        client.get_escrow(&r1.escrow_id).status,
        EscrowStatus::Released
    );
    assert_eq!(
        client.get_escrow(&r2.escrow_id).status,
        EscrowStatus::Released
    );

    // Settled escrows cannot be released again
    let settled_res = client.try_batch_release_escrows(&ids);
    assert_eq!(settled_res, Err(Ok(PaymentError::EscrowAlreadySettled)));
}

#[test]
fn test_get_escrow_metrics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let rec1 = Address::generate(&env);
    let rec2 = Address::generate(&env);
    let rec3 = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &1000_0000000);

    // Initial metrics: all zero
    let init_metrics = client.get_escrow_metrics();
    assert_eq!(init_metrics.total_escrows, 0);
    assert_eq!(init_metrics.pending_count, 0);
    assert_eq!(init_metrics.disputed_count, 0);
    assert_eq!(init_metrics.settled_count, 0);
    assert_eq!(init_metrics.total_locked_amount, 0);

    // Create 3 escrows
    let _r1 = client.pay(&sender, &rec1, &token_client.address, &100_0000000);
    let r2 = client.pay(&sender, &rec2, &token_client.address, &200_0000000);
    let r3 = client.pay(&sender, &rec3, &token_client.address, &300_0000000);

    // Put escrow 2 into Disputed status
    client.dispute_escrow(&sender, &r2.escrow_id);

    // Settle escrow 3 via release
    client.release_escrow(&r3.escrow_id);

    let metrics = client.get_escrow_metrics();
    assert_eq!(metrics.total_escrows, 3);
    assert_eq!(metrics.pending_count, 1);
    assert_eq!(metrics.disputed_count, 1);
    assert_eq!(metrics.settled_count, 1);
    assert_eq!(metrics.total_locked_amount, 300_0000000); // 100 + 200
}

#[test]
fn test_batch_refund_escrows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let rec1 = Address::generate(&env);
    let rec2 = Address::generate(&env);
    let stranger = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Create 2 escrows
    let r1 = client.pay(&sender, &rec1, &token_client.address, &100_0000000);
    let r2 = client.pay(&sender, &rec2, &token_client.address, &150_0000000);

    assert_eq!(token_client.balance(&sender), 250_0000000);

    let mut ids = Vec::new(&env);
    ids.push_back(r1.escrow_id);
    ids.push_back(r2.escrow_id);

    // Stranger tries to refund -> Unauthorized
    let stranger_res = client.try_batch_refund_escrows(&stranger, &ids);
    assert_eq!(stranger_res, Err(Ok(PaymentError::Unauthorized)));

    // Sender tries to refund before timelock -> EscrowTimelockActive
    let premature_res = client.try_batch_refund_escrows(&sender, &ids);
    assert_eq!(premature_res, Err(Ok(PaymentError::EscrowTimelockActive)));

    // Admin refunds in bulk immediately
    let refunded_count = client.batch_refund_escrows(&admin, &ids);
    assert_eq!(refunded_count, 2);

    // Balances returned to sender
    assert_eq!(token_client.balance(&sender), 500_0000000);
    assert_eq!(
        client.get_escrow(&r1.escrow_id).status,
        EscrowStatus::Refunded
    );
    assert_eq!(
        client.get_escrow(&r2.escrow_id).status,
        EscrowStatus::Refunded
    );

    // Subsequent batch refund fails as already settled
    let reserialized = client.try_batch_refund_escrows(&admin, &ids);
    assert_eq!(reserialized, Err(Ok(PaymentError::EscrowAlreadySettled)));
}

#[test]
fn test_get_payment_configuration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    // Initial configuration check
    let cfg = client.get_configuration();
    assert_eq!(cfg.admin, admin);
    assert_eq!(cfg.pending_admin, None);
    assert_eq!(cfg.escrow_period, 3600);
    assert_eq!(cfg.spend_cap, 50_0000000);
    assert_eq!(cfg.min_payment_amount, 1);
    assert_eq!(cfg.is_paused, false);
    assert_eq!(cfg.guardian, None);
    assert_eq!(cfg.require_allowlist, false);
    assert_eq!(cfg.total_volume, 0);
    assert_eq!(cfg.payment_sequence, 0);

    // Set guardian and min payment amount
    let guardian = Address::generate(&env);
    client.set_guardian(&guardian);
    client.set_min_payment_amount(&10_0000000);

    let updated_cfg = client.get_configuration();
    assert_eq!(updated_cfg.guardian, Some(guardian));
    assert_eq!(updated_cfg.min_payment_amount, 10_0000000);

    // Trigger pause circuit breaker via set_paused
    client.set_paused(&true);
    assert_eq!(client.get_configuration().is_paused, true);
}

#[test]
fn test_batch_dispute_escrows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let rec1 = Address::generate(&env);
    let rec2 = Address::generate(&env);
    let stranger = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Create 2 escrows
    let r1 = client.pay(&sender, &rec1, &token_client.address, &100_0000000);
    let r2 = client.pay(&sender, &rec2, &token_client.address, &150_0000000);

    let mut ids = Vec::new(&env);
    ids.push_back(r1.escrow_id);
    ids.push_back(r2.escrow_id);

    // Stranger tries to batch dispute -> InvalidDisputeParty
    let stranger_res = client.try_batch_dispute_escrows(&stranger, &ids);
    assert_eq!(stranger_res, Err(Ok(PaymentError::InvalidDisputeParty)));

    // Sender disputes both escrows in batch
    let disputed_count = client.batch_dispute_escrows(&sender, &ids);
    assert_eq!(disputed_count, 2);

    assert_eq!(
        client.get_escrow(&r1.escrow_id).status,
        EscrowStatus::Disputed
    );
    assert_eq!(
        client.get_escrow(&r2.escrow_id).status,
        EscrowStatus::Disputed
    );

    // Subsequent batch dispute fails because they are no longer Pending
    let reserialized = client.try_batch_dispute_escrows(&sender, &ids);
    assert_eq!(reserialized, Err(Ok(PaymentError::EscrowAlreadySettled)));
}

#[test]
fn test_batch_arbitrate_escrows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let rec1 = Address::generate(&env);
    let rec2 = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Create 2 escrows
    let r1 = client.pay(&sender, &rec1, &token_client.address, &100_0000000);
    let r2 = client.pay(&sender, &rec2, &token_client.address, &150_0000000);

    let mut ids = Vec::new(&env);
    ids.push_back(r1.escrow_id);
    ids.push_back(r2.escrow_id);

    // Dispute both escrows
    client.batch_dispute_escrows(&sender, &ids);
    assert_eq!(
        client.get_escrow(&r1.escrow_id).status,
        EscrowStatus::Disputed
    );
    assert_eq!(
        client.get_escrow(&r2.escrow_id).status,
        EscrowStatus::Disputed
    );

    // Since mock_all_auths is on, let's verify admin call succeeds
    let arbitrated_count = client.batch_arbitrate_escrows(&ids, &true);
    assert_eq!(arbitrated_count, 2);

    // Both settled to Released and tokens received by recipients
    assert_eq!(
        client.get_escrow(&r1.escrow_id).status,
        EscrowStatus::Released
    );
    assert_eq!(
        client.get_escrow(&r2.escrow_id).status,
        EscrowStatus::Released
    );
    assert_eq!(token_client.balance(&rec1), 100_0000000);
    assert_eq!(token_client.balance(&rec2), 150_0000000);

    // Subsequent arbitration fails
    let reserialized = client.try_batch_arbitrate_escrows(&ids, &false);
    assert_eq!(reserialized, Err(Ok(PaymentError::EscrowAlreadySettled)));
}

#[test]
fn test_active_escrows_paged() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let rec1 = Address::generate(&env);
    let rec2 = Address::generate(&env);
    let rec3 = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &1000_0000000);

    // Create 3 escrows
    let r1 = client.pay(&sender, &rec1, &token_client.address, &100_0000000);
    let r2 = client.pay(&sender, &rec2, &token_client.address, &200_0000000);
    let r3 = client.pay(&sender, &rec3, &token_client.address, &300_0000000);

    // Dispute r2
    client.dispute_escrow(&sender, &r2.escrow_id);

    // Release r3 (settled)
    client.release_escrow(&r3.escrow_id);

    // Total active = 2 (r1=Pending, r2=Disputed). r3=Released is omitted.
    let all_active = client.get_active_escrows_paged(&0, &10);
    assert_eq!(all_active.len(), 2);
    assert_eq!(all_active.get(0).unwrap().id, r1.escrow_id);
    assert_eq!(all_active.get(1).unwrap().id, r2.escrow_id);

    // Page 1: offset 0, limit 1
    let p1 = client.get_active_escrows_paged(&0, &1);
    assert_eq!(p1.len(), 1);
    assert_eq!(p1.get(0).unwrap().id, r1.escrow_id);

    // Page 2: offset 1, limit 1
    let p2 = client.get_active_escrows_paged(&1, &1);
    assert_eq!(p2.len(), 1);
    assert_eq!(p2.get(0).unwrap().id, r2.escrow_id);

    // Out of bounds page: offset 5, limit 10
    let empty_page = client.get_active_escrows_paged(&5, &10);
    assert_eq!(empty_page.len(), 0);

    // Zero limit
    let zero_page = client.get_active_escrows_paged(&0, &0);
    assert_eq!(zero_page.len(), 0);
}

#[test]
fn test_emergency_pause_with_duration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let guardian = Address::generate(&env);
    client.set_guardian(&guardian);

    let stranger = Address::generate(&env);

    // Initial pause status is clean
    let initial_status = client.get_pause_status();
    assert_eq!(initial_status.is_paused, false);
    assert_eq!(initial_status.paused_until, 0);
    assert_eq!(initial_status.paused_by, None);

    // Stranger cannot trigger emergency pause
    let unauthorized_res = client.try_emergency_pause_with_duration(&stranger, &300);
    assert_eq!(unauthorized_res, Err(Ok(PaymentError::Unauthorized)));

    // Guardian triggers 300-second emergency pause
    let start_ts = env.ledger().timestamp();
    client.emergency_pause_with_duration(&guardian, &300);

    let status = client.get_pause_status();
    assert_eq!(status.is_paused, true);
    assert_eq!(status.paused_until, start_ts + 300);
    assert_eq!(status.paused_by, Some(guardian.clone()));
    assert_eq!(client.is_paused(), true);

    // While in the window, payments are blocked
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    let pay_attempt = client.try_pay(&sender, &recipient, &token_client.address, &10_0000000);
    assert_eq!(pay_attempt, Err(Ok(PaymentError::ContractPaused)));

    // Advance timestamp past expiration window (start_ts + 301)
    env.ledger().set_timestamp(start_ts + 301);

    // Now auto-unpaused!
    assert_eq!(client.is_paused(), false);
    assert_eq!(client.get_pause_status().is_paused, false);

    // Payment now succeeds
    let pay_success = client.pay(&sender, &recipient, &token_client.address, &10_0000000);
    assert_eq!(pay_success.status, PaymentStatus::Approved);

    // Admin can emergency pause and manually unpause early
    client.emergency_pause_with_duration(&admin, &5000);
    assert_eq!(client.is_paused(), true);
    client.set_paused(&false);
    assert_eq!(client.is_paused(), false);
    let cleared_status = client.get_pause_status();
    assert_eq!(cleared_status.is_paused, false);
    assert_eq!(cleared_status.paused_until, 0);
    assert_eq!(cleared_status.paused_by, None);
}

#[test]
fn test_get_payment_sequence_details() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &100_0000000);

    let d0 = client.get_payment_sequence_details();
    assert_eq!(d0.current_sequence, 0);
    assert_eq!(d0.total_volume, 0);
    assert_eq!(d0.min_payment_amount, 1);
    assert_eq!(d0.is_paused, false);
    assert_eq!(d0.active_spend_cap, 100_0000000);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    // Payment 1: approved
    let r1 = client.pay(&sender, &recipient, &token_client.address, &20_0000000);
    assert_eq!(r1.status, PaymentStatus::Approved);

    // Payment 2: routed to escrow (exceeds 100 cap)
    let r2 = client.pay(&sender, &recipient, &token_client.address, &150_0000000);
    assert_eq!(r2.status, PaymentStatus::Escrowed);

    let d1 = client.get_payment_sequence_details();
    assert_eq!(d1.current_sequence, 2);
    assert_eq!(d1.total_volume, 170_0000000);
    assert_eq!(d1.is_paused, false);
    assert_eq!(d1.active_spend_cap, 100_0000000);
}

#[test]
fn test_batch_compliance_queries() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &100_0000000);

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);

    // Add a1 and a2 to denylist
    let mut deny_batch = Vec::new(&env);
    deny_batch.push_back(a1.clone());
    deny_batch.push_back(a2.clone());
    client.add_to_denylist_batch(&deny_batch);

    // Add a3 to allowlist
    client.add_to_allowlist(&a3);

    let check_batch = Vec::from_array(&env, [a1.clone(), a2.clone(), a3.clone()]);

    let deny_res = client.batch_is_denylisted(&check_batch);
    assert_eq!(deny_res.get(0).unwrap(), true);
    assert_eq!(deny_res.get(1).unwrap(), true);
    assert_eq!(deny_res.get(2).unwrap(), false);

    let allow_res = client.batch_is_allowlisted(&check_batch);
    assert_eq!(allow_res.get(0).unwrap(), false);
    assert_eq!(allow_res.get(1).unwrap(), false);
    assert_eq!(allow_res.get(2).unwrap(), true);
}

#[test]
fn test_get_escrow_ids_by_status_paged() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &50_0000000);

    let sender = Address::generate(&env);
    let r1_rec = Address::generate(&env);
    let r2_rec = Address::generate(&env);
    let r3_rec = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &1000_0000000);

    let e1 = client.pay(&sender, &r1_rec, &token_client.address, &100_0000000);
    let e2 = client.pay(&sender, &r2_rec, &token_client.address, &120_0000000);
    let e3 = client.pay(&sender, &r3_rec, &token_client.address, &140_0000000);

    // e2 -> dispute
    client.dispute_escrow(&sender, &e2.escrow_id);
    // e3 -> release
    client.release_escrow(&e3.escrow_id);

    // Status 1: Pending (only e1)
    let pending_ids = client.get_escrow_ids_by_status_paged(&1, &0, &10);
    assert_eq!(pending_ids.len(), 1);
    assert_eq!(pending_ids.get(0).unwrap(), e1.escrow_id);

    // Status 4: Disputed (only e2)
    let disputed_ids = client.get_escrow_ids_by_status_paged(&4, &0, &10);
    assert_eq!(disputed_ids.len(), 1);
    assert_eq!(disputed_ids.get(0).unwrap(), e2.escrow_id);

    // Status 2: Released (only e3)
    let released_ids = client.get_escrow_ids_by_status_paged(&2, &0, &10);
    assert_eq!(released_ids.len(), 1);
    assert_eq!(released_ids.get(0).unwrap(), e3.escrow_id);

    // Status 0: All (paged)
    let all_p1 = client.get_escrow_ids_by_status_paged(&0, &0, &2);
    assert_eq!(all_p1.len(), 2);
    assert_eq!(all_p1.get(0).unwrap(), e1.escrow_id);
    assert_eq!(all_p1.get(1).unwrap(), e2.escrow_id);

    let all_p2 = client.get_escrow_ids_by_status_paged(&0, &2, &2);
    assert_eq!(all_p2.len(), 1);
    assert_eq!(all_p2.get(0).unwrap(), e3.escrow_id);
}

#[test]
fn test_get_gateway_telemetry_snapshot() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SafeguardPayments, ());
    let client = SafeguardPaymentsClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &3600, &200_0000000);

    let guardian = Address::generate(&env);
    client.set_guardian(&guardian);

    let snap0 = client.get_gateway_telemetry_snapshot();
    assert_eq!(snap0.admin, admin);
    assert_eq!(snap0.guardian, Some(guardian));
    assert_eq!(snap0.policy_contract, None);
    assert_eq!(snap0.active_spend_cap, 200_0000000);
    assert_eq!(snap0.current_sequence, 0);
    assert_eq!(snap0.total_escrows, 0);
    assert_eq!(snap0.total_volume, 0);
    assert_eq!(snap0.is_paused, false);

    // Process a payment
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_client = create_token_contract(&env, &token_admin);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_admin_client.mint(&sender, &500_0000000);

    client.pay(&sender, &recipient, &token_client.address, &50_0000000);

    let snap1 = client.get_gateway_telemetry_snapshot();
    assert_eq!(snap1.current_sequence, 1);
    assert_eq!(snap1.total_volume, 50_0000000);
}
