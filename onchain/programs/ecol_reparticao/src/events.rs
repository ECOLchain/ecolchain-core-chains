use anchor_lang::prelude::*;
use ecol_lote::Papel;

use crate::state::BpsReparticao;

#[event]
pub struct DistribuicaoCriada {
    pub distribuicao: Pubkey,
    pub seq: u64,
    pub credito: Pubkey,
    pub receita_centavos: u64,
    pub bps_industria: u16,
    pub total_por_papel: [u64; 4],
    pub ecolchain_centavos: u64,
    pub merkle_root: [u8; 32],
    pub num_folhas: u32,
}

#[event]
pub struct RepasseRegistrado {
    pub distribuicao: Pubkey,
    pub seq: u64,
    pub cupons_centavos: u64,
    pub ecolchain_centavos: u64,
    pub zupy_ref_hash: [u8; 32],
    pub ecolchain_ref_hash: [u8; 32],
}

#[event]
pub struct ResgateRegistrado {
    pub distribuicao: Pubkey,
    pub seq: u64,
    pub indice: u32,
    pub beneficiario: Pubkey,
    pub papel: Papel,
    pub valor_centavos: u64,
    pub recibo_hash: [u8; 32],
}

#[event]
pub struct ReparticaoConfigAtualizada {
    pub admin: Pubkey,
    pub admin_pendente: Pubkey,
    pub operador: Pubkey,
    pub zupy: Pubkey,
    pub intermediador: Pubkey,
    pub bps: BpsReparticao,
    pub pausado: bool,
}
