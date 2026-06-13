// No imports needed for these constants

pub const PLATFORM_AUTHORITY_SEED: &[u8] = b"platform_authority";
pub const TITLE_CONFIG_SEED: &[u8] = b"title_config";
pub const UNLOCK_RECEIPT_SEED: &[u8] = b"unlock_receipt";
pub const USER_VAULT_SEED: &[u8] = b"user_vault";
pub const REVENUE_VAULT_SEED: &[u8] = b"revenue_vault";

pub const BPS_DENOMINATOR: u16 = 10_000;
pub const SCALE: u128 = 1_000_000_000_000; // accumulator precision (1e12)
pub const MAX_TITLE_ID_LEN: usize = 32;
pub const MAX_EPISODE_ID_LEN: usize = 32;
