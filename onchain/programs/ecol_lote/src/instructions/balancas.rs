use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_BALANCA, SEED_GLOBAL, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::BalancaAtualizada,
    state::{codificar_nome, Balanca, GlobalConfig, Papel, Participante},
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(dispositivo: Pubkey)]
pub struct OperadorRegisterBalanca<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(seeds = [SEED_PARTICIPANTE, dono_part.carteira.as_ref()], bump = dono_part.bump)]
    pub dono_part: Account<'info, Participante>,
    #[account(
        init,
        payer = payer,
        space = 8 + Balanca::INIT_SPACE,
        seeds = [SEED_BALANCA, dispositivo.as_ref()],
        bump
    )]
    pub balanca: Account<'info, Balanca>,
    pub system_program: Program<'info, System>,
}

pub fn handle_operador_register_balanca(
    ctx: Context<OperadorRegisterBalanca>,
    dispositivo: Pubkey,
    nome: String,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let nome = codificar_nome(&nome)?;
    let dono_part = &ctx.accounts.dono_part;
    require!(dono_part.ativo, EcolLoteError::ParticipanteInativo);
    require!(
        matches!(dono_part.papel, Papel::Cooperativa | Papel::Industria),
        EcolLoteError::PapelIncorreto
    );
    let dono = dono_part.carteira;
    let balanca = &mut ctx.accounts.balanca;
    balanca.dono = dono;
    balanca.dispositivo = dispositivo;
    balanca.ativa = true;
    balanca.bump = ctx.bumps.balanca;
    balanca.nome = nome;
    emit_cpi!(BalancaAtualizada {
        balanca: balanca.key(),
        dono,
        dispositivo,
        ativa: true,
        nome,
    });
    Ok(())
}

/// Desativa uma balança (equipamento trocado, chave comprometida, aferição vencida) ou a reativa.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorSetBalancaAtiva<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_BALANCA, balanca.dispositivo.as_ref()],
        bump = balanca.bump
    )]
    pub balanca: Account<'info, Balanca>,
}

pub fn handle_operador_set_balanca_ativa(
    ctx: Context<OperadorSetBalancaAtiva>,
    ativa: bool,
) -> Result<()> {
    let balanca = &mut ctx.accounts.balanca;
    balanca.ativa = ativa;
    emit_cpi!(BalancaAtualizada {
        balanca: balanca.key(),
        dono: balanca.dono,
        dispositivo: balanca.dispositivo,
        ativa,
        nome: balanca.nome,
    });
    Ok(())
}

/// Define ou corrige o nome de exibição de uma balança (também preenche cadastros antigos, sem nome).
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorSetBalancaNome<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_BALANCA, balanca.dispositivo.as_ref()],
        bump = balanca.bump
    )]
    pub balanca: Account<'info, Balanca>,
}

pub fn handle_operador_set_balanca_nome(
    ctx: Context<OperadorSetBalancaNome>,
    nome: String,
) -> Result<()> {
    let balanca = &mut ctx.accounts.balanca;
    balanca.nome = codificar_nome(&nome)?;
    emit_cpi!(BalancaAtualizada {
        balanca: balanca.key(),
        dono: balanca.dono,
        dispositivo: balanca.dispositivo,
        ativa: balanca.ativa,
        nome: balanca.nome,
    });
    Ok(())
}

impl Balanca {
    pub fn exigir_ativa_de(&self, dono: &Pubkey) -> Result<()> {
        require!(self.ativa, EcolLoteError::BalancaInativa);
        require_keys_eq!(self.dono, *dono, EcolLoteError::BalancaDeOutroDono);
        Ok(())
    }
}
