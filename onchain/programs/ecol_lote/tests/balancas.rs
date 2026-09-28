//! Nome de exibição das balanças (cadastro e correção pelo operador).
mod common;

use {
    anchor_lang::AccountDeserialize,
    common::*,
    ecol_lote::{error::EcolLoteError as E, state::Balanca},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

fn nome(amb: &Ambiente, dispositivo: &Keypair) -> String {
    let conta = amb
        .svm
        .get_account(&balanca_pda(&dispositivo.pubkey()))
        .unwrap();
    let campo = Balanca::try_deserialize(&mut conta.data.as_slice())
        .unwrap()
        .nome;
    let fim = campo.iter().position(|&b| b == 0).unwrap_or(campo.len());
    String::from_utf8(campo[..fim].to_vec()).unwrap()
}

#[test]
fn cadastro_grava_o_nome_da_balanca() {
    let mut amb = Ambiente::novo();
    let disp = Keypair::new();
    let coop = amb.cooperativa.pubkey();
    amb.registrar_balanca_com_nome(coop, disp.pubkey(), "Balança do galpão 2")
        .unwrap();
    assert_eq!(nome(&amb, &disp), "Balança do galpão 2");
}

#[test]
fn nome_da_balanca_validado() {
    let mut amb = Ambiente::novo();
    let disp = Keypair::new();
    let coop = amb.cooperativa.pubkey();
    assert_erro(
        amb.registrar_balanca_com_nome(coop, disp.pubkey(), ""),
        E::NomeInvalido,
    );
    assert_erro(
        amb.registrar_balanca_com_nome(coop, disp.pubkey(), &"b".repeat(33)),
        E::NomeInvalido,
    );
}

#[test]
fn operador_corrige_o_nome_da_balanca() {
    let mut amb = Ambiente::novo();
    let disp = Keypair::new();
    let coop = amb.cooperativa.pubkey();
    amb.registrar_balanca_com_nome(coop, disp.pubkey(), "Balança antiga do pátio")
        .unwrap();
    let operador = amb.operador.insecure_clone();
    amb.set_balanca_nome_por(&operador, disp.pubkey(), "Pátio")
        .unwrap();
    assert_eq!(nome(&amb, &disp), "Pátio");
    let intruso = Keypair::new();
    assert_erro(
        amb.set_balanca_nome_por(&intruso, disp.pubkey(), "X"),
        E::NaoEhOperador,
    );
}
