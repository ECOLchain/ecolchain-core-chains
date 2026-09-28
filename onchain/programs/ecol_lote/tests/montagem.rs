//! ADR 0010: lote de origem com ou sem coletor, lote de venda montado, fechado com a pesagem do
//! consolidado (faixa sobre a soma das origens) e desmontável antes da venda.
mod common;

use {
    anchor_lang::prelude::Pubkey,
    common::*,
    ecol_lote::{
        accounts as ctas, error::EcolLoteError as E, instruction as ixs, pesagem::TipoPesagem,
        EstadoLote, OrigemEntrega,
    },
    litesvm::types::TransactionMetadata,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction_error::TransactionError,
};

/// Lote de origem da cooperativa principal com a origem indicada.
fn origem(
    amb: &mut Ambiente,
    id: u64,
    peso_g: u64,
    origem: OrigemEntrega,
) -> Result<TransactionMetadata, TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let mut dados = amb.dados_entrega(id, peso_g, PLASTICO);
    dados.origem = origem;
    dados.origem_ref = h(&format!("recibo-{id}"));
    let alvo = entrega_pda(&coop.pubkey(), id);
    let pesagem = ix_pesagem(
        &balanca,
        TipoPesagem::Entrega,
        &alvo,
        peso_g,
        dados.ts_pesagem,
    );
    let ix = amb.ix_registrar_entrega(&coop.pubkey(), balanca_pda(&balanca.pubkey()), dados);
    amb.enviar_varias(&[pesagem, ix], &[&coop])
}

fn e(amb: &Ambiente, id: u64) -> Pubkey {
    entrega_pda(&amb.cooperativa.pubkey(), id)
}

/// Lote de venda `lote_id` em montagem com as origens `ids` vinculadas.
fn montar(amb: &mut Ambiente, lote_id: u64, ids: &[u64]) -> Pubkey {
    let coop = amb.cooperativa.insecure_clone();
    let dados = amb.dados(lote_id);
    amb.abrir_lote(&coop, dados).unwrap();
    let lote = lote_pda(&coop.pubkey(), lote_id);
    let entregas: Vec<Pubkey> = ids.iter().map(|i| e(amb, *i)).collect();
    amb.add_entregas(&coop, lote, &entregas).unwrap();
    lote
}

fn fechar(
    amb: &mut Ambiente,
    lote: Pubkey,
    peso_g: u64,
) -> Result<TransactionMetadata, TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    amb.fechar_lote(&coop, &balanca, lote, peso_g)
}

fn desmontar(
    amb: &mut Ambiente,
    lote: Pubkey,
    ids: &[u64],
) -> Result<TransactionMetadata, TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    let entregas: Vec<Pubkey> = ids.iter().map(|i| e(amb, *i)).collect();
    amb.desmontar(&coop, lote, &entregas)
}

