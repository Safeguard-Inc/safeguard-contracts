#![no_std]
#![allow(deprecated)]
#![allow(clippy::all)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol,
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
#[derive(Clone)]
enum DataKey {
    Admin,
    PolicyContract,
    EscrowPeriod,
    EscrowCounter,
    Escrow(u64),
    SpendCap,
    IsPaused,
    Denylist(Address),
    Allowlist(Address),
    RequireAllowlist,
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

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::EscrowPeriod, &escrow_period);
        env.storage().instance().set(&DataKey::SpendCap, &spend_cap);
        env.storage().instance().set(&DataKey::IsPaused, &false);
        env.storage().instance().set(&DataKey::EscrowCounter, &0u64);
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

    /// Pause or unpause payment operations.
    pub fn set_paused(env: Env, paused: bool) -> Result<(), PaymentError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::IsPaused, &paused);
        env.events().publish((symbol_short!("paused"),), paused);
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

        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false);
        if is_paused {
            return Err(PaymentError::ContractPaused);
        }

        if amount <= 0 {
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

        let spend_cap: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SpendCap)
            .unwrap_or(i128::MAX);
        let timestamp = env.ledger().timestamp();

        // 3. Routing: direct vs escrow
        if amount > spend_cap {
            // Divert to escrow for admin review
            let token_client = token::Client::new(&env, &token);
            token_client.transfer(&sender, &env.current_contract_address(), &amount);

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

        let token_client = token::Client::new(&env, &record.token);
        token_client.transfer(
            &env.current_contract_address(),
            &record.recipient,
            &record.amount,
        );

        record.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &record);

        env.events().publish(
            (symbol_short!("esc_rel"),),
            (escrow_id, record.recipient.clone(), record.amount),
        );

        Ok(())
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

        let token_client = token::Client::new(&env, &record.token);
        token_client.transfer(
            &env.current_contract_address(),
            &record.sender,
            &record.amount,
        );

        record.status = EscrowStatus::Refunded;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &record);

        env.events().publish(
            (symbol_short!("esc_ref"),),
            (escrow_id, record.sender.clone(), record.amount),
        );

        Ok(())
    }

    /// Retrieve an escrow record by ID.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<EscrowRecord, PaymentError> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(PaymentError::EscrowNotFound)
    }

    /// Read contract configuration.
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
        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false);
        let total_escrows: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);
        (admin, spend_cap, escrow_period, is_paused, total_escrows)
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
