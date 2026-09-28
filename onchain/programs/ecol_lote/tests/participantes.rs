//! Nome de exibição dos participantes (cadastro e correção pelo operador).
mod common;

use {
    common::*,
    ecol_lote::{error::EcolLoteError as E, Papel},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

/// Lê o nome gravado no campo de tamanho fixo (UTF-8 completado com zeros).
fn nome(amb: &Ambiente, carteira: &Keypair) -> String {
    let campo = amb.participante(&carteira.pubkey()).nome;
    let fim = campo.iter().position(|&b| b == 0).unwrap_or(campo.len());
    String::from_utf8(campo[..fim].to_vec()).unwrap()
}

#[test]
fn cadastro_grava_o_nome() {
    let mut amb = Ambiente::novo();
    let coop = Keypair::new();
    amb.registrar_com_nome(coop.pubkey(), Papel::Cooperativa, "Cooperativa 1")
        .unwrap();
    assert_eq!(nome(&amb, &coop), "Cooperativa 1");
}

#[test]
fn nome_validado_no_cadastro() {
    let mut amb = Ambiente::novo();
    let c = Keypair::new();
    assert_erro(
        amb.registrar_com_nome(c.pubkey(), Papel::Coletor, "  "),
        E::NomeInvalido,
    );
    // 33 bytes: passa do limite.
    let longo = "a".repeat(33);
    assert_erro(
        amb.registrar_com_nome(c.pubkey(), Papel::Coletor, &longo),
        E::NomeInvalido,
    );
    assert_erro(
        amb.registrar_com_nome(c.pubkey(), Papel::Coletor, "a\0b"),
        E::NomeInvalido,
    );
    // Exatamente 32 bytes, com acentos (UTF-8 multibyte).
    let limite = "Cooperativa São João ÇÃÕ 12";
    assert!(limite.len() <= 32);
    amb.registrar_com_nome(c.pubkey(), Papel::Coletor, limite)
        .unwrap();
    assert_eq!(nome(&amb, &c), limite);
}

#[test]
fn operador_corrige_o_nome() {
    let mut amb = Ambiente::novo();
    let coop = Keypair::new();
    amb.registrar_com_nome(coop.pubkey(), Papel::Cooperativa, "Coop Um com nome longo")
        .unwrap();
    let operador = amb.operador.insecure_clone();
    amb.set_nome_por(&operador, coop.pubkey(), "Cooperativa 1")
        .unwrap();
    // O nome menor não deixa sobra do anterior.
    assert_eq!(nome(&amb, &coop), "Cooperativa 1");
    let p = amb.participante(&coop.pubkey());
    assert_eq!((p.papel, p.ativo), (Papel::Cooperativa, true));
    assert_erro(
        amb.set_nome_por(&operador, coop.pubkey(), ""),
        E::NomeInvalido,
    );
}

#[test]
fn so_operador_corrige_o_nome() {
    let mut amb = Ambiente::novo();
    let coop = Keypair::new();
    amb.registrar_com_nome(coop.pubkey(), Papel::Cooperativa, "Cooperativa 1")
        .unwrap();
    let intruso = Keypair::new();
    assert_erro(
        amb.set_nome_por(&intruso, coop.pubkey(), "Outra"),
        E::NaoEhOperador,
    );
    let admin = amb.admin.insecure_clone();
    assert_erro(
        amb.set_nome_por(&admin, coop.pubkey(), "Outra"),
        E::NaoEhOperador,
    );
}
