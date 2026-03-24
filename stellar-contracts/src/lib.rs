#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Bytes, Env,
    Symbol, Vec,
};

// ── Error codes ───────────────────────────────────────────────────────────
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ZeroAmount = 4,
    ExceedsLimit = 5,
    InsufficientFunds = 6,
    ContractPaused = 7,
    WithdrawalLocked = 8,
    RequestNotFound = 9,
    ReferenceTooLong = 10,
    WithdrawalLocked = 7,
    RequestNotFound = 8,
    NotAllowed = 9
    TokenNotWhitelisted = 9,
}

// ── Models ────────────────────────────────────────────────────────────────
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawRequest {
    pub to: Address,
    pub token: Address,
    pub amount: i128,
    pub unlock_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub id: u64,
    pub depositor: Address,
    pub amount: i128,
    pub ledger: u32,
    pub reference: Bytes,
}

/// Maximum allowed length for a deposit reference (bytes).
const MAX_REFERENCE_LEN: u32 = 64;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenConfig {
    pub limit: i128,
    pub total_deposited: i128,
}

// ── Storage keys ──────────────────────────────────────────────────────────
#[contracttype]
pub enum DataKey {
    Admin,
    Token,
    LockPeriod,
    Paused,
    WithdrawQueue(u64),
    NextRequestID,
    ReceiptCounter,
    Receipt(u64),
    AllowlistEnabled,
    Allowed(Address),
}

