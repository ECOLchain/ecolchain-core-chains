use anchor_lang::prelude::*;

#[constant]
pub const SEED_CONFIG: &[u8] = b"credito_config";

#[constant]
pub const SEED_CREDITO: &[u8] = b"credito";

/// Unicidade do número de série do registro: PDA `["serial", padrão, sha256(serial)]`.
#[constant]
pub const SEED_SERIAL: &[u8] = b"serial";

/// PDA que assina a CPI `credito_mark_agregado` no `ecol_lote`. Deve ser a `credito_autoridade`
/// gravada no `GlobalConfig` do `ecol_lote`.
#[constant]
pub const SEED_AUTORIDADE: &[u8] = b"autoridade_credito";

pub const SERIAL_MAX_LEN: usize = 64;

/// Lotes por transação em `operador_add_lotes` (cada lote custa uma CPI e 1 conta gravável).
pub const MAX_LOTES_POR_TX: usize = 10;