fn set_tolerancia(
    amb: &mut Ambiente,
    quem: &Keypair,
    perda: u16,
    excesso: u16,
) -> Result<TransactionMetadata, TransactionError> {
    let ix = amb.ix(
        ixs::AdminSetToleranciaMontagem {
            perda_bps: perda,
            excesso_bps: excesso,
        },
        ctas::AdminUpdateGlobal {
            admin: quem.pubkey(),
            config: config_pda(),
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    amb.enviar(ix, &[quem])
}

#[test]
fn origens_sem_coletor_somam_no_peso_sem_coletor() {
    let mut amb = Ambiente::novo();
    origem(&mut amb, 1, 400_000, OrigemEntrega::Coletor).unwrap();
    origem(&mut amb, 2, 350_000, OrigemEntrega::Doacao).unwrap();
    origem(&mut amb, 3, 150_000, OrigemEntrega::TriagemPropria).unwrap();
    origem(&mut amb, 4, 100_000, OrigemEntrega::Compra).unwrap();
    origem(&mut amb, 5, 0, OrigemEntrega::Avulso).unwrap_err();
    origem(&mut amb, 5, 50_000, OrigemEntrega::Avulso).unwrap();

    let coop = amb.cooperativa.pubkey();
    let conta = amb.svm.get_account(&entrega_pda(&coop, 2)).unwrap();
    let doacao = <ecol_lote::state::Entrega as anchor_lang::AccountDeserialize>::try_deserialize(
        &mut conta.data.as_slice(),
    )
    .unwrap();
    assert_eq!(
        (doacao.origem, doacao.origem_ref),
        (OrigemEntrega::Doacao, h("recibo-2"))
    );

    let lote = montar(&mut amb, 10, &[1, 2, 3, 4, 5]);
    let l = amb.lote(lote);
    assert_eq!(
        (l.qtd_entregas, l.peso_entregas_g, l.peso_sem_coletor_g),
        (5, 1_050_000, 650_000)
    );
}

#[test]
fn lote_so_de_origens_sem_coletor_vai_a_leilao() {
    let mut amb = Ambiente::novo();
    origem(&mut amb, 1, 500_000, OrigemEntrega::Doacao).unwrap();
    let lote = montar(&mut amb, 10, &[1]);
    fechar(&mut amb, lote, 500_000).unwrap();
    amb.anunciar(lote).unwrap();
    assert_eq!(amb.lote(lote).peso_sem_coletor_g, 500_000);
}

#[test]
fn faixa_do_consolidado_padrao_menos_10_mais_2() {
    let mut amb = Ambiente::novo();
    for (i, id) in (1..=4u64).enumerate() {
        origem(&mut amb, id, 1_000_000, OrigemEntrega::Coletor).unwrap();
        montar(&mut amb, 10 + i as u64, &[id]);
    }
    let lote = |amb: &Ambiente, id: u64| lote_pda(&amb.cooperativa.pubkey(), id);
    // Soma de 1.000 kg: aceito de 900 kg a 1.020 kg, inclusive.
    let (l10, l11, l12, l13) = (
        lote(&amb, 10),
        lote(&amb, 11),
        lote(&amb, 12),
        lote(&amb, 13),
    );
    assert_erro(
        fechar(&mut amb, l10, 899_999),
        E::PesoConsolidadoForaDaFaixa,
    );
    assert_erro(
        fechar(&mut amb, l11, 1_020_001),
        E::PesoConsolidadoForaDaFaixa,
    );
    fechar(&mut amb, l10, 900_000).unwrap();
    fechar(&mut amb, l11, 1_020_000).unwrap();
    // O config muda a faixa (5% / 5%).
    let admin = amb.admin.insecure_clone();
    set_tolerancia(&mut amb, &admin, 500, 500).unwrap();
    assert_eq!(
        (
            amb.config().perda_montagem_bps,
            amb.config().excesso_montagem_bps
        ),
        (500, 500)
    );
    assert_erro(
        fechar(&mut amb, l12, 949_999),
        E::PesoConsolidadoForaDaFaixa,
    );
    fechar(&mut amb, l12, 1_050_000).unwrap();
    fechar(&mut amb, l13, 950_000).unwrap();
}

#[test]
fn tolerancia_de_montagem_so_pelo_admin_e_com_limites() {
    let mut amb = Ambiente::novo();
    let operador = amb.operador.insecure_clone();
    assert_erro(set_tolerancia(&mut amb, &operador, 500, 500), E::NaoEhAdmin);
    let admin = amb.admin.insecure_clone();
    for (perda, excesso) in [(0, 200), (1_000, 0), (3_001, 200), (1_000, 3_001)] {
        assert_erro(
            set_tolerancia(&mut amb, &admin, perda, excesso),
            E::ParametroInvalido,
        );
    }
    set_tolerancia(&mut amb, &admin, 3_000, 3_000).unwrap();
}

#[test]
fn desmontar_em_montagem_devolve_as_origens_em_etapas() {
    let mut amb = Ambiente::novo();
    origem(&mut amb, 1, 300_000, OrigemEntrega::Coletor).unwrap();
    origem(&mut amb, 2, 200_000, OrigemEntrega::Doacao).unwrap();
    origem(&mut amb, 3, 100_000, OrigemEntrega::Coletor).unwrap();
    let lote = montar(&mut amb, 10, &[1, 2, 3]);

    desmontar(&mut amb, lote, &[2]).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::EmDesmontagem);
    assert_eq!(
        (l.qtd_entregas, l.peso_entregas_g, l.peso_sem_coletor_g),
        (2, 400_000, 0)
    );
    // Em desmontagem não recebe origens nem fecha.
    let coop = amb.cooperativa.insecure_clone();
    let dois = e(&amb, 2);
    assert_erro(amb.add_entregas(&coop, lote, &[dois]), E::TransicaoInvalida);
    assert_erro(fechar(&mut amb, lote, 400_000), E::TransicaoInvalida);

    desmontar(&mut amb, lote, &[1, 3]).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Desfeito);
    // As origens voltam a ficar disponíveis e entram em outro lote.
    let outro = montar(&mut amb, 11, &[1, 2, 3]);
    assert_eq!(amb.lote(outro).qtd_entregas, 3);
    // Desfeito é terminal.
    assert_erro(desmontar(&mut amb, lote, &[]), E::TransicaoInvalida);
}

