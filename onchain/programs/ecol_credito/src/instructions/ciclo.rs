use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;

use crate::{
    constants::{SEED_CONFIG, SEED_CREDITO, SEED_SERIAL, SERIAL_MAX_LEN},
    error::EcolCreditoError,
    events::{
        CreditoAposentado, CreditoEmitido, CreditoRejeitado, CreditoTransicionado, CreditoVendido,
    },
    state::{Credito, CreditoConfig, PadraoRegistro, SerialRegistro},
};

impl PadraoRegistro {
    pub fn seed(self) -> [u8; 1] {
        [self as u8]
    }
}

/// O registrador atesta a emissão no registro oficial. O serial fica numa PDA própria,
/// então o mesmo crédito do registro não pode ser vinculado a dois créditos EcolChain.
#[event_cpi]
#[derive(Accounts)]
#[instruction(serial: String, serial_hash: [u8; 32])]
pub struct RegistradorRecordEmissao<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub registrador: Signer<'info>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = registrador @ EcolCreditoError::NaoEhRegistrador)]
    pub config: Account<'info, CreditoConfig>,
    #[account(mut, seeds = [SEED_CREDITO, &credito.credito_id.to_le_bytes()], bump = credito.bump)]
    pub credito: Account<'info, Credito>,
    #[account(
        init,
        payer = payer,
        space = 8 + SerialRegistro::INIT_SPACE,
        seeds = [SEED_SERIAL, credito.padrao.seed().as_ref(), serial_hash.as_ref()],
        bump
    )]
    pub serial_registro: Account<'info, SerialRegistro>,
    pub system_program: Program<'info, System>,
}

pub fn handle_registrador_record_emissao(
    ctx: Context<RegistradorRecordEmissao>,
    serial: String,
    serial_hash: [u8; 32],
    tco2e_milli: u64,
) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    require!(
        !serial.is_empty() && serial.len() <= SERIAL_MAX_LEN,
        EcolCreditoError::SerialInvalido
    );
    // O hash nas seeds tem de ser o do serial gravado.
    require!(
        hashv(&[serial.as_bytes()]).to_bytes() == serial_hash,
        EcolCreditoError::SerialInvalido
    );
    require!(tco2e_milli > 0, EcolCreditoError::ParametroInvalido);

    let agora = Clock::get()?.unix_timestamp;
    let credito_key = ctx.accounts.credito.key();
    let ator = ctx.accounts.registrador.key();
    ctx.accounts.serial_registro.credito = credito_key;
    ctx.accounts.serial_registro.bump = ctx.bumps.serial_registro;

    let credito = &mut ctx.accounts.credito;
    let para = credito.estado.emitir()?;
    credito.serial = serial.clone();
    credito.tco2e_milli = tco2e_milli;
    let (de, seq) = credito.transicionar(para, agora)?;
    let seq_emissao = credito.proximo_seq()?;
    emit_cpi!(CreditoTransicionado {
        credito: credito_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
    emit_cpi!(CreditoEmitido {
        credito: credito_key,
        seq: seq_emissao,
        serial,
        tco2e_milli
    });
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct RegistradorRecordRejeicao<'info> {
    pub registrador: Signer<'info>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = registrador @ EcolCreditoError::NaoEhRegistrador)]
    pub config: Account<'info, CreditoConfig>,
    #[account(mut, seeds = [SEED_CREDITO, &credito.credito_id.to_le_bytes()], bump = credito.bump)]
    pub credito: Account<'info, Credito>,
}

/// Validação negada: o crédito não é emitido e não há repartição. Os lotes continuam `Agregado`
/// (não podem ser reaproveitados em outro crédito — escolha conservadora contra dupla contagem).
pub fn handle_registrador_record_rejeicao(
    ctx: Context<RegistradorRecordRejeicao>,
    motivo_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    require!(motivo_hash != [0; 32], EcolCreditoError::ReferenciaVazia);
    let agora = Clock::get()?.unix_timestamp;
    let credito_key = ctx.accounts.credito.key();
    let ator = ctx.accounts.registrador.key();
    let credito = &mut ctx.accounts.credito;
    let para = credito.estado.rejeitar()?;
    let (de, seq) = credito.transicionar(para, agora)?;
    let seq_rej = credito.proximo_seq()?;
    emit_cpi!(CreditoTransicionado {
        credito: credito_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
    emit_cpi!(CreditoRejeitado {
        credito: credito_key,
        seq: seq_rej,
        motivo_hash
    });
    Ok(())
}

/// Venda registrada com a assinatura do comprador (carteira embutida) e do operador.
/// O valor é a base da repartição (`ecol_reparticao`), por isso não fica só com a EcolChain.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorRecordVenda<'info> {
    pub operador: Signer<'info>,
    pub comprador: Signer<'info>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = operador @ EcolCreditoError::NaoEhOperador)]
    pub config: Account<'info, CreditoConfig>,
    #[account(mut, seeds = [SEED_CREDITO, &credito.credito_id.to_le_bytes()], bump = credito.bump)]
    pub credito: Account<'info, Credito>,
}

pub fn handle_operador_record_venda(
    ctx: Context<OperadorRecordVenda>,
    valor_centavos: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_ativo()?;
    require!(valor_centavos > 0, EcolCreditoError::ParametroInvalido);
    let comprador = ctx.accounts.comprador.key();
    require!(
        comprador != config.admin && comprador != config.registrador,
        EcolCreditoError::ChavesDeConfigInvalidas
    );
    let agora = Clock::get()?.unix_timestamp;
    let credito_key = ctx.accounts.credito.key();
    let ator = ctx.accounts.operador.key();
    let credito = &mut ctx.accounts.credito;
    let para = credito.estado.vender()?;
    credito.comprador = comprador;
    credito.valor_venda_centavos = valor_centavos;
    let (de, seq) = credito.transicionar(para, agora)?;
    let seq_venda = credito.proximo_seq()?;
    emit_cpi!(CreditoTransicionado {
        credito: credito_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
    emit_cpi!(CreditoVendido {
        credito: credito_key,
        seq: seq_venda,
        comprador,
        valor_centavos
    });
    Ok(())
}

/// O registrador atesta a aposentadoria no registro em nome do comprador.
pub fn handle_registrador_record_aposentadoria(
    ctx: Context<RegistradorRecordRejeicao>,
    prova_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    require!(prova_hash != [0; 32], EcolCreditoError::ReferenciaVazia);
    let agora = Clock::get()?.unix_timestamp;
    let credito_key = ctx.accounts.credito.key();
    let ator = ctx.accounts.registrador.key();
    let credito = &mut ctx.accounts.credito;
    let para = credito.estado.aposentar()?;
    credito.prova_aposentadoria_hash = prova_hash;
    let (de, seq) = credito.transicionar(para, agora)?;
    let seq_ap = credito.proximo_seq()?;
    emit_cpi!(CreditoTransicionado {
        credito: credito_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
    emit_cpi!(CreditoAposentado {
        credito: credito_key,
        seq: seq_ap,
        prova_hash
    });
    Ok(())
}
