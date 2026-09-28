use anchor_lang::prelude::*;

use crate::state::{EstadoLote, Material, ModoOperacao, Papel};

/// Todo evento de lote carrega `seq`, sequencial por lote, para o indexador detectar lacunas ou reordenação.
#[event]
pub struct LoteCriado {
    pub lote: Pubkey,
    pub seq: u64,
    pub cooperativa: Pubkey,
    pub lote_id: u64,
    pub material: Material,
    pub peso_g: u64,
    pub coletor_ref: [u8; 32],
    pub evidencias_hash: [u8; 32],
    pub ts: i64,
}

#[event]
pub struct LoteTransicionado {
    pub lote: Pubkey,
    pub seq: u64,
    pub de: EstadoLote,
    pub para: EstadoLote,
    pub ator: Pubkey,
    pub ts: i64,
}

#[event]
pub struct LoteVendido {
    pub lote: Pubkey,
    pub seq: u64,
    pub industria: Pubkey,
    pub valor_centavos: u64,
    pub escrow_ref_hash: [u8; 32],
    pub lances_root: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MovimentoEscrow {
    Liberacao,
    Reembolso,
}

/// Registro de um movimento do escrow em R$ atestado pelo intermediador.
#[event]
pub struct EscrowMovimentado {
    pub lote: Pubkey,
    pub seq: u64,
    pub movimento: MovimentoEscrow,
    pub ref_hash: [u8; 32],
    pub valor_centavos: u64,
}

#[event]
pub struct LotePesoAlterado {
    pub lote: Pubkey,
    pub seq: u64,
    pub peso_anterior_g: u64,
    pub peso_novo_g: u64,
    pub ator: Pubkey,
}

#[event]
pub struct ParticipanteAtualizado {
    pub carteira: Pubkey,
    pub papel: Papel,
    pub ativo: bool,
    pub kyc_hash: [u8; 32],
}

#[event]
pub struct ConfigAtualizada {
    pub admin: Pubkey,
    pub arbitro: Pubkey,
    pub operador: Pubkey,
    pub intermediador: Pubkey,
    pub credito_autoridade: Pubkey,
    pub prazo_entrega_s: i64,
    pub tolerancia_peso_bps: u16,
    pub modo: ModoOperacao,
}
