use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_GLOBAL, TOLERANCIA_MONTAGEM_MAX_BPS},
    error::EcolLoteError,
    events::ConfigAtualizada,
    program::EcolLote,
    state::{GlobalConfig, ModoOperacao},
};

/// Parâmetros alteráveis pelo admin. O próprio admin muda só em dois passos
/// (`admin_propose_admin` + `admin_accept_admin`), para um erro de digitação não travar o config.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ConfigParams {
    pub arbitro: Pubkey,
    pub operador: Pubkey,
    pub intermediador: Pubkey,
    pub credito_autoridade: Pubkey,
    pub prazo_entrega_s: i64,
    pub tolerancia_peso_bps: u16,
}

impl GlobalConfig {
    fn aplicar(&mut self, p: &ConfigParams) -> Result<()> {
        GlobalConfig::validar_parametros(p.prazo_entrega_s, p.tolerancia_peso_bps)?;
        // Um papel por chave: por exemplo, árbitro == intermediador reembolsaria sozinho.
        let chaves = [
            self.admin,
            p.arbitro,
            p.operador,
            p.intermediador,
            p.credito_autoridade,
        ];
        for (i, a) in chaves.iter().enumerate() {
            require!(
                *a != Pubkey::default(),
                EcolLoteError::ChavesDeConfigInvalidas
            );
            require!(
                !chaves[..i].contains(a),
                EcolLoteError::ChavesDeConfigInvalidas
            );
        }
        self.arbitro = p.arbitro;
        self.operador = p.operador;
        self.intermediador = p.intermediador;
        self.credito_autoridade = p.credito_autoridade;
        self.prazo_entrega_s = p.prazo_entrega_s;
        self.tolerancia_peso_bps = p.tolerancia_peso_bps;
        Ok(())
    }

    fn evento(&self) -> ConfigAtualizada {
        ConfigAtualizada {
            admin: self.admin,
            admin_pendente: self.admin_pendente,
            arbitro: self.arbitro,
            operador: self.operador,
            intermediador: self.intermediador,
            credito_autoridade: self.credito_autoridade,
            prazo_entrega_s: self.prazo_entrega_s,
            tolerancia_peso_bps: self.tolerancia_peso_bps,
            modo: self.modo,
            guardiao: self.guardiao,
            perda_montagem_bps: self.tolerancia_montagem().0,
            excesso_montagem_bps: self.tolerancia_montagem().1,
        }
    }
}

/// Cria a configuração global. Exige a upgrade authority para ninguém se adiantar ao deploy.
#[event_cpi]
#[derive(Accounts)]
pub struct AdminInitGlobal<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub upgrade_authority: Signer<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + GlobalConfig::INIT_SPACE,
        seeds = [SEED_GLOBAL],
        bump
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(constraint = este_programa.programdata_address()? == Some(program_data.key()))]
    pub este_programa: Program<'info, EcolLote>,
    #[account(
        constraint = program_data.upgrade_authority_address == Some(upgrade_authority.key())
            @ EcolLoteError::NaoEhUpgradeAuthority
    )]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

pub fn handle_admin_init_global(
    ctx: Context<AdminInitGlobal>,
    admin: Pubkey,
    params: ConfigParams,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = admin;
    config.aplicar(&params)?;
    config.modo = ModoOperacao::Normal;
    config.bump = ctx.bumps.config;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct AdminUpdateGlobal<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        has_one = admin @ EcolLoteError::NaoEhAdmin
    )]
    pub config: Account<'info, GlobalConfig>,
}

pub fn handle_admin_update_global(
    ctx: Context<AdminUpdateGlobal>,
    params: ConfigParams,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.aplicar(&params)?;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

pub fn handle_admin_set_modo(ctx: Context<AdminUpdateGlobal>, modo: ModoOperacao) -> Result<()> {
    let config = &mut ctx.accounts.config;
    if modo == ModoOperacao::Normal && config.modo != ModoOperacao::Normal {
        config.retomado_em = Clock::get()?.unix_timestamp;
    }
    config.modo = modo;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

/// Passo 1 da troca de admin. `Pubkey::default()` cancela uma proposta pendente.
pub fn handle_admin_propose_admin(
    ctx: Context<AdminUpdateGlobal>,
    novo_admin: Pubkey,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        novo_admin == Pubkey::default() || !config.eh_chave_de_papel(&novo_admin),
        EcolLoteError::ChavesDeConfigInvalidas
    );
    config.admin_pendente = novo_admin;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

/// Passo 2: o novo admin prova que controla a chave assinando.
#[event_cpi]
#[derive(Accounts)]
pub struct AdminAcceptAdmin<'info> {
    pub novo_admin: Signer<'info>,
    #[account(
        mut,
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        constraint = config.admin_pendente == novo_admin.key() @ EcolLoteError::SemAdminPendente
    )]
    pub config: Account<'info, GlobalConfig>,
}

pub fn handle_admin_accept_admin(ctx: Context<AdminAcceptAdmin>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.novo_admin.key();
    config.admin_pendente = Pubkey::default();
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

/// Define (ou remove, com `Pubkey::default()`) o guardião, que só pode pausar.
pub fn handle_admin_set_guardiao(ctx: Context<AdminUpdateGlobal>, guardiao: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        guardiao != config.admin,
        EcolLoteError::ChavesDeConfigInvalidas
    );
    config.guardiao = guardiao;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

/// Faixa aceita na pesagem do lote de venda consolidado sobre a soma dos lotes de origem.
pub fn handle_admin_set_tolerancia_montagem(
    ctx: Context<AdminUpdateGlobal>,
    perda_bps: u16,
    excesso_bps: u16,
) -> Result<()> {
    require!(
        perda_bps > 0
            && excesso_bps > 0
            && perda_bps <= TOLERANCIA_MONTAGEM_MAX_BPS
            && excesso_bps <= TOLERANCIA_MONTAGEM_MAX_BPS,
        EcolLoteError::ParametroInvalido
    );
    let config = &mut ctx.accounts.config;
    config.perda_montagem_bps = perda_bps;
    config.excesso_montagem_bps = excesso_bps;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

/// O guardião restringe o modo sem timelock (ex.: incidente). Voltar ao Normal só pelo admin.
#[event_cpi]
#[derive(Accounts)]
pub struct GuardiaoPausar<'info> {
    pub guardiao: Signer<'info>,
    #[account(
        mut,
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        constraint = config.guardiao != Pubkey::default()
            && config.guardiao == guardiao.key() @ EcolLoteError::NaoEhGuardiao
    )]
    pub config: Account<'info, GlobalConfig>,
}

pub fn handle_guardiao_pausar(ctx: Context<GuardiaoPausar>, modo: ModoOperacao) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        modo.restricao() > config.modo.restricao(),
        EcolLoteError::GuardiaoSoRestringe
    );
    config.modo = modo;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}
