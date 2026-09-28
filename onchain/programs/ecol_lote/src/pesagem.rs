//! Atestação de pesagem: a balança assina `(domínio, tipo, lote, peso_g, ts)` com Ed25519 e o
//! cliente coloca a instrução do programa Ed25519 imediatamente antes da instrução do `ecol_lote`.

use anchor_lang::prelude::*;
use solana_instructions_sysvar::get_instruction_relative;

use crate::{
    constants::{DOMINIO_PESAGEM, JANELA_PESAGEM_S},
    error::EcolLoteError,
};

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum TipoPesagem {
    /// Pesagem na cooperativa, que cria o lote.
    Origem = 0,
    /// Pesagem na indústria, no recebimento.
    Recebimento = 1,
}

pub const MENSAGEM_LEN: usize = 17 + 1 + 32 + 8 + 8;

pub fn mensagem(tipo: TipoPesagem, lote: &Pubkey, peso_g: u64, ts: i64) -> [u8; MENSAGEM_LEN] {
    let mut m = [0u8; MENSAGEM_LEN];
    m[..17].copy_from_slice(DOMINIO_PESAGEM);
    m[17] = tipo as u8;
    m[18..50].copy_from_slice(lote.as_ref());
    m[50..58].copy_from_slice(&peso_g.to_le_bytes());
    m[58..66].copy_from_slice(&ts.to_le_bytes());
    m
}

// Layout da instrução do programa Ed25519 (uma assinatura):
// [0] num_signatures, [1] padding, depois 7 × u16 LE:
// signature_offset, signature_ix_index, public_key_offset, public_key_ix_index,
// message_offset, message_size, message_ix_index.
const CABECALHO_LEN: usize = 2 + 7 * 2;
/// `ix_index == u16::MAX` significa "dados dentro desta mesma instrução".
const MESMA_INSTRUCAO: u16 = u16::MAX;

fn u16_em(dados: &[u8], pos: usize) -> Result<u16> {
    let b = dados
        .get(pos..pos.checked_add(2).ok_or(EcolLoteError::AtestacaoInvalida)?)
        .ok_or(EcolLoteError::AtestacaoInvalida)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn trecho(dados: &[u8], offset: u16, len: usize) -> Result<&[u8]> {
    let inicio = usize::from(offset);
    let fim = inicio
        .checked_add(len)
        .ok_or(EcolLoteError::AtestacaoInvalida)?;
    dados
        .get(inicio..fim)
        .ok_or_else(|| error!(EcolLoteError::AtestacaoInvalida))
}

/// Confere que a instrução anterior (índice relativo -1) é do programa Ed25519, com uma única
/// assinatura, cujos dados estão todos nela mesma, feita por `dispositivo` sobre `esperada`.
/// O runtime já rejeitou a transação se a assinatura fosse inválida; aqui se prova *quem* assinou *o quê*.
pub fn verificar_assinatura_balanca(
    instrucoes_sysvar: &AccountInfo,
    dispositivo: &Pubkey,
    esperada: &[u8],
) -> Result<()> {
    let ix = get_instruction_relative(-1, instrucoes_sysvar)
        .map_err(|_| error!(EcolLoteError::AtestacaoAusente))?;
    require_keys_eq!(
        ix.program_id,
        solana_sdk_ids::ed25519_program::ID,
        EcolLoteError::AtestacaoAusente
    );
    let d = &ix.data;
    require!(d.len() >= CABECALHO_LEN, EcolLoteError::AtestacaoInvalida);
    require!(d[0] == 1, EcolLoteError::AtestacaoInvalida);

    let assinatura_ix = u16_em(d, 4)?;
    let chave_offset = u16_em(d, 6)?;
    let chave_ix = u16_em(d, 8)?;
    let mensagem_offset = u16_em(d, 10)?;
    let mensagem_len = u16_em(d, 12)?;
    let mensagem_ix = u16_em(d, 14)?;
    // Sem isso, a assinatura poderia ser verificada sobre bytes de outra instrução da transação.
    require!(
        assinatura_ix == MESMA_INSTRUCAO
            && chave_ix == MESMA_INSTRUCAO
            && mensagem_ix == MESMA_INSTRUCAO,
        EcolLoteError::AtestacaoInvalida
    );

    let chave = trecho(d, chave_offset, 32)?;
    require!(
        chave == dispositivo.as_ref(),
        EcolLoteError::BalancaIncorreta
    );
    let msg = trecho(d, mensagem_offset, usize::from(mensagem_len))?;
    require!(msg == esperada, EcolLoteError::PesagemDivergente);
    Ok(())
}

pub fn exigir_pesagem_recente(ts: i64, agora: i64) -> Result<()> {
    let idade = agora.checked_sub(ts).ok_or(EcolLoteError::Overflow)?;
    require!(
        (-JANELA_PESAGEM_S..=JANELA_PESAGEM_S).contains(&idade),
        EcolLoteError::PesagemExpirada
    );
    Ok(())
}
