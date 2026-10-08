use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_CARTEIRA, SEED_GLOBAL, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::{CarteiraAtualizada, ParticipanteAtualizado},
    state::{codificar_nome, Carteira, GlobalConfig, Papel, Participante},
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
    /// CHECK: só se confere que está vazia: carteira vinculada a outro ator não vira titular.
    #[account(seeds = [SEED_CARTEIRA, carteira.as_ref()], bump)]
    pub vinculo: UncheckedAccount<'info>,
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
    require!(
        ctx.accounts.vinculo.data_is_empty(),
        EcolLoteError::CarteiraEmUso
    );
    let nome = codificar_nome(&nome)?;
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
    participante.nome = codificar_nome(&nome)?;
    emit_cpi!(ParticipanteAtualizado {
        carteira: participante.carteira,
        papel: participante.papel,
        ativo: participante.ativo,
        kyc_hash: participante.kyc_hash,
        nome: participante.nome,
    });
    Ok(())
}

/// Vincula uma carteira ao participante, com nome (ADR 0011). Assina o operador ou o titular.
/// O endereço não pode ser titular de outro cadastro; o do próprio titular pode, só para nomeá-lo.
#[event_cpi]
#[derive(Accounts)]
#[instruction(endereco: Pubkey)]
pub struct ParticipanteAddCarteira<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// Operador ou carteira titular do participante.
    pub autoridade: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        seeds = [SEED_PARTICIPANTE, participante.carteira.as_ref()],
        bump = participante.bump
    )]
    pub participante: Account<'info, Participante>,
    /// CHECK: cadastro de titular do endereço; tem de estar vazio, salvo se for o próprio titular.
    #[account(seeds = [SEED_PARTICIPANTE, endereco.as_ref()], bump)]
    pub endereco_part: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + Carteira::INIT_SPACE,
        seeds = [SEED_CARTEIRA, endereco.as_ref()],
        bump
    )]
    pub carteira: Account<'info, Carteira>,
    pub system_program: Program<'info, System>,
}

pub fn handle_participante_add_carteira(
    ctx: Context<ParticipanteAddCarteira>,
    endereco: Pubkey,
    nome: String,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_modo_normal()?;
    let participante = &ctx.accounts.participante;
    let ator = ctx.accounts.autoridade.key();
    exigir_gestor(config, participante, &ator)?;
    require!(
        endereco != Pubkey::default(),
        EcolLoteError::ParametroInvalido
    );
    require!(
        !config.eh_chave_de_papel(&endereco),
        EcolLoteError::CarteiraReservada
    );
    require!(
        endereco == participante.carteira || ctx.accounts.endereco_part.data_is_empty(),
        EcolLoteError::CarteiraEmUso
    );
    let nome = codificar_nome(&nome)?;
    let titular = participante.carteira;
    let carteira = &mut ctx.accounts.carteira;
    carteira.endereco = endereco;
    carteira.participante = titular;
    carteira.nome = nome;
    carteira.ativa = true;
    carteira.criada_em = Clock::get()?.unix_timestamp;
    carteira.bump = ctx.bumps.carteira;
    emit_cpi!(CarteiraAtualizada {
        endereco,
        participante: titular,
        nome,
        ativa: true,
        ator,
    });
    Ok(())
}

/// Renomeia, ativa ou desativa uma carteira vinculada. Assina o operador ou o titular.
/// Desativar o registro do próprio titular não tira dele a assinatura (só o operador desativa o ator).
#[event_cpi]
#[derive(Accounts)]
pub struct ParticipanteUpdateCarteira<'info> {
    pub autoridade: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        seeds = [SEED_PARTICIPANTE, participante.carteira.as_ref()],
        bump = participante.bump
    )]
    pub participante: Account<'info, Participante>,
    #[account(
        mut,
        seeds = [SEED_CARTEIRA, carteira.endereco.as_ref()],
        bump = carteira.bump,
        constraint = carteira.participante == participante.carteira @ EcolLoteError::CarteiraNaoAutorizada
    )]
    pub carteira: Account<'info, Carteira>,
}

pub fn handle_participante_update_carteira(
    ctx: Context<ParticipanteUpdateCarteira>,
    nome: String,
    ativa: bool,
) -> Result<()> {
    let ator = ctx.accounts.autoridade.key();
    exigir_gestor(&ctx.accounts.config, &ctx.accounts.participante, &ator)?;
    let carteira = &mut ctx.accounts.carteira;
    carteira.nome = codificar_nome(&nome)?;
    carteira.ativa = ativa;
    emit_cpi!(CarteiraAtualizada {
        endereco: carteira.endereco,
        participante: carteira.participante,
        nome: carteira.nome,
        ativa,
        ator,
    });
    Ok(())
}

/// Gerência das carteiras: o operador ou o titular (ativo). Carteira vinculada não inclui outra.
fn exigir_gestor(config: &GlobalConfig, participante: &Participante, ator: &Pubkey) -> Result<()> {
    if *ator == config.operador {
        return Ok(());
    }
    require_keys_eq!(*ator, participante.carteira, EcolLoteError::NaoEhOperador);
    require!(participante.ativo, EcolLoteError::ParticipanteInativo);
    Ok(())
}

impl Participante {
    pub fn exigir(&self, papel: Papel) -> Result<()> {
        require!(self.ativo, EcolLoteError::ParticipanteInativo);
        require!(self.papel == papel, EcolLoteError::PapelIncorreto);
        Ok(())
    }

    /// Cooperativa ou Clean Tech ativa.
    pub fn exigir_recebedor(&self) -> Result<()> {
        require!(self.ativo, EcolLoteError::ParticipanteInativo);
        require!(self.papel.eh_recebedor(), EcolLoteError::PapelIncorreto);
        Ok(())
    }
}
