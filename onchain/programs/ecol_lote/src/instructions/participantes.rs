use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_GLOBAL, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::ParticipanteAtualizado,
    state::{GlobalConfig, Papel, Participante},
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(carteira: Pubkey)]
pub struct OperadorRegisterParticipante<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        init,
        payer = payer,
        space = 8 + Participante::INIT_SPACE,
        seeds = [SEED_PARTICIPANTE, carteira.as_ref()],
        bump
    )]
    pub participante: Account<'info, Participante>,
    pub system_program: Program<'info, System>,
}

pub fn handle_operador_register_participante(
    ctx: Context<OperadorRegisterParticipante>,
    carteira: Pubkey,
    papel: Papel,
    kyc_hash: [u8; 32],
    nome: String,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_modo_normal()?;
    require!(kyc_hash != [0; 32], EcolLoteError::ReferenciaVazia);
    require!(
        carteira != Pubkey::default(),
        EcolLoteError::ParametroInvalido
    );
    require!(
        !config.eh_chave_de_papel(&carteira),
        EcolLoteError::CarteiraReservada
    );
    let nome = Participante::codificar_nome(&nome)?;
    let participante = &mut ctx.accounts.participante;
    participante.carteira = carteira;
    participante.papel = papel;
    participante.ativo = true;
    participante.kyc_hash = kyc_hash;
    participante.bump = ctx.bumps.participante;
    participante.nome = nome;
    emit_cpi!(ParticipanteAtualizado {
        carteira,
        papel,
        ativo: true,
        kyc_hash,
        nome,
    });
    Ok(())
}

/// Ativa ou desativa um participante (o papel não muda: para trocar de papel, usar outra carteira).
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorSetParticipanteAtivo<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_PARTICIPANTE, participante.carteira.as_ref()],
        bump = participante.bump
    )]
    pub participante: Account<'info, Participante>,
}

pub fn handle_operador_set_participante_ativo(
    ctx: Context<OperadorSetParticipanteAtivo>,
    ativo: bool,
) -> Result<()> {
    let participante = &mut ctx.accounts.participante;
    participante.ativo = ativo;
    emit_cpi!(ParticipanteAtualizado {
        carteira: participante.carteira,
        papel: participante.papel,
        ativo,
        kyc_hash: participante.kyc_hash,
        nome: participante.nome,
    });
    Ok(())
}

/// Define ou corrige o nome de exibição de um participante (também preenche cadastros antigos, sem nome).
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorSetParticipanteNome<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_PARTICIPANTE, participante.carteira.as_ref()],
        bump = participante.bump
    )]
    pub participante: Account<'info, Participante>,
}

pub fn handle_operador_set_participante_nome(
    ctx: Context<OperadorSetParticipanteNome>,
    nome: String,
) -> Result<()> {
    let participante = &mut ctx.accounts.participante;
    participante.nome = Participante::codificar_nome(&nome)?;
    emit_cpi!(ParticipanteAtualizado {
        carteira: participante.carteira,
        papel: participante.papel,
        ativo: participante.ativo,
        kyc_hash: participante.kyc_hash,
        nome: participante.nome,
    });
    Ok(())
}

impl Participante {
    pub fn exigir(&self, papel: Papel) -> Result<()> {
        require!(self.ativo, EcolLoteError::ParticipanteInativo);
        require!(self.papel == papel, EcolLoteError::PapelIncorreto);
        Ok(())
    }
}