#[test]
fn desmontar_lote_fechado_queima_o_asset() {
    let mut amb = Ambiente::novo();
    amb.criar_lote(1).unwrap();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    let asset = asset_pda(&lote);
    assert!(amb
        .svm
        .get_account(&asset)
        .is_some_and(|c| c.data.len() > 1));

    desmontar(&mut amb, lote, &[1]).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Desfeito);
    // Core deixa só o marcador de conta queimada (1 byte) ou nada.
    assert!(amb
        .svm
        .get_account(&asset)
        .is_none_or(|c| c.data.len() <= 1));
}

#[test]
fn desmontar_so_antes_da_venda_e_fora_do_anuncio() {
    let mut amb = Ambiente::novo();
    amb.criar_lote(1).unwrap();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.anunciar(lote).unwrap();
    assert_erro(desmontar(&mut amb, lote, &[1]), E::TransicaoInvalida);
    // Leilão encerrado sem lance: pode desfazer.
    amb.set_tempo(T0 + 3600);
    amb.encerrar_sem_lance(lote).unwrap();
    desmontar(&mut amb, lote, &[1]).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Desfeito);

    // Vendido: não.
    amb.set_tempo(T0);
    amb.criar_lote(2).unwrap();
    let vendido = lote_pda(&amb.cooperativa.pubkey(), 2);
    amb.anunciar(vendido).unwrap();
    amb.vender(vendido).unwrap();
    assert_erro(desmontar(&mut amb, vendido, &[2]), E::TransicaoInvalida);
}

#[test]
fn desmontar_confere_as_origens_e_a_cooperativa() {
    let mut amb = Ambiente::novo();
    origem(&mut amb, 1, 100_000, OrigemEntrega::Coletor).unwrap();
    origem(&mut amb, 2, 100_000, OrigemEntrega::Coletor).unwrap();
    let a = montar(&mut amb, 10, &[1]);
    montar(&mut amb, 11, &[2]);
    // Origem vinculada a outro lote.
    assert_erro(desmontar(&mut amb, a, &[2]), E::EntregaDeOutroLote);
    // Sem origens na transação, com o lote ainda cheio.
    assert_erro(desmontar(&mut amb, a, &[]), E::QuantidadeDeEntregasInvalida);
    // Outra cooperativa não desmonta.
    let outra = Keypair::new();
    amb.svm.airdrop(&outra.pubkey(), 1_000_000_000).unwrap();
    let um = e(&amb, 1);
    assert_erro(amb.desmontar(&outra, a, &[um]), E::NaoEhCooperativaDoLote);
}
