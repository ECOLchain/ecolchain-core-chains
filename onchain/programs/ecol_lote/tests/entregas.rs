//! Entregas de coletores (pesadas pela balança da cooperativa) e montagem do lote.
mod common;

use {
    anchor_lang::prelude::Pubkey,
    common::*,
    ecol_lote::{error::EcolLoteError as E, pesagem::TipoPesagem, Entrega, Papel},
    litesvm::types::TransactionMetadata,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction_error::TransactionError,
};

const PAPEL_MATERIAL: u16 = 2;

fn entrega(amb: &Ambiente, id: u64) -> Entrega {
    use anchor_lang::AccountDeserialize;
    let conta = amb
        .svm
        .get_account(&entrega_pda(&amb.cooperativa.pubkey(), id))
        .unwrap();
    Entrega::try_deserialize(&mut conta.data.as_slice()).unwrap()
}

fn registrar(
    amb: &mut Ambiente,
    id: u64,
    peso_g: u64,
    material: u16,
) -> Result<TransactionMetadata, TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    amb.registrar_entrega(&coop, &balanca, id, peso_g, material)
}

/// Lote criado com a pesagem do fardo, sem nenhuma entrega vinculada ainda.
fn lote_vazio(amb: &mut Ambiente, lote_id: u64, peso_g: u64) -> Pubkey {
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let dados = amb.dados(lote_id, peso_g);
    let lote = lote_pda(&coop.pubkey(), lote_id);
    let pesagem = ix_pesagem(
        &balanca,
        TipoPesagem::Origem,
        &lote,
        peso_g,
        dados.ts_pesagem,
    );
    let criar = amb.ix_criar_lote(&coop.pubkey(), balanca_pda(&balanca.pubkey()), dados);
    amb.enviar_varias(&[pesagem, criar], &[&coop]).unwrap();
    lote
}

fn vincular(
    amb: &mut Ambiente,
    lote: Pubkey,
    ids: &[u64],
) -> Result<TransactionMetadata, TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    let entregas: Vec<Pubkey> = ids
        .iter()
        .map(|i| entrega_pda(&coop.pubkey(), *i))
        .collect();
    amb.add_entregas(&coop, lote, &entregas)
}

#[test]
fn entrega_registrada_com_a_pesagem_da_balanca() {
    let mut amb = Ambiente::novo();
    registrar(&mut amb, 1, 250_000, PLASTICO).unwrap();
    let e = entrega(&amb, 1);
    assert_eq!(
        (e.peso_g, e.material, e.lote),
        (250_000, PLASTICO, Pubkey::default())
    );
    assert_eq!(e.balanca, balanca_pda(&amb.balanca_coop.pubkey()));
    // Mesmo id não registra de novo.
    assert!(registrar(&mut amb, 1, 250_000, PLASTICO).is_err());
}

#[test]
fn pesagem_da_entrega_precisa_ser_do_tipo_entrega_e_da_balanca_certa() {
    let mut amb = Ambiente::novo();
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let dados = amb.dados_entrega(1, 100_000, PLASTICO);
    let alvo = entrega_pda(&coop.pubkey(), 1);

    // Assinatura de origem (lote) reaproveitada numa entrega: recusada.
    let errada = ix_pesagem(
        &balanca,
        TipoPesagem::Origem,
        &alvo,
        100_000,
        dados.ts_pesagem,
    );
    let ix = amb.ix_registrar_entrega(
        &coop.pubkey(),
        balanca_pda(&balanca.pubkey()),
        dados.clone(),
    );
    assert_erro(
        amb.enviar_varias(&[errada, ix], &[&coop]),
        E::PesagemDivergente,
    );

    // Balança da indústria não pesa entrega da cooperativa.
    let ind_balanca = amb.balanca_ind.insecure_clone();
    let pesagem = ix_pesagem(
        &ind_balanca,
        TipoPesagem::Entrega,
        &alvo,
        100_000,
        dados.ts_pesagem,
    );
    let ix = amb.ix_registrar_entrega(&coop.pubkey(), balanca_pda(&ind_balanca.pubkey()), dados);
    assert_erro(
        amb.enviar_varias(&[pesagem, ix], &[&coop]),
        E::BalancaDeOutroDono,
    );
}

#[test]
fn entrega_exige_cooperativa_material_ativo_e_coletor() {
    let mut amb = Ambiente::novo();
    // Indústria não registra entrega.
    let ind = amb.industria.insecure_clone();
    let bi = amb.balanca_ind.insecure_clone();
    assert_erro(
        amb.registrar_entrega(&ind, &bi, 1, 100_000, PLASTICO),
        E::PapelIncorreto,
    );

    amb.atualizar_material(PLASTICO, "Plástico", false).unwrap();
    assert_erro(
        registrar(&mut amb, 2, 100_000, PLASTICO),
        E::MaterialInativo,
    );
    amb.atualizar_material(PLASTICO, "Plástico", true).unwrap();

    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let mut dados = amb.dados_entrega(3, 100_000, PLASTICO);
    dados.coletor_ref = [0; 32];
    let alvo = entrega_pda(&coop.pubkey(), 3);
    let pesagem = ix_pesagem(
        &balanca,
        TipoPesagem::Entrega,
        &alvo,
        100_000,
        dados.ts_pesagem,
    );
    let ix = amb.ix_registrar_entrega(&coop.pubkey(), balanca_pda(&balanca.pubkey()), dados);
    assert_erro(
        amb.enviar_varias(&[pesagem, ix], &[&coop]),
        E::ReferenciaVazia,
    );
}