// ── Events ────────────────────────────────────────────────────────────────
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowlistToggled {
    pub enabled: bool,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowlistAddrAdded {
    #[topic]
    pub addr: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowlistAddrRemoved {
    #[topic]
    pub addr: Address,
    TokenRegistry(Address),
}

// ── Contract ──────────────────────────────────────────────────────────────
#[contract]
pub struct FiatBridge;

#[contractimpl]
impl FiatBridge {
    /// Initialise the bridge once. Sets admin and registers the first whitelisted token.
    pub fn init(env: Env, admin: Address, token: Address, limit: i128) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        if limit <= 0 {
            return Err(Error::ZeroAmount);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        let config = TokenConfig {
            limit,
            total_deposited: 0,
        };
        env.storage()
            .instance()
            .set(&DataKey::TotalDeposited, &0_i128);
        env.storage().instance().set(&DataKey::Paused, &false);
            .persistent()
            .set(&DataKey::TokenRegistry(token), &config);
        Ok(())
    }

    /// Lock tokens inside the bridge and issue a deposit receipt.
    /// Returns the unique receipt ID on success.
    pub fn deposit(
        env: Env,
        from: Address,
        amount: i128,
        reference: Bytes,
    ) -> Result<u64, Error> {
        if Self::is_paused(env.clone()) {
            return Err(Error::ContractPaused);
        }
    /// Lock tokens inside the bridge. Caller must authorise.
    /// The token must be registered in the whitelist.
    pub fn deposit(env: Env, from: Address, amount: i128, token: Address) -> Result<(), Error> {
        from.require_auth();

        // Allowlist gate: when enabled, only approved addresses may deposit.
        let allowlist_on: bool = env
            .storage()
            .instance()
            .get(&DataKey::AllowlistEnabled)
            .unwrap_or(false);
        if allowlist_on {
            if !env
                .storage()
                .persistent()
                .has(&DataKey::Allowed(from.clone()))
            {
                return Err(Error::NotAllowed);
            }
        }

        if amount <= 0 {
            return Err(Error::ZeroAmount);
        }

        let mut config: TokenConfig = env
            .storage()
            .persistent()
            .get(&DataKey::TokenRegistry(token.clone()))
            .ok_or(Error::TokenNotWhitelisted)?;

        if amount > config.limit {
            return Err(Error::ExceedsLimit);
        }

        token::Client::new(&env, &token).transfer(
            &from,
            &env.current_contract_address(),
            &amount,
        );

        // ── Create deposit receipt ────────────────────────────────────
        let receipt_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ReceiptCounter)
            .unwrap_or(0);
        let receipt = Receipt {
            id: receipt_id,
            depositor: from.clone(),
            amount,
            ledger: env.ledger().sequence(),
            reference,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Receipt(receipt_id), &receipt);
        env.storage()
            .instance()
            .set(&DataKey::ReceiptCounter, &(receipt_id + 1));

        // ── Update totals ─────────────────────────────────────────────
        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalDeposited)
            .unwrap_or(0);
        config.total_deposited += amount;
        env.storage()
            .persistent()
            .set(&DataKey::TokenRegistry(token.clone()), &config);

        // ── Events ────────────────────────────────────────────────────
        env.events()
            .publish((Symbol::new(&env, "deposit"), from), amount);
        env.events()
            .publish((Symbol::new(&env, "receipt_issued"),), receipt_id);

        Ok(receipt_id)
    }

    /// Withdraw tokens from the bridge. Caller must authorise.
    pub fn withdraw(env: Env, to: Address, amount: i128) -> Result<(), Error> {
        if Self::is_paused(env.clone()) {
            return Err(Error::ContractPaused);
        }
    /// No whitelist check — allows draining balances of removed tokens.
    pub fn withdraw(env: Env, to: Address, amount: i128, token: Address) -> Result<(), Error> {
        to.require_auth();
        if amount <= 0 {
            return Err(Error::ZeroAmount);
        }

        let token_client = token::Client::new(&env, &token);

        let balance = token_client.balance(&env.current_contract_address());
        if amount > balance {
            return Err(Error::InsufficientFunds);
        }

        token_client.transfer(&env.current_contract_address(), &to, &amount);

        env.events()
            .publish((Symbol::new(&env, "withdraw"), to), amount);

        Ok(())
    }

    /// Register a withdrawal request that matures after the lock period. Admin only.
    pub fn request_withdrawal(env: Env, to: Address, amount: i128, token: Address) -> Result<u64, Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        if amount <= 0 {
            return Err(Error::ZeroAmount);
        }

        let lock_period: u32 = env.storage().instance().get(&DataKey::LockPeriod).unwrap_or(0);
        let unlock_ledger = env.ledger().sequence() + lock_period;

        let request_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextRequestID)
            .unwrap_or(0);

        let request = WithdrawRequest {
            to,
            token,
            amount,
            unlock_ledger,
        };

        env.storage()
            .persistent()
            .set(&DataKey::WithdrawQueue(request_id), &request);
        env.storage()
            .instance()
            .set(&DataKey::NextRequestID, &(request_id + 1));

        Ok(request_id)
    }

    /// Execute a matured withdrawal request.
    pub fn execute_withdrawal(env: Env, request_id: u64) -> Result<(), Error> {
        let request: WithdrawRequest = env
            .storage()
            .persistent()
            .get(&DataKey::WithdrawQueue(request_id))
            .ok_or(Error::RequestNotFound)?;

        if env.ledger().sequence() < request.unlock_ledger {
            return Err(Error::WithdrawalLocked);
        }

        let token_client = token::Client::new(&env, &request.token);

        let balance = token_client.balance(&env.current_contract_address());
        if request.amount > balance {
            return Err(Error::InsufficientFunds);
        }

        token_client.transfer(&env.current_contract_address(), &request.to, &request.amount);

        env.storage()
            .persistent()
            .remove(&DataKey::WithdrawQueue(request_id));

        Ok(())
    }

    /// Cancel a pending withdrawal request. Admin only.
    pub fn cancel_withdrawal(env: Env, request_id: u64) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        if !env
            .storage()
            .persistent()
            .has(&DataKey::WithdrawQueue(request_id))
        {
            return Err(Error::RequestNotFound);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::WithdrawQueue(request_id));
        Ok(())
    }

    /// Set the mandatory delay period for withdrawals (in ledgers). Admin only.
    pub fn set_lock_period(env: Env, ledgers: u32) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::LockPeriod, &ledgers);
        Ok(())
    }

    /// Update the per-deposit limit for a specific token. Admin only.
    pub fn set_limit(env: Env, token: Address, new_limit: i128) -> Result<(), Error> {
        if new_limit <= 0 {
            return Err(Error::ZeroAmount);
        }
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        let mut config: TokenConfig = env
            .storage()
            .persistent()
            .get(&DataKey::TokenRegistry(token.clone()))
            .ok_or(Error::TokenNotWhitelisted)?;
        config.limit = new_limit;
        env.storage()
            .persistent()
            .set(&DataKey::TokenRegistry(token), &config);
        Ok(())
    }

    /// Hand admin rights to a new address. Current admin must authorise.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    pub fn pause(env: Env) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    // ── Token registry management (admin-only) ───────────────────────────

    /// Add a new token to the whitelist. Admin only.
    pub fn add_token(env: Env, token: Address, limit: i128) -> Result<(), Error> {
        if limit <= 0 {
            return Err(Error::ZeroAmount);
        }
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        let config = TokenConfig {
            limit,
            total_deposited: 0,
        };
        env.storage()
            .persistent()
            .set(&DataKey::TokenRegistry(token.clone()), &config);

        env.events()
            .publish((Symbol::new(&env, "token_added"),), token);
        Ok(())
    }

    /// Remove a token from the whitelist. Admin only.
    /// Does not affect existing balances — admin can still drain remaining tokens.
    pub fn remove_token(env: Env, token: Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        if !env
            .storage()
            .persistent()
            .has(&DataKey::TokenRegistry(token.clone()))
        {
            return Err(Error::TokenNotWhitelisted);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::TokenRegistry(token.clone()));

        env.events()
            .publish((Symbol::new(&env, "token_removed"),), token);
        Ok(())
    }

    // ── View functions ────────────────────────────────────────────────────
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }
    /// Returns the default (init) token address.
    pub fn get_token(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)
    }
    /// Per-deposit limit for the default (init) token.
    pub fn get_limit(env: Env) -> Result<i128, Error> {
        let tok: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)?;
        let config: TokenConfig = env
            .storage()
            .persistent()
            .get(&DataKey::TokenRegistry(tok))
            .ok_or(Error::NotInitialized)?;
        Ok(config.limit)
    }
    /// Current balance of the default (init) token held by this contract.
    pub fn get_balance(env: Env) -> Result<i128, Error> {
        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)?;
        Ok(token::Client::new(&env, &token_id).balance(&env.current_contract_address()))
    }
    /// Cumulative deposit total for the default (init) token.
    pub fn get_total_deposited(env: Env) -> Result<i128, Error> {
        let tok: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)?;
        let config: TokenConfig = env
            .storage()
            .persistent()
            .get(&DataKey::TokenRegistry(tok))
            .ok_or(Error::NotInitialized)?;
        Ok(config.total_deposited)
    }
    /// Get details of a withdrawal request.
    pub fn get_withdrawal_request(env: Env, request_id: u64) -> Option<WithdrawRequest> {
        env.storage()
            .persistent()
            .get(&DataKey::WithdrawQueue(request_id))
    }
    /// Get the current lock period in ledgers.
    pub fn get_lock_period(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::LockPeriod).unwrap_or(0)
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    // ── Receipt view functions ─────────────────────────────────────────
    // ── Allowlist management (admin-only) ─────────────────────────────

    /// Enable or disable the deposit allowlist. Admin only.
    pub fn set_allowlist_enabled(env: Env, enabled: bool) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::AllowlistEnabled, &enabled);

        AllowlistToggled { enabled }.publish(&env);
        Ok(())
    }

    /// Add a single address to the deposit allowlist. Admin only.
    pub fn allowlist_add(env: Env, addr: Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::Allowed(addr.clone()), &true);

        AllowlistAddrAdded { addr }.publish(&env);
        Ok(())
    }

    /// Remove a single address from the deposit allowlist. Admin only.
    pub fn allowlist_remove(env: Env, addr: Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        env.storage()
            .persistent()
            .remove(&DataKey::Allowed(addr.clone()));

        AllowlistAddrRemoved { addr }.publish(&env);
        Ok(())
    }

    /// Bulk-add addresses to the deposit allowlist. Admin only.
    pub fn allowlist_add_batch(env: Env, addrs: Vec<Address>) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        for addr in addrs.iter() {
            env.storage()
                .persistent()
                .set(&DataKey::Allowed(addr.clone()), &true);
            AllowlistAddrAdded { addr }.publish(&env);
        }
        Ok(())
    }

    /// Bulk-remove addresses from the deposit allowlist. Admin only.
    pub fn allowlist_remove_batch(env: Env, addrs: Vec<Address>) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        for addr in addrs.iter() {
            env.storage()
                .persistent()
                .remove(&DataKey::Allowed(addr.clone()));
            AllowlistAddrRemoved { addr }.publish(&env);
        }
        Ok(())
    }

    // ── Allowlist view functions ───────────────────────────────────────

    /// Check whether a given address is on the allowlist.
    pub fn is_allowed(env: Env, addr: Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Allowed(addr))
    }

    /// Check whether the deposit allowlist is currently enabled.
    pub fn get_allowlist_enabled(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::AllowlistEnabled)
            .unwrap_or(false)
    }
    /// Look up a token's configuration (limit and cumulative deposits).
    pub fn get_token_config(env: Env, token: Address) -> Option<TokenConfig> {
        env.storage()
            .persistent()
            .get(&DataKey::TokenRegistry(token))
    }
}

mod test;
