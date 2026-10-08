use anchor_lang::prelude::*;

use crate::error::EcolLoteError;

/// Lote de distribuição (PDA `["distrib", importador, id]`): garrafas que o importador pôs no mercado.
/// É a base da meta de logística reversa; não segue para venda (ADR 0011).
#[account]
#[derive(InitSpace)]
pub struct Distribuicao {
    pub importador: Pubkey,
    pub distrib_id: u64,
    pub material: u16,
    pub variacao: u8,
    pub qtd_garrafas: u32,
    /// Peso estimado em gramas (o importador não pesa em balança atestada).
    pub peso_estimado_g: u64,
    /// Hash da NF/DI de importação (texto normalizado, como as referências de origem).
    pub doc_hash: [u8; 32],
    /// Data de emissão da NF/DI informada pelo importador (o registro pode ser posterior).
    pub emitido_em: i64,
    pub criado_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 32],
}

/// Etapas da coleta do importador.
///
/// ```text
/// Coletada → EmEntrega → Entregue → Reciclada
///            EmEntrega → Coletada (destino recusou / importador cancelou)
/// ```
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum EstadoColeta {
    /// Garrafas recolhidas pelo importador.
    Coletada,
    /// A caminho de uma cooperativa ou Clean Tech (`destino`).
    EmEntrega,
    /// O destino pesou e registrou o lote de origem (`entrega`).
    Entregue,
    /// O lote de venda que recebeu a origem foi reciclado e o importador anexou a NF.
    Reciclada,
}

/// Coleta do importador (PDA `["coleta", importador, id]`). Na entrega vira um lote de origem
/// (`Entrega` com origem `Importador` e `origem_ref` = chave desta conta) na cooperativa ou Clean Tech.
#[account]
#[derive(InitSpace)]
pub struct Coleta {
    pub importador: Pubkey,
    pub coleta_id: u64,
    pub material: u16,
    pub variacao: u8,
    pub qtd_garrafas: u32,
    pub peso_estimado_g: u64,
    /// Hash do ponto de coleta (LGPD: nunca o endereço em texto).
    pub local_ref: [u8; 32],
    /// Titular da cooperativa ou Clean Tech de destino (`Pubkey::default()` antes de enviar).
    pub destino: Pubkey,
    /// Lote de origem criado pelo destino.
    pub entrega: Pubkey,
    /// Hash da NF de reciclagem informada pelo importador.
    pub nf_reciclagem_hash: [u8; 32],
    pub estado: EstadoColeta,
    /// Data da coleta informada pelo importador (o registro pode ser posterior).
    pub coletado_em: i64,
    pub criado_em: i64,
    pub atualizado_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 32],
}

impl Coleta {
    pub fn exigir_estado(&self, estado: EstadoColeta) -> Result<()> {
        require!(self.estado == estado, EcolLoteError::ColetaEstadoInvalido);
        Ok(())
    }
}
