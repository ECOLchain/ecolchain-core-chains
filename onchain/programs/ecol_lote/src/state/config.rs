use anchor_lang::prelude::*;

use crate::{constants::TOLERANCIA_PESO_MAX_BPS, error::EcolLoteError};

/// Nível de operação do programa (kill switch).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum ModoOperacao {
    /// Todas as instruções liberadas.
    Normal,
    /// Só disputas, crank de prazo e resolução pelo árbitro (ex.: incidente em investigação).
    SomenteDisputas,
    /// Nenhuma transição de lote; só instruções do admin.
    Pausado,
}

/// Configuração global, única por deploy (PDA `["global"]`).
#[account]
#[derive(InitSpace)]
pub struct GlobalConfig {
    /// Squads multisig (vault) que altera esta configuração.
    pub admin: Pubkey,
    /// Multisig que resolve disputas.
    pub arbitro: Pubkey,
    /// Backend da EcolChain: cadastra participantes e encerra leilões.
    pub operador: Pubkey,
    /// Chave do intermediador de pagamento que atesta depósito, liberação e reembolso do escrow em R$.
    pub intermediador: Pubkey,
    /// PDA do programa `ecol_credito` autorizada a marcar lotes como agregados.
    pub credito_autoridade: Pubkey,
    /// Prazo, a partir da venda, para o lote ser recebido pela indústria.
    pub prazo_entrega_s: i64,
    /// Divergência máxima entre o peso da cooperativa e o recebido antes de abrir disputa.
    pub tolerancia_peso_bps: u16,
    pub modo: ModoOperacao,
    pub bump: u8,
    pub _reserved: [u8; 64],
}

impl GlobalConfig {
    pub fn exigir_modo_normal(&self) -> Result<()> {
        require!(self.modo == ModoOperacao::Normal, EcolLoteError::ProgramaPausado);
        Ok(())
    }

    /// Disputas continuam andando em `SomenteDisputas`, para não travar dinheiro retido no escrow.
    pub fn exigir_disputas_liberadas(&self) -> Result<()> {
        require!(self.modo != ModoOperacao::Pausado, EcolLoteError::ProgramaPausado);
        Ok(())
    }

    pub fn validar_parametros(prazo_entrega_s: i64, tolerancia_peso_bps: u16) -> Result<()> {
        require!(prazo_entrega_s > 0, EcolLoteError::ParametroInvalido);
        require!(
            tolerancia_peso_bps <= TOLERANCIA_PESO_MAX_BPS,
            EcolLoteError::ParametroInvalido
        );
        Ok(())
    }
}
