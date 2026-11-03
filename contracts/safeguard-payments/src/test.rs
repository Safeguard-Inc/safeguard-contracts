#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

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
