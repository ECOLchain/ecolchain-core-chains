//! Cadastro de materiais pelo operador e uso nos lotes.
mod common;

use {
    common::*,
    ecol_lote::{error::EcolLoteError as E, pesagem::TipoPesagem, ModoOperacao},
    litesvm::types::TransactionMetadata,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction_error::TransactionError,
};

/// Cria um lote da cooperativa principal com o material indicado.
fn criar_lote_de(
    amb: &mut Ambiente,
    lote_id: u64,
    material: u16,
) -> Result<TransactionMetadata, TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let mut dados = amb.dados(lote_id, PESO_G);
    dados.material = material;
    let lote = lote_pda(&coop.pubkey(), lote_id);
    let pesagem = ix_pesagem(
        &balanca,
        TipoPesagem::Origem,
        &lote,
        PESO_G,
        dados.ts_pesagem,
    );
    let criar = amb.ix_criar_lote(&coop.pubkey(), balanca_pda(&balanca.pubkey()), dados);
    amb.enviar_varias(&[pesagem, criar], &[&coop])
}

#[test]
fn operador_cadastra_material() {
    let mut amb = Ambiente::novo();
    amb.criar_material(2, "Papelão").unwrap();
    let m = amb.material(2);
    assert_eq!((m.codigo, m.nome.as_str(), m.ativo), (2, "Papelão", true));
    // O código é único.
    assert!(amb.criar_material(2, "Outro nome").is_err());
}

#[test]
fn so_operador_cadastra_material() {
    let mut amb = Ambiente::novo();
    let intruso = Keypair::new();
    assert_erro(
        amb.criar_material_por(&intruso, 3, "Vidro"),
        E::NaoEhOperador,
    );
    let admin = amb.admin.insecure_clone();
    assert_erro(amb.criar_material_por(&admin, 3, "Vidro"), E::NaoEhOperador);
}

#[test]
fn nome_do_material_validado() {
    let mut amb = Ambiente::novo();
    assert_erro(amb.criar_material(3, ""), E::NomeInvalido);
    assert_erro(amb.criar_material(3, "   "), E::NomeInvalido);
    assert_erro(amb.criar_material(3, &"x".repeat(33)), E::NomeInvalido);
    amb.criar_material(3, &"x".repeat(32)).unwrap();
    assert_erro(amb.atualizar_material(3, "", true), E::NomeInvalido);
}

#[test]
fn lote_usa_material_cadastrado_e_ativo() {
    let mut amb = Ambiente::novo();
    criar_lote_de(&mut amb, 1, PLASTICO).unwrap();
    assert_eq!(
        amb.lote(lote_pda(&amb.cooperativa.pubkey(), 1)).material,
        PLASTICO
    );

    // Material inexistente: a conta não existe e a instrução falha.
    assert!(criar_lote_de(&mut amb, 2, 99).is_err());

    // Material desativado não entra em lotes novos; reativado volta a valer.
    amb.atualizar_material(PLASTICO, "Plástico PET", false)
        .unwrap();
    assert_erro(criar_lote_de(&mut amb, 3, PLASTICO), E::MaterialInativo);
    amb.atualizar_material(PLASTICO, "Plástico PET", true)
        .unwrap();
    criar_lote_de(&mut amb, 4, PLASTICO).unwrap();
    assert_eq!(amb.material(PLASTICO).nome, "Plástico PET");
}

#[test]
fn cadastro_de_material_so_em_modo_normal() {
    let mut amb = Ambiente::novo();
    amb.set_modo(ModoOperacao::Pausado).unwrap();
    assert_erro(amb.criar_material(5, "Alumínio"), E::ProgramaPausado);
}
