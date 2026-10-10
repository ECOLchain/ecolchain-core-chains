//! Número do MTR (Manifesto de Transporte de Resíduos) informado por quem retira o lote.
mod common;

use {
    common::*,
    ecol_lote::{error::EcolLoteError as E, EstadoLote},
    solana_signer::Signer,
};

/// Lote 1 vendido, esperando a retirada.
fn lote_vendido(amb: &mut Ambiente) -> anchor_lang::prelude::Pubkey {
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    lote
}

#[test]
fn lote_vendido_sem_mtr_e_retirada_grava_o_numero() {
    let mut amb = Ambiente::novo();
    let lote = lote_vendido(&mut amb);
    assert_eq!(amb.lote(lote).mtr, 0);
    let (t, c) = (amb.transportador.insecure_clone(), amb.cooperativa.insecure_clone());
    amb.retirar_com_mtr(&t, &c, lote, 987_654).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.mtr, 987_654);
    assert_eq!(l.estado, EstadoLote::EmTransporte);
}

#[test]
fn mtr_fora_da_faixa_falha() {
    let mut amb = Ambiente::novo();
    let lote = lote_vendido(&mut amb);
    let (t, c) = (amb.transportador.insecure_clone(), amb.cooperativa.insecure_clone());
    assert_erro(amb.retirar_com_mtr(&t, &c, lote, 0), E::MtrInvalido);
    assert_erro(amb.retirar_com_mtr(&t, &c, lote, 1_000_000), E::MtrInvalido);
    // Os limites valem: um dígito e seis dígitos.
    amb.retirar_com_mtr(&t, &c, lote, 999_999).unwrap();
    assert_eq!(amb.lote(lote).mtr, 999_999);
}

#[test]
fn mtr_de_um_digito_vale() {
    let mut amb = Ambiente::novo();
    let lote = lote_vendido(&mut amb);
    let (t, c) = (amb.transportador.insecure_clone(), amb.cooperativa.insecure_clone());
    amb.retirar_com_mtr(&t, &c, lote, 1).unwrap();
    assert_eq!(amb.lote(lote).mtr, 1);
}
