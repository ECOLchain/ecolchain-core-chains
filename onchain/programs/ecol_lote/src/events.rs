use anchor_lang::prelude::*;

use crate::state::{EstadoLote, ModoOperacao, Papel};

/// Todo evento de lote carrega `seq`, sequencial por lote, para o indexador detectar lacunas ou reordenação.
#[event]
pub struct LoteCriado {
    pub lote: Pubkey,
    pub seq: u64,
    pub cooperativa: Pubkey,
    pub lote_id: u64,
    pub material: u16,
    pub peso_g: u64,
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
    pub admin_pendente: Pubkey,
    pub arbitro: Pubkey,
    pub operador: Pubkey,
    pub intermediador: Pubkey,
    pub credito_autoridade: Pubkey,
    pub prazo_entrega_s: i64,
    pub tolerancia_peso_bps: u16,
    pub modo: ModoOperacao,
    pub guardiao: Pubkey,
}

#[event]
pub struct BalancaAtualizada {
    pub balanca: Pubkey,
    pub dono: Pubkey,
    pub dispositivo: Pubkey,
    pub ativa: bool,
}

/// Pesagem atestada por balança (origem ou recebimento).
#[event]
pub struct PesagemAtestada {
    pub lote: Pubkey,
    pub seq: u64,
    pub balanca: Pubkey,
    pub peso_g: u64,
    pub ts_pesagem: i64,
    pub recebimento: bool,
}

#[event]
pub struct MaterialAtualizado {
    pub material: Pubkey,
    pub codigo: u16,
    pub nome: String,
    pub ativo: bool,
}

#[event]
pub struct EntregaRegistrada {
    pub entrega: Pubkey,
    pub cooperativa: Pubkey,
    pub entrega_id: u64,
    pub coletor_ref: [u8; 32],
    pub material: u16,
    pub peso_g: u64,
    pub balanca: Pubkey,
    pub ts_pesagem: i64,
}

/// Um evento por entrega vinculada, em ordem: reconstrói `entregas_hash` do lote.
#[event]
pub struct EntregaNoLote {
    pub lote: Pubkey,
    pub seq: u64,
    pub entrega: Pubkey,
    pub coletor_ref: [u8; 32],
    pub peso_g: u64,
}
