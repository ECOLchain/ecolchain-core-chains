use anchor_lang::prelude::*;

use crate::{
    constants::{
        EXCESSO_MONTAGEM_PADRAO_BPS, PERDA_MONTAGEM_PADRAO_BPS, PRAZO_ENTREGA_MAX_S,
        TOLERANCIA_PESO_MAX_BPS,
    },
    error::EcolLoteError,
};

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

impl ModoOperacao {
    /// Grau de restrição: Normal < SomenteDisputas < Pausado.
    pub fn restricao(self) -> u8 {
        match self {
            ModoOperacao::Normal => 0,
            ModoOperacao::SomenteDisputas => 1,
            ModoOperacao::Pausado => 2,
        }
    }
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
    /// Novo admin proposto; só assume quando ele mesmo assina `admin_accept_admin`.
    pub admin_pendente: Pubkey,
    /// Último retorno ao modo Normal (início da carência do crank).
    pub retomado_em: i64,
    pub bump: u8,
    /// Guardião: só pode restringir o modo (pausar), nunca liberar nem mudar config.
    /// `Pubkey::default()` = sem guardião. Ocupa parte do antigo espaço reservado (layout inalterado).
    pub guardiao: Pubkey,
    /// Perda máxima do lote de venda consolidado sobre a soma dos lotes de origem (0 = padrão).
    pub perda_montagem_bps: u16,
    /// Excesso máximo do lote de venda consolidado sobre a soma dos lotes de origem (0 = padrão).
    pub excesso_montagem_bps: u16,
    pub _reserved: [u8; 28],
}

impl GlobalConfig {
    pub fn exigir_modo_normal(&self) -> Result<()> {
        require!(
            self.modo == ModoOperacao::Normal,
            EcolLoteError::ProgramaPausado
        );
        Ok(())
    }

    /// Disputas e liquidação do escrow continuam em `SomenteDisputas`, para não travar dinheiro retido.
    pub fn exigir_disputas_liberadas(&self) -> Result<()> {
        require!(
            self.modo != ModoOperacao::Pausado,
            EcolLoteError::ProgramaPausado
        );
        Ok(())
    }

    /// Chave de papel do config (não pode ser participante nem acumular outro papel).
    pub fn eh_chave_de_papel(&self, chave: &Pubkey) -> bool {
        [
            self.admin,
            self.arbitro,
            self.operador,
            self.intermediador,
            self.credito_autoridade,
        ]
        .contains(chave)
            || (self.guardiao != Pubkey::default() && *chave == self.guardiao)
    }

    /// Limites vigentes da pesagem do lote consolidado: `(perda, excesso)` em bps.
    /// Configs antigos (campos zerados) usam os padrões.
    pub fn tolerancia_montagem(&self) -> (u16, u16) {
        let ou_padrao = |v: u16, padrao: u16| if v == 0 { padrao } else { v };
        (
            ou_padrao(self.perda_montagem_bps, PERDA_MONTAGEM_PADRAO_BPS),
            ou_padrao(self.excesso_montagem_bps, EXCESSO_MONTAGEM_PADRAO_BPS),
        )
    }

    pub fn validar_parametros(prazo_entrega_s: i64, tolerancia_peso_bps: u16) -> Result<()> {
        require!(
            prazo_entrega_s > 0 && prazo_entrega_s <= PRAZO_ENTREGA_MAX_S,
            EcolLoteError::ParametroInvalido
        );
        require!(
            tolerancia_peso_bps <= TOLERANCIA_PESO_MAX_BPS,
            EcolLoteError::ParametroInvalido
        );
        Ok(())
    }
}
