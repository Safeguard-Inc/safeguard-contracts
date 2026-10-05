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
    client.pay(&sender, &blocked_recipient, &token_client.address, &50_0000000);
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
