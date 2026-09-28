use anchor_lang::prelude::*;
use ecol_lote::Material;

use crate::state::{EstadoCredito, PadraoRegistro};

#[event]
pub struct CreditoCriado {
    pub credito: Pubkey,
    pub seq: u64,
    pub credito_id: u64,
    pub padrao: PadraoRegistro,
    pub material: Material,
    pub vintage: u16,
    pub metodologia_hash: [u8; 32],
}

/// Um evento por lote agregado, em ordem: reconstrói `lotes_hash`.
#[event]
pub struct LoteAgregado {
    pub credito: Pubkey,
    pub seq: u64,
    pub lote: Pubkey,
    pub peso_g: u64,
    pub lotes_hash: [u8; 32],
}

#[event]
pub struct CreditoTransicionado {
    pub credito: Pubkey,
    pub seq: u64,
    pub de: EstadoCredito,
    pub para: EstadoCredito,
    pub ator: Pubkey,
    pub ts: i64,
}

#[event]
pub struct CreditoEmitido {
    pub credito: Pubkey,
    pub seq: u64,
    pub serial: String,
    pub tco2e_milli: u64,
}

#[event]
pub struct CreditoRejeitado {
    pub credito: Pubkey,
    pub seq: u64,
    pub motivo_hash: [u8; 32],
}

#[event]
pub struct CreditoVendido {
    pub credito: Pubkey,
    pub seq: u64,
    pub comprador: Pubkey,
    pub valor_centavos: u64,
}

#[event]
pub struct CreditoAposentado {
    pub credito: Pubkey,
    pub seq: u64,
    pub prova_hash: [u8; 32],
}

#[event]
pub struct CreditoConfigAtualizada {
    pub admin: Pubkey,
    pub admin_pendente: Pubkey,
    pub operador: Pubkey,
    pub registrador: Pubkey,
    pub pausado: bool,
}
