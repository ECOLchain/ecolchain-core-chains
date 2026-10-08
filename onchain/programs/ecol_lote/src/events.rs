use anchor_lang::prelude::*;

use crate::state::{EstadoColeta, EstadoLote, ModoOperacao, OrigemEntrega, Papel};

/// Todo evento de lote carrega `seq`, sequencial por lote, para o indexador detectar lacunas ou reordenação.
///
/// Lote de venda aberto para montagem (ainda sem pesagem nem Core asset).
#[event]
pub struct LoteMontagemIniciada {
    pub lote: Pubkey,
    pub seq: u64,
    pub cooperativa: Pubkey,
    pub lote_id: u64,
    pub material: u16,
    pub evidencias_hash: [u8; 32],
    pub ts: i64,
}

/// Lote de venda fechado: pesagem do consolidado registrada e Core asset criado.
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
    pub nome: [u8; 32],
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
    /// Faixa vigente da pesagem do lote consolidado (já com os padrões aplicados).
    pub perda_montagem_bps: u16,
    pub excesso_montagem_bps: u16,
}

#[event]
pub struct BalancaAtualizada {
    pub balanca: Pubkey,
    pub dono: Pubkey,
    pub dispositivo: Pubkey,
    pub ativa: bool,
    pub nome: [u8; 32],
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
    pub origem: OrigemEntrega,
    pub origem_ref: [u8; 32],
    pub material: u16,
    pub peso_g: u64,
    pub balanca: Pubkey,
    pub ts_pesagem: i64,
    pub variacao: u8,
    pub qtd_garrafas: u32,
}

/// Um evento por entrega vinculada, em ordem: reconstrói `entregas_hash` do lote.
#[event]
pub struct EntregaNoLote {
    pub lote: Pubkey,
    pub seq: u64,
    pub entrega: Pubkey,
    pub origem: OrigemEntrega,
    pub origem_ref: [u8; 32],
    pub peso_g: u64,
}

/// Lote de origem devolvido ao desfazer a união; volta a ficar disponível.
#[event]
pub struct EntregaDevolvida {
    pub lote: Pubkey,
    pub seq: u64,
    pub entrega: Pubkey,
    pub peso_g: u64,
}

#[event]
pub struct VariacaoAtualizada {
    pub variacao: Pubkey,
    pub material: u16,
    pub indice: u8,
    pub nome: [u8; 32],
    pub ativa: bool,
}

#[event]
pub struct MaterialGarrafasAtualizado {
    pub material: Pubkey,
    pub codigo: u16,
    pub conta_garrafas: bool,
}

/// Carteira vinculada, renomeada, ativada ou desativada.
#[event]
pub struct CarteiraAtualizada {
    pub endereco: Pubkey,
    pub participante: Pubkey,
    pub nome: [u8; 32],
    pub ativa: bool,
    /// Quem fez a alteração (operador ou titular).
    pub ator: Pubkey,
}

/// Garrafas postas no mercado pelo importador.
#[event]
pub struct DistribuicaoRegistrada {
    pub distribuicao: Pubkey,
    pub importador: Pubkey,
    pub distrib_id: u64,
    pub material: u16,
    pub variacao: u8,
    pub qtd_garrafas: u32,
    pub peso_estimado_g: u64,
    pub doc_hash: [u8; 32],
    pub emitido_em: i64,
    pub ts: i64,
}

/// Coleta do importador criada ou mudando de etapa.
#[event]
pub struct ColetaAtualizada {
    pub coleta: Pubkey,
    pub importador: Pubkey,
    pub de: Option<EstadoColeta>,
    pub para: EstadoColeta,
    pub destino: Pubkey,
    pub entrega: Pubkey,
    pub nf_reciclagem_hash: [u8; 32],
    /// Carteira que assinou.
    pub ator: Pubkey,
    pub ts: i64,
}
