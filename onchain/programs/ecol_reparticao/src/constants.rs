use anchor_lang::prelude::*;

#[constant]
pub const SEED_CONFIG: &[u8] = b"rep_config";

/// Uma distribuição por crédito vendido: PDA `["dist", credito]`.
#[constant]
pub const SEED_DISTRIBUICAO: &[u8] = b"dist";

pub const BPS_BASE: u64 = 10_000;

/// Fatia mínima da EcolChain (1%), que recebe também o resíduo de arredondamento.
pub const ECOLCHAIN_MIN_BPS: u16 = 100;

/// Limite de beneficiários por distribuição (bitmap de 512 bytes; prova de até 12 níveis).
pub const MAX_FOLHAS: u32 = 4_096;

pub const MAX_PROVA: usize = 16;
