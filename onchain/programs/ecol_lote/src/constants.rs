use anchor_lang::prelude::*;

#[constant]
pub const SEED_GLOBAL: &[u8] = b"global";

#[constant]
pub const SEED_PARTICIPANTE: &[u8] = b"part";

#[constant]
pub const SEED_LOTE: &[u8] = b"lote";

/// Base dos percentuais (basis points): 10_000 = 100%.
pub const BPS_BASE: u64 = 10_000;

/// Limite superior de `tolerancia_peso_bps` (20%), para impedir que a verificação de peso seja desligada.
pub const TOLERANCIA_PESO_MAX_BPS: u16 = 2_000;