#[test]
fn montagem_soma_as_entregas_e_encadeia_o_hash() {
    let mut amb = Ambiente::novo();
    for (id, peso) in [(1, 300_000), (2, 450_000), (3, 250_000)] {
        registrar(&mut amb, id, peso, PLASTICO).unwrap();
    }
    let lote = lote_vazio(&mut amb, 10, 990_000);
    vincular(&mut amb, lote, &[1, 2]).unwrap();
    vincular(&mut amb, lote, &[3]).unwrap();

    let l = amb.lote(lote);
    assert_eq!(
        (l.qtd_entregas, l.peso_entregas_g, l.peso_g),
        (3, 1_000_000, 990_000)
    );
    // Auditor reconstrói o hash a partir das entregas, na ordem de vinculação.
    let mut h = [0u8; 32];
    for (id, peso) in [(1u64, 300_000u64), (2, 450_000), (3, 250_000)] {
        let e = entrega_pda(&amb.cooperativa.pubkey(), id);
        h = solana_sha256_hasher::hashv(&[&h, e.as_ref(), &peso.to_le_bytes()]).to_bytes();
    }
    assert_eq!(l.entregas_hash, h);
    assert_eq!(entrega(&amb, 2).lote, lote);
    amb.anunciar(lote).unwrap();
}

#[test]
fn entrega_entra_em_um_lote_so_e_do_mesmo_material() {
    let mut amb = Ambiente::novo();
    amb.criar_material(PAPEL_MATERIAL, "Papel").unwrap();
    registrar(&mut amb, 1, 100_000, PLASTICO).unwrap();
    registrar(&mut amb, 2, 100_000, PAPEL_MATERIAL).unwrap();
    let a = lote_vazio(&mut amb, 10, 100_000);
    let b = lote_vazio(&mut amb, 11, 100_000);
    vincular(&mut amb, a, &[1]).unwrap();
    assert_erro(vincular(&mut amb, b, &[1]), E::EntregaJaVinculada);
    assert_erro(vincular(&mut amb, b, &[2]), E::MaterialDiferente);
}

#[test]
fn entrega_de_outra_cooperativa_e_lote_de_outra_cooperativa() {
    let mut amb = Ambiente::novo();
    let outra = Keypair::new();
    let outra_balanca = Keypair::new();
    amb.svm.airdrop(&outra.pubkey(), 1_000_000_000).unwrap();
    amb.registrar(outra.pubkey(), Papel::Cooperativa).unwrap();
    amb.registrar_balanca(outra.pubkey(), outra_balanca.pubkey())
        .unwrap();
    amb.registrar_entrega(&outra, &outra_balanca, 1, 100_000, PLASTICO)
        .unwrap();

    let lote = lote_vazio(&mut amb, 10, 100_000);
    let coop = amb.cooperativa.insecure_clone();
    let alheia = entrega_pda(&outra.pubkey(), 1);
    assert_erro(
        amb.add_entregas(&coop, lote, &[alheia]),
        E::EntregaDeOutraCooperativa,
    );
    // A outra cooperativa não mexe no lote desta.
    assert_erro(
        amb.add_entregas(&outra, lote, &[alheia]),
        E::NaoEhCooperativaDoLote,
    );
}

#[test]
fn lote_sem_entregas_nao_vai_a_leilao_e_montagem_so_antes_do_anuncio() {
    let mut amb = Ambiente::novo();
    let lote = lote_vazio(&mut amb, 10, 100_000);
    assert_erro(amb.anunciar(lote), E::LoteSemEntregas);

    registrar(&mut amb, 1, 100_000, PLASTICO).unwrap();
    registrar(&mut amb, 2, 100_000, PLASTICO).unwrap();
    vincular(&mut amb, lote, &[1]).unwrap();
    amb.anunciar(lote).unwrap();
    assert_erro(vincular(&mut amb, lote, &[2]), E::TransicaoInvalida);
}

#[test]
fn quantidade_de_entregas_por_transacao() {
    let mut amb = Ambiente::novo();
    let lote = lote_vazio(&mut amb, 10, 1_100_000);
    assert_erro(
        vincular(&mut amb, lote, &[]),
        E::QuantidadeDeEntregasInvalida,
    );
    for id in 1..=11 {
        registrar(&mut amb, id, 100_000, PLASTICO).unwrap();
    }
    let onze: Vec<u64> = (1..=11).collect();
    assert_erro(
        vincular(&mut amb, lote, &onze),
        E::QuantidadeDeEntregasInvalida,
    );
    vincular(&mut amb, lote, &onze[..10]).unwrap();
    assert_eq!(amb.lote(lote).qtd_entregas, 10);
}
