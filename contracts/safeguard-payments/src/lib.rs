#![no_std]
#![allow(deprecated)]
#![allow(clippy::all)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env,
    IntoVal, Symbol, Vec,
};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ContractPaused = 4,
    InvalidAmount = 5,
    SpendCapExceeded = 6,
    PolicyDenied = 7,
    EscrowNotFound = 8,
    EscrowAlreadySettled = 9,
    EscrowTimelockActive = 10,
    RecipientDenylisted = 11,
    SenderDenylisted = 12,
    PendingAdminNotFound = 13,
    InvalidEscrowPeriod = 14,
    EscrowUnderDispute = 15,
    InvalidDisputeParty = 16,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentStatus {
    Approved = 1,
    Escrowed = 2,
    Blocked = 3,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Pending = 1,
    Released = 2,
    Refunded = 3,
    Disputed = 4,
    Arbitrated = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentReceipt {
    pub sender: Address,
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    pub status: PaymentStatus,
    pub reason_code: u32,
    pub escrow_id: u64,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRecord {
    pub id: u64,
    pub sender: Address,
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    pub created_at: u64,
    pub release_after: u64,
    pub status: EscrowStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowMetrics {
    pub total_escrows: u64,
    pub pending_count: u64,
    pub disputed_count: u64,
    pub settled_count: u64,
    pub total_locked_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentCheckParam {
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentSimulationVerdict {
    pub recipient: Address,
    pub amount: i128,
    pub status: PaymentStatus,
    pub reason_code: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentGatewayConfig {
    pub admin: Address,
    pub pending_admin: Option<Address>,
    pub policy_contract: Option<Address>,
    pub guardian: Option<Address>,
    pub escrow_period: u64,
    pub spend_cap: i128,
    pub min_payment_amount: i128,
    pub is_paused: bool,
    pub require_allowlist: bool,
    pub total_volume: i128,
    pub payment_sequence: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseStatus {
    pub is_paused: bool,
    pub paused_until: u64,
    pub paused_by: Option<Address>,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    PendingAdmin,
    PolicyContract,
    EscrowPeriod,
    EscrowCounter,
    Escrow(u64),
    SpendCap,
    IsPaused,
    PauseUntil,
    PausedBy,
    Guardian,
    Denylist(Address),
    Allowlist(Address),
    RequireAllowlist,
    TotalVolume,
    PaymentSequence,
    MinPaymentAmount,
    TokenSpendCap(Address),
}

#[contract]
pub struct SafeguardPayments;

#[contractimpl]
impl SafeguardPayments {
    /// Initialize the Safeguard payment gateway contract.
    pub fn initialize(
        env: Env,
        admin: Address,
        escrow_period: u64,
        spend_cap: i128,
    ) -> Result<(), PaymentError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PaymentError::AlreadyInitialized);
        }

        admin.require_auth();

        if escrow_period == 0 {
            return Err(PaymentError::InvalidEscrowPeriod);
        }
        if spend_cap < 0 {
            return Err(PaymentError::InvalidAmount);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::EscrowPeriod, &escrow_period);
        env.storage().instance().set(&DataKey::SpendCap, &spend_cap);
        env.storage().instance().set(&DataKey::IsPaused, &false);
        env.storage().instance().set(&DataKey::EscrowCounter, &0u64);
        env.storage().instance().set(&DataKey::TotalVolume, &0i128);
        env.storage().instance().set(&DataKey::PaymentSequence, &0u64);
        env.storage().instance().set(&DataKey::MinPaymentAmount, &1i128);
        env.storage()
            .instance()
            .set(&DataKey::RequireAllowlist, &false);

        env.events().publish((symbol_short!("init"),), admin);

        Ok(())
    }

    /// Update admin address.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events().publish((symbol_short!("set_adm"),), new_admin);
        Ok(())
    }

    /// Propose a new admin address (Step 1 of two-step admin handover).
    pub fn propose_admin(env: Env, new_admin: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        env.events()
            .publish((symbol_short!("prop_adm"),), new_admin);
        Ok(())
    }

    /// Claim admin role (Step 2 of two-step admin handover).
    /// Requires authorization from the proposed pending admin.
    pub fn claim_admin(env: Env) -> Result<(), PaymentError> {
        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .ok_or(PaymentError::PendingAdminNotFound)?;

        pending.require_auth();

        env.storage().instance().set(&DataKey::Admin, &pending);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events().publish((symbol_short!("set_adm"),), pending);
        Ok(())
    }

    /// Retrieve currently proposed pending admin, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PendingAdmin)
    }

    /// Cancel a pending admin handover proposal. Current admin only.
    pub fn cancel_admin_transfer(env: Env) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        if env.storage().instance().has(&DataKey::PendingAdmin) {
            env.storage().instance().remove(&DataKey::PendingAdmin);
            env.events().publish((symbol_short!("cncl_adm"),), ());
            Ok(())
        } else {
            Err(PaymentError::PendingAdminNotFound)
        }
    }

    /// Assign an emergency risk guardian address. Admin only.
    pub fn set_guardian(env: Env, guardian: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Guardian, &guardian);
        env.events().publish((symbol_short!("set_guard"),), guardian);
        Ok(())
    }

    /// Retrieve the configured emergency guardian address.
    pub fn get_guardian(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Guardian)
    }

    /// Emergency circuit breaker: trigger immediate contract freeze.
    /// Can be called by the configured Guardian OR Admin.
    pub fn pause_by_guardian(env: Env, caller: Address) -> Result<(), PaymentError> {
        caller.require_auth();

        let is_admin = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .map(|a| a == caller)
            .unwrap_or(false);

        let is_guardian = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Guardian)
            .map(|g| g == caller)
            .unwrap_or(false);

        if !is_admin && !is_guardian {
            return Err(PaymentError::Unauthorized);
        }

        env.storage().instance().set(&DataKey::IsPaused, &true);
        env.events().publish((symbol_short!("guard_pse"),), caller);
        Ok(())
    }

    /// Pause or unpause payment operations.
    pub fn set_paused(env: Env, paused: bool) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::IsPaused, &paused);
        if !paused {
            env.storage().instance().remove(&DataKey::PauseUntil);
            env.storage().instance().remove(&DataKey::PausedBy);
        }
        env.events().publish((symbol_short!("paused"),), paused);
        Ok(())
    }

    /// Engage emergency circuit breaker pause with an automatic expiration timelock duration in seconds.
    /// Can be called by Admin or configured Guardian.
    pub fn emergency_pause_with_duration(
        env: Env,
        caller: Address,
        duration_seconds: u64,
    ) -> Result<(), PaymentError> {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PaymentError::NotInitialized)?;
        let is_admin = caller == admin;

        let is_guardian = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Guardian)
            .map(|g| g == caller)
            .unwrap_or(false);

        if !is_admin && !is_guardian {
            return Err(PaymentError::Unauthorized);
        }

        let until = env.ledger().timestamp().saturating_add(duration_seconds);
        env.storage().instance().set(&DataKey::IsPaused, &true);
        env.storage().instance().set(&DataKey::PauseUntil, &until);
        env.storage().instance().set(&DataKey::PausedBy, &caller);

        env.events().publish(
            (symbol_short!("em_pause"),),
            (caller, until),
        );

        Ok(())
    }

    /// Retrieve full pause status including whether pause is active, expiry timestamp, and initiator.
    pub fn get_pause_status(env: Env) -> PauseStatus {
        let active = Self::is_paused(env.clone());
        let paused_until: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PauseUntil)
            .unwrap_or(0);
        let paused_by: Option<Address> = env.storage().instance().get(&DataKey::PausedBy);

        PauseStatus {
            is_paused: active,
            paused_until,
            paused_by,
        }
    }

    /// Update minimum payment amount threshold (prevents dusting and sub-minimum spam).
    pub fn set_min_payment_amount(env: Env, min_amount: i128) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        if min_amount < 0 {
            return Err(PaymentError::InvalidAmount);
        }
        env.storage()
            .instance()
            .set(&DataKey::MinPaymentAmount, &min_amount);
        env.events()
            .publish((symbol_short!("set_min"),), min_amount);
        Ok(())
    }

    /// Retrieve current minimum payment threshold.
    pub fn get_min_payment_amount(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::MinPaymentAmount)
            .unwrap_or(1)
    }

    /// Set a token-specific spend cap. When set, payments in this token use this cap
    /// instead of the global default spend cap. Admin only.
    pub fn set_token_spend_cap(
        env: Env,
        token: Address,
        cap: i128,
    ) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        if cap < 0 {
            return Err(PaymentError::InvalidAmount);
        }
        env.storage()
            .persistent()
            .set(&DataKey::TokenSpendCap(token.clone()), &cap);
        env.events().publish((symbol_short!("t_cap_set"),), (token, cap));
        Ok(())
    }

    /// Retrieve the effective spend cap for a token.
    /// Returns the per-token spend cap if configured; otherwise falls back to global default.
    pub fn get_effective_spend_cap(env: Env, token: Address) -> i128 {
        if let Some(token_cap) = env
            .storage()
            .persistent()
            .get::<_, i128>(&DataKey::TokenSpendCap(token))
        {
            token_cap
        } else {
            env.storage()
                .instance()
                .get(&DataKey::SpendCap)
                .unwrap_or(i128::MAX)
        }
    }

    /// Retrieve specifically configured token spend cap, without falling back.
    pub fn get_token_spend_cap(env: Env, token: Address) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&DataKey::TokenSpendCap(token))
    }

    /// Remove a per-token spend cap override, restoring the global default cap.
    pub fn remove_token_spend_cap(env: Env, token: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::TokenSpendCap(token.clone()));
        env.events().publish((symbol_short!("t_cap_rem"),), token);
        Ok(())
    }

    /// Update spend cap (amounts exceeding this cap are routed to Escrow for review).
    pub fn set_spend_cap(env: Env, new_cap: i128) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        if new_cap < 0 {
            return Err(PaymentError::InvalidAmount);
        }
        env.storage().instance().set(&DataKey::SpendCap, &new_cap);
        env.events().publish((symbol_short!("set_cap"),), new_cap);
        Ok(())
    }

    /// Add an address to the denylist.
    pub fn add_to_denylist(env: Env, address: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::Denylist(address.clone()), &true);
        env.events().publish((symbol_short!("deny_add"),), address);
        Ok(())
    }

    /// Remove an address from the denylist.
    pub fn remove_from_denylist(env: Env, address: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::Denylist(address.clone()));
        env.events().publish((symbol_short!("deny_rem"),), address);
        Ok(())
    }

    /// Check if an address is denylisted.
    pub fn is_denylisted(env: Env, address: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Denylist(address))
            .unwrap_or(false)
    }

    /// Configure the external SafeguardPolicy contract address.
    pub fn set_policy_contract(env: Env, policy_contract: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::PolicyContract, &policy_contract);
        env.events()
            .publish((symbol_short!("set_pol"),), policy_contract);
        Ok(())
    }

    /// Retrieve the configured SafeguardPolicy contract address.
    pub fn get_policy_contract(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PolicyContract)
    }

    /// Remove the configured SafeguardPolicy contract address.
    pub fn remove_policy_contract(env: Env) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage().instance().remove(&DataKey::PolicyContract);
        env.events().publish((symbol_short!("rem_pol"),), ());
        Ok(())
    }

    /// Extend the TTL of the contract's instance storage and code.
    /// Can be called to ensure the contract remains live and accessible on ledger.
    pub fn extend_instance_ttl(env: Env, threshold: u32, extend_to: u32) {
        env.storage().instance().extend_ttl(threshold, extend_to);
    }

    /// Extend the TTL of an active escrow record in persistent storage.
    /// Ensures escrow state cannot be archived before timelock expiry or settlement.
    pub fn extend_escrow_ttl(
        env: Env,
        escrow_id: u64,
        threshold: u32,
        extend_to: u32,
    ) -> Result<(), PaymentError> {
        let key = DataKey::Escrow(escrow_id);
        if !env.storage().persistent().has(&key) {
            return Err(PaymentError::EscrowNotFound);
        }
        env.storage().persistent().extend_ttl(&key, threshold, extend_to);
        Ok(())
    }

    /// Execute a policy-guarded payment in a SEP-41 SAC token.
    ///
    /// Outcomes:
    /// - Normal amount within cap + compliant parties -> Direct Transfer (`Approved`)
    /// - High value (> spend cap) -> Diverted to Escrow (`Escrowed`)
    /// - Denylisted sender/recipient or zero amount -> Revert / Denied (`Blocked`)
    pub fn pay(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
    ) -> Result<PaymentReceipt, PaymentError> {
        sender.require_auth();

        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(PaymentError::NotInitialized);
        }

        if Self::is_paused(env.clone()) {
            return Err(PaymentError::ContractPaused);
        }

        let min_amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::MinPaymentAmount)
            .unwrap_or(1);
        if amount < min_amount || amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        // 1. Check denylist for sender
        if Self::is_denylisted(env.clone(), sender.clone()) {
            env.events().publish(
                (symbol_short!("pay_block"), symbol_short!("snd_deny")),
                (sender.clone(), recipient.clone(), amount),
            );
            return Err(PaymentError::SenderDenylisted);
        }

        // 2. Check denylist for recipient
        if Self::is_denylisted(env.clone(), recipient.clone()) {
            env.events().publish(
                (symbol_short!("pay_block"), symbol_short!("rcp_deny")),
                (sender.clone(), recipient.clone(), amount),
            );
            return Err(PaymentError::RecipientDenylisted);
        }

        // 3. Screen sender and recipient against external PolicyContract if wired
        if let Some(policy_contract) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::PolicyContract)
        {
            let auth_fn = Symbol::new(&env, "is_authorized");

            // Screen sender
            let sender_args = (sender.clone(), token.clone()).into_val(&env);
            let sender_allowed = match env.try_invoke_contract::<bool, soroban_sdk::Error>(
                &policy_contract,
                &auth_fn,
                sender_args,
            ) {
                Ok(Ok(allowed)) => allowed,
                _ => false, // fail closed if call fails, reverts, or returns false
            };

            if !sender_allowed {
                env.events().publish(
                    (symbol_short!("pay_block"), symbol_short!("pol_snd")),
                    (sender.clone(), recipient.clone(), amount),
                );
                return Err(PaymentError::PolicyDenied);
            }

            // Screen recipient
            let recipient_args = (recipient.clone(), token.clone()).into_val(&env);
            let recipient_allowed = match env.try_invoke_contract::<bool, soroban_sdk::Error>(
                &policy_contract,
                &auth_fn,
                recipient_args,
            ) {
                Ok(Ok(allowed)) => allowed,
                _ => false, // fail closed if call fails, reverts, or returns false
            };

            if !recipient_allowed {
                env.events().publish(
                    (symbol_short!("pay_block"), symbol_short!("pol_rcp")),
                    (sender.clone(), recipient.clone(), amount),
                );
                return Err(PaymentError::PolicyDenied);
            }
        }

        let spend_cap = Self::get_effective_spend_cap(env.clone(), token.clone());
        let timestamp = env.ledger().timestamp();

        // Update cumulative payment volume
        let current_volume: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalVolume)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalVolume, &(current_volume + amount));

        // Increment payment sequence number
        let current_seq: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PaymentSequence)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::PaymentSequence, &(current_seq + 1));

        // 3. Routing: direct vs escrow
        if amount > spend_cap {
            let token_client = token::Client::new(&env, &token);
            let contract_addr = env.current_contract_address();
            token_client.transfer(&sender, &contract_addr, &amount);

            let mut counter: u64 = env
                .storage()
                .instance()
                .get(&DataKey::EscrowCounter)
                .unwrap_or(0);
            counter += 1;
            env.storage()
                .instance()
                .set(&DataKey::EscrowCounter, &counter);

            let escrow_period: u64 = env
                .storage()
                .instance()
                .get(&DataKey::EscrowPeriod)
                .unwrap_or(86400);
            let release_after = timestamp + escrow_period;

            let record = EscrowRecord {
                id: counter,
                sender: sender.clone(),
                recipient: recipient.clone(),
                token: token.clone(),
                amount,
                created_at: timestamp,
                release_after,
                status: EscrowStatus::Pending,
            };

            env.storage()
                .persistent()
                .set(&DataKey::Escrow(counter), &record);

            env.events().publish(
                (symbol_short!("pay_esc"),),
                (counter, sender.clone(), recipient.clone(), amount),
            );

            Ok(PaymentReceipt {
                sender,
                recipient,
                token,
                amount,
                status: PaymentStatus::Escrowed,
                reason_code: 6, // spend cap exceeded -> flagged
                escrow_id: counter,
                timestamp,
            })
        } else {
            // Approved: direct SAC transfer
            let token_client = token::Client::new(&env, &token);
            token_client.transfer(&sender, &recipient, &amount);

            env.events().publish(
                (symbol_short!("pay_app"),),
                (sender.clone(), recipient.clone(), amount),
            );

            Ok(PaymentReceipt {
                sender,
                recipient,
                token,
                amount,
                status: PaymentStatus::Approved,
                reason_code: 0,
                escrow_id: 0,
                timestamp,
            })
        }
    }

    /// Raise a dispute on an active escrow. Can only be invoked by the sender or the recipient.
    pub fn dispute_escrow(env: Env, caller: Address, escrow_id: u64) -> Result<(), PaymentError> {
        caller.require_auth();

        let mut record: EscrowRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)?;

        if record.status != EscrowStatus::Pending {
            return Err(PaymentError::EscrowAlreadySettled);
        }

        if caller != record.sender && caller != record.recipient {
            return Err(PaymentError::InvalidDisputeParty);
        }

        record.status = EscrowStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &record);

        env.events().publish(
            (soroban_sdk::symbol_short!("esc_disp"), escrow_id),
            (caller, record.amount),
        );

        Ok(())
    }

    /// Raise a dispute across multiple active escrows in bulk.
    /// Can only be invoked by a party (sender or recipient) of each escrow.
    pub fn batch_dispute_escrows(
        env: Env,
        caller: Address,
        escrow_ids: Vec<u64>,
    ) -> Result<u32, PaymentError> {
        caller.require_auth();

        let mut count = 0u32;
        for id in escrow_ids.iter() {
            let mut record: EscrowRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Escrow(id))
                .ok_or(PaymentError::EscrowNotFound)?;

            if record.status != EscrowStatus::Pending {
                return Err(PaymentError::EscrowAlreadySettled);
            }

            if caller != record.sender && caller != record.recipient {
                return Err(PaymentError::InvalidDisputeParty);
            }

            record.status = EscrowStatus::Disputed;
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(id), &record);

            env.events().publish(
                (soroban_sdk::symbol_short!("esc_disp"), id),
                (caller.clone(), record.amount),
            );

            count += 1;
        }

        Ok(count)
    }

    /// Admin arbitrates a disputed escrow. Admin can route resolution either to the recipient or refund to the sender.
    pub fn arbitrate_escrow(env: Env, escrow_id: u64, release_to_recipient: bool) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;

        let mut record: EscrowRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)?;

        if record.status != EscrowStatus::Disputed {
            return Err(PaymentError::EscrowAlreadySettled);
        }

        let token_client = token::Client::new(&env, &record.token);

        if release_to_recipient {
            record.status = EscrowStatus::Released;
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(escrow_id), &record);

            token_client.transfer(
                &env.current_contract_address(),
                &record.recipient,
                &record.amount,
            );

            env.events().publish(
                (symbol_short!("esc_arb"), escrow_id),
                (record.recipient.clone(), record.amount, true),
            );
        } else {
            record.status = EscrowStatus::Refunded;
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(escrow_id), &record);

            token_client.transfer(
                &env.current_contract_address(),
                &record.sender,
                &record.amount,
            );

            env.events().publish(
                (symbol_short!("esc_arb"), escrow_id),
                (record.sender.clone(), record.amount, false),
            );
        }

        Ok(())
    }

    /// Admin arbitrates multiple disputed escrows in bulk.
    /// If `release_to_recipient` is true, funds are paid to recipients; if false, refunded to senders.
    pub fn batch_arbitrate_escrows(
        env: Env,
        escrow_ids: Vec<u64>,
        release_to_recipient: bool,
    ) -> Result<u32, PaymentError> {
        Self::require_admin(&env)?;

        let mut count = 0u32;
        for id in escrow_ids.iter() {
            let mut record: EscrowRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Escrow(id))
                .ok_or(PaymentError::EscrowNotFound)?;

            if record.status != EscrowStatus::Disputed {
                return Err(PaymentError::EscrowAlreadySettled);
            }

            let token_client = token::Client::new(&env, &record.token);

            if release_to_recipient {
                record.status = EscrowStatus::Released;
                env.storage()
                    .persistent()
                    .set(&DataKey::Escrow(id), &record);

                token_client.transfer(
                    &env.current_contract_address(),
                    &record.recipient,
                    &record.amount,
                );

                env.events().publish(
                    (symbol_short!("esc_arb"), id),
                    (record.recipient.clone(), record.amount, true),
                );
            } else {
                record.status = EscrowStatus::Refunded;
                env.storage()
                    .persistent()
                    .set(&DataKey::Escrow(id), &record);

                token_client.transfer(
                    &env.current_contract_address(),
                    &record.sender,
                    &record.amount,
                );

                env.events().publish(
                    (symbol_short!("esc_arb"), id),
                    (record.sender.clone(), record.amount, false),
                );
            }

            count += 1;
        }

        Ok(count)
    }

    /// Admin releases an escrowed payment to the recipient.
    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;

        let mut record: EscrowRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)?;

        if record.status != EscrowStatus::Pending {
            return Err(PaymentError::EscrowAlreadySettled);
        }

        // Effects before interactions: mark settled, then transfer.
        record.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &record);

        let token_client = token::Client::new(&env, &record.token);
        token_client.transfer(
            &env.current_contract_address(),
            &record.recipient,
            &record.amount,
        );

        env.events().publish(
            (symbol_short!("esc_rel"),),
            (escrow_id, record.recipient.clone(), record.amount),
        );

        Ok(())
    }

    /// Admin releases multiple active escrows to their respective recipients in bulk.
    pub fn batch_release_escrows(
        env: Env,
        escrow_ids: Vec<u64>,
    ) -> Result<u32, PaymentError> {
        Self::require_admin(&env)?;

        let mut count = 0u32;
        for id in escrow_ids.iter() {
            let mut record: EscrowRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Escrow(id))
                .ok_or(PaymentError::EscrowNotFound)?;

            if record.status != EscrowStatus::Pending {
                return Err(PaymentError::EscrowAlreadySettled);
            }

            record.status = EscrowStatus::Released;
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(id), &record);

            let token_client = token::Client::new(&env, &record.token);
            token_client.transfer(
                &env.current_contract_address(),
                &record.recipient,
                &record.amount,
            );

            env.events().publish(
                (symbol_short!("esc_rel"),),
                (id, record.recipient.clone(), record.amount),
            );
            count += 1;
        }

        Ok(count)
    }

    /// Admin partially releases an escrowed payment to the recipient.
    /// If partial_amount == record.amount, marks the escrow as Released.
    /// Otherwise, decrements the escrow record amount and transfers partial_amount to recipient,
    /// keeping the escrow record active in Pending status.
    pub fn release_escrow_partial(
        env: Env,
        escrow_id: u64,
        partial_amount: i128,
    ) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;

        if partial_amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let mut record: EscrowRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)?;

        if record.status != EscrowStatus::Pending {
            return Err(PaymentError::EscrowAlreadySettled);
        }

        if partial_amount > record.amount {
            return Err(PaymentError::InvalidAmount);
        }

        if partial_amount == record.amount {
            record.status = EscrowStatus::Released;
            record.amount = 0;
        } else {
            record.amount -= partial_amount;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &record);

        let token_client = token::Client::new(&env, &record.token);
        token_client.transfer(
            &env.current_contract_address(),
            &record.recipient,
            &partial_amount,
        );

        env.events().publish(
            (symbol_short!("esc_part"),),
            (escrow_id, record.recipient.clone(), partial_amount, record.amount),
        );

        Ok(())
    }

    /// Admin extends the dispute or release timelock (`release_after`) on an active escrow (Pending or Disputed).
    /// Allows investigation or extended arbitration window without funds prematurely unlocking.
    pub fn extend_dispute_period(
        env: Env,
        escrow_id: u64,
        extension_seconds: u64,
    ) -> Result<u64, PaymentError> {
        Self::require_admin(&env)?;

        if extension_seconds == 0 {
            return Err(PaymentError::InvalidEscrowPeriod);
        }

        let mut record: EscrowRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)?;

        if record.status != EscrowStatus::Pending && record.status != EscrowStatus::Disputed {
            return Err(PaymentError::EscrowAlreadySettled);
        }

        record.release_after += extension_seconds;
        let new_release_after = record.release_after;

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &record);

        env.events().publish(
            (symbol_short!("esc_ext"), escrow_id),
            (new_release_after, extension_seconds),
        );

        Ok(new_release_after)
    }

    /// Admin extends the dispute or release timelock (`release_after`) across multiple active escrows in bulk.
    pub fn batch_extend_escrows(
        env: Env,
        escrow_ids: Vec<u64>,
        extension_seconds: u64,
    ) -> Result<u32, PaymentError> {
        Self::require_admin(&env)?;

        if extension_seconds == 0 {
            return Err(PaymentError::InvalidEscrowPeriod);
        }

        let mut count = 0u32;
        for id in escrow_ids.iter() {
            let mut record: EscrowRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Escrow(id))
                .ok_or(PaymentError::EscrowNotFound)?;

            if record.status != EscrowStatus::Pending && record.status != EscrowStatus::Disputed {
                return Err(PaymentError::EscrowAlreadySettled);
            }

            record.release_after += extension_seconds;
            let new_release_after = record.release_after;

            env.storage()
                .persistent()
                .set(&DataKey::Escrow(id), &record);

            env.events().publish(
                (symbol_short!("esc_ext"), id),
                (new_release_after, extension_seconds),
            );
            count += 1;
        }

        Ok(count)
    }

    /// Refund an escrowed payment back to the sender.
    /// Can be initiated by admin immediately, or by the sender after `release_after` timelock.
    pub fn refund_escrow(env: Env, caller: Address, escrow_id: u64) -> Result<(), PaymentError> {
        caller.require_auth();

        let mut record: EscrowRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)?;

        if record.status != EscrowStatus::Pending {
            return Err(PaymentError::EscrowAlreadySettled);
        }

        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let is_admin = caller == admin;

        if !is_admin {
            if caller != record.sender {
                return Err(PaymentError::Unauthorized);
            }
            if env.ledger().timestamp() < record.release_after {
                return Err(PaymentError::EscrowTimelockActive);
            }
        }

        // Effects before interactions: mark settled, then transfer.
        record.status = EscrowStatus::Refunded;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &record);

        let token_client = token::Client::new(&env, &record.token);
        token_client.transfer(
            &env.current_contract_address(),
            &record.sender,
            &record.amount,
        );

        env.events().publish(
            (symbol_short!("esc_ref"),),
            (escrow_id, record.sender.clone(), record.amount),
        );

        Ok(())
    }

    /// Refund multiple escrowed payments back to their senders in bulk.
    /// Can be initiated by admin immediately, or by a sender for escrows where caller == record.sender and release_after timelock has passed.
    pub fn batch_refund_escrows(
        env: Env,
        caller: Address,
        escrow_ids: Vec<u64>,
    ) -> Result<u32, PaymentError> {
        caller.require_auth();

        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let is_admin = caller == admin;

        let mut count = 0u32;
        for id in escrow_ids.iter() {
            let mut record: EscrowRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Escrow(id))
                .ok_or(PaymentError::EscrowNotFound)?;

            if record.status != EscrowStatus::Pending {
                return Err(PaymentError::EscrowAlreadySettled);
            }

            if !is_admin {
                if caller != record.sender {
                    return Err(PaymentError::Unauthorized);
                }
                if env.ledger().timestamp() < record.release_after {
                    return Err(PaymentError::EscrowTimelockActive);
                }
            }

            record.status = EscrowStatus::Refunded;
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(id), &record);

            let token_client = token::Client::new(&env, &record.token);
            token_client.transfer(
                &env.current_contract_address(),
                &record.sender,
                &record.amount,
            );

            env.events().publish(
                (symbol_short!("esc_ref"),),
                (id, record.sender.clone(), record.amount),
            );

            count += 1;
        }

        Ok(count)
    }

    /// Aggregated on-chain telemetry for all escrow records.
    pub fn get_escrow_metrics(env: Env) -> EscrowMetrics {
        let total_escrows: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);

        let mut pending_count = 0u64;
        let mut disputed_count = 0u64;
        let mut settled_count = 0u64;
        let mut total_locked_amount = 0i128;

        for id in 1..=total_escrows {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<_, EscrowRecord>(&DataKey::Escrow(id))
            {
                match record.status {
                    EscrowStatus::Pending => {
                        pending_count += 1;
                        total_locked_amount += record.amount;
                    }
                    EscrowStatus::Disputed => {
                        disputed_count += 1;
                        total_locked_amount += record.amount;
                    }
                    EscrowStatus::Released | EscrowStatus::Refunded | EscrowStatus::Arbitrated => {
                        settled_count += 1;
                    }
                }
            }
        }

        EscrowMetrics {
            total_escrows,
            pending_count,
            disputed_count,
            settled_count,
            total_locked_amount,
        }
    }

    /// Telemetry and configuration query returning live operational parameters.
    pub fn get_configuration(env: Env) -> Result<PaymentGatewayConfig, PaymentError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PaymentError::Unauthorized)?;

        let pending_admin: Option<Address> = env.storage().instance().get(&DataKey::PendingAdmin);
        let policy_contract: Option<Address> = env.storage().instance().get(&DataKey::PolicyContract);
        let guardian: Option<Address> = env.storage().instance().get(&DataKey::Guardian);
        let escrow_period: u64 = env.storage().instance().get(&DataKey::EscrowPeriod).unwrap_or(0);
        let spend_cap: i128 = env.storage().instance().get(&DataKey::SpendCap).unwrap_or(0);
        let min_payment_amount: i128 = env.storage().instance().get(&DataKey::MinPaymentAmount).unwrap_or(0);
        let is_paused: bool = Self::is_paused(env.clone());
        let require_allowlist: bool = env.storage().instance().get(&DataKey::RequireAllowlist).unwrap_or(false);
        let total_volume: i128 = env.storage().instance().get(&DataKey::TotalVolume).unwrap_or(0);
        let payment_sequence: u64 = env.storage().instance().get(&DataKey::PaymentSequence).unwrap_or(0);

        Ok(PaymentGatewayConfig {
            admin,
            pending_admin,
            policy_contract,
            guardian,
            escrow_period,
            spend_cap,
            min_payment_amount,
            is_paused,
            require_allowlist,
            total_volume,
            payment_sequence,
        })
    }

    /// Retrieve an escrow record by ID.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<EscrowRecord, PaymentError> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)
    }

    /// Retrieve multiple escrow records in a single query invocation.
    /// Non-existent IDs are omitted from the returned list.
    pub fn get_escrows_batch(env: Env, escrow_ids: Vec<u64>) -> Vec<EscrowRecord> {
        let mut results = Vec::new(&env);
        for id in escrow_ids.iter() {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<_, EscrowRecord>(&DataKey::Escrow(id))
            {
                results.push_back(record);
            }
        }
        results
    }

    /// Check whether an escrow record exists and is currently in Pending status.
    pub fn is_active_escrow(env: Env, escrow_id: u64) -> bool {
        match env
            .storage()
            .persistent()
            .get::<_, EscrowRecord>(&DataKey::Escrow(escrow_id))
        {
            Some(record) => record.status == EscrowStatus::Pending,
            None => false,
        }
    }

    /// Retrieve active escrows (in Pending or Disputed status) with offset and limit pagination.
    pub fn get_active_escrows_paged(
        env: Env,
        offset: u32,
        limit: u32,
    ) -> Vec<EscrowRecord> {
        if limit == 0 {
            return Vec::new(&env);
        }

        let total_escrows: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);

        let mut matched = 0u32;
        let mut results = Vec::new(&env);

        for id in 1..=total_escrows {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<_, EscrowRecord>(&DataKey::Escrow(id))
            {
                if record.status == EscrowStatus::Pending || record.status == EscrowStatus::Disputed {
                    if matched >= offset {
                        results.push_back(record);
                        if results.len() >= limit {
                            break;
                        }
                    }
                    matched += 1;
                }
            }
        }

        results
    }

    /// Retrieve all escrow records associated with a specific sender address.
    pub fn get_escrows_by_sender(env: Env, sender: Address) -> Vec<EscrowRecord> {
        let total_escrows: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);
        let mut results = Vec::new(&env);

        for id in 1..=total_escrows {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<_, EscrowRecord>(&DataKey::Escrow(id))
            {
                if record.sender == sender {
                    results.push_back(record);
                }
            }
        }
        results
    }

    /// Retrieve all escrow records associated with a specific recipient address.
    pub fn get_escrows_by_recipient(env: Env, recipient: Address) -> Vec<EscrowRecord> {
        let total_escrows: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);
        let mut results = Vec::new(&env);

        for id in 1..=total_escrows {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<_, EscrowRecord>(&DataKey::Escrow(id))
            {
                if record.recipient == recipient {
                    results.push_back(record);
                }
            }
        }
        results
    }

    /// Read contract configuration.
    /// Retrieve all escrow records currently in Disputed status awaiting arbitration.
    pub fn get_disputed_escrows(env: Env) -> Vec<EscrowRecord> {
        let total_escrows: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);
        let mut results = Vec::new(&env);

        for id in 1..=total_escrows {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<_, EscrowRecord>(&DataKey::Escrow(id))
            {
                if record.status == EscrowStatus::Disputed {
                    results.push_back(record);
                }
            }
        }
        results
    }

    pub fn get_config(env: Env) -> (Address, i128, u64, bool, u64) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let spend_cap: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SpendCap)
            .unwrap_or(0);
        let escrow_period: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowPeriod)
            .unwrap_or(0);
        let is_paused: bool = Self::is_paused(env.clone());
        let total_escrows: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);
        (admin, spend_cap, escrow_period, is_paused, total_escrows)
    }

    /// Add multiple addresses to the denylist in a single batch transaction.
    pub fn add_to_denylist_batch(env: Env, addresses: Vec<Address>) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        for address in addresses.iter() {
            env.storage()
                .persistent()
                .set(&DataKey::Denylist(address.clone()), &true);
            env.events().publish((symbol_short!("deny_add"),), address);
        }
        Ok(())
    }

    /// Remove multiple addresses from the denylist in a single batch transaction.
    pub fn remove_from_denylist_batch(env: Env, addresses: Vec<Address>) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        for address in addresses.iter() {
            env.storage()
                .persistent()
                .remove(&DataKey::Denylist(address.clone()));
            env.events().publish((symbol_short!("deny_rem"),), address);
        }
        Ok(())
    }

    /// Check whether multiple addresses are denylisted in a single batch query.
    pub fn get_denylist_batch(env: Env, addresses: Vec<Address>) -> Vec<bool> {
        let mut results = Vec::new(&env);
        for address in addresses.iter() {
            let is_blocked: bool = env
                .storage()
                .persistent()
                .get(&DataKey::Denylist(address))
                .unwrap_or(false);
            results.push_back(is_blocked);
        }
        results
    }

    /// Add an address to the gateway allowlist. Admin only.
    pub fn add_to_allowlist(env: Env, address: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::Allowlist(address.clone()), &true);
        env.events().publish((symbol_short!("allow_add"),), address);
        Ok(())
    }

    /// Remove an address from the gateway allowlist. Admin only.
    pub fn remove_from_allowlist(env: Env, address: Address) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::Allowlist(address.clone()));
        env.events().publish((symbol_short!("allow_rem"),), address);
        Ok(())
    }

    /// Check if an address is on the gateway allowlist.
    pub fn is_allowlisted(env: Env, address: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Allowlist(address))
            .unwrap_or(false)
    }

    /// Check whether multiple addresses are on the gateway allowlist in a single batch query.
    pub fn get_allowlist_batch(env: Env, addresses: Vec<Address>) -> Vec<bool> {
        let mut results = Vec::new(&env);
        for address in addresses.iter() {
            let is_allowed: bool = env
                .storage()
                .persistent()
                .get(&DataKey::Allowlist(address))
                .unwrap_or(false);
            results.push_back(is_allowed);
        }
        results
    }

    /// Retrieve current spend cap.
    pub fn get_spend_cap(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::SpendCap)
            .unwrap_or(0)
    }

    /// Retrieve total payments sequence number counter (number of processed payments).
    pub fn get_payment_sequence(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::PaymentSequence)
            .unwrap_or(0)
    }

    /// Retrieve total cumulative payment volume processed by the contract in stroops/token units.
    pub fn get_total_volume(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalVolume)
            .unwrap_or(0)
    }

    /// Retrieve total escrows count.
    pub fn get_escrow_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0)
    }

    /// Retrieve current admin address.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Check if contract operations are paused.
    /// If paused with an emergency timelock duration, returns false once ledger timestamp >= paused_until.
    pub fn is_paused(env: Env) -> bool {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false);
        if !paused {
            return false;
        }
        if let Some(until) = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::PauseUntil)
        {
            if until > 0 && env.ledger().timestamp() >= until {
                return false;
            }
        }
        true
    }

    /// Auto-extend instance storage TTL to prevent state archival.
    pub fn extend_ttl(env: Env, threshold: u32, extend_to: u32) {
        env.storage().instance().extend_ttl(threshold, extend_to);
    }

    /// Dry-run simulation of a payment batch against current gateway policy, spend cap,
    /// and recipient denylist status. Does NOT require authentication and does NOT transfer tokens.
    pub fn simulate_payment_batch(
        env: Env,
        sender: Address,
        payments: Vec<PaymentCheckParam>,
    ) -> Vec<PaymentSimulationVerdict> {
        let is_paused: bool = Self::is_paused(env.clone());
        let _global_spend_cap: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SpendCap)
            .unwrap_or(0);
        let min_amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::MinPaymentAmount)
            .unwrap_or(1);
        let is_sender_denylisted = Self::is_denylisted(env.clone(), sender.clone());

        let mut verdicts = Vec::new(&env);
        for payment in payments.iter() {
            if is_paused {
                verdicts.push_back(PaymentSimulationVerdict {
                    recipient: payment.recipient.clone(),
                    amount: payment.amount,
                    status: PaymentStatus::Blocked,
                    reason_code: 104,
                });
                continue;
            }

            if payment.amount < min_amount {
                verdicts.push_back(PaymentSimulationVerdict {
                    recipient: payment.recipient.clone(),
                    amount: payment.amount,
                    status: PaymentStatus::Blocked,
                    reason_code: 105,
                });
                continue;
            }

            if is_sender_denylisted {
                verdicts.push_back(PaymentSimulationVerdict {
                    recipient: payment.recipient.clone(),
                    amount: payment.amount,
                    status: PaymentStatus::Blocked,
                    reason_code: 112,
                });
                continue;
            }

            if Self::is_denylisted(env.clone(), payment.recipient.clone()) {
                verdicts.push_back(PaymentSimulationVerdict {
                    recipient: payment.recipient.clone(),
                    amount: payment.amount,
                    status: PaymentStatus::Blocked,
                    reason_code: 111,
                });
                continue;
            }

            let effective_cap = Self::get_effective_spend_cap(env.clone(), payment.token.clone());
            if payment.amount > effective_cap {
                verdicts.push_back(PaymentSimulationVerdict {
                    recipient: payment.recipient.clone(),
                    amount: payment.amount,
                    status: PaymentStatus::Escrowed,
                    reason_code: 201,
                });
            } else {
                verdicts.push_back(PaymentSimulationVerdict {
                    recipient: payment.recipient.clone(),
                    amount: payment.amount,
                    status: PaymentStatus::Approved,
                    reason_code: 200,
                });
            }
        }

        verdicts
    }

    fn require_admin(env: &Env) -> Result<(), PaymentError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PaymentError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }
}

#[cfg(test)]
mod test;
