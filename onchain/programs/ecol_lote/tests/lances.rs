//! ADR 0013: lances on-chain no leilão. A indústria dá o lance sozinha; depois do prazo, a venda vai ao
//! líder pelo valor do lance.
mod common;

use {
    common::*,
    ecol_lote::{error::EcolLoteError as E, EstadoLote, Papel},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

const MINIMO: u64 = 25_000; // preço mínimo do `anunciar` dos testes
const PRAZO: i64 = T0 + 3600;

/// Lote 1 em leilão e uma segunda indústria cadastrada.
fn leilao(amb: &mut Ambiente) -> (anchor_lang::prelude::Pubkey, Keypair) {
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    let outra = Keypair::new();
    amb.registrar(outra.pubkey(), Papel::Industria).unwrap();
    (lote, outra)
}

fn cancelar(
    amb: &mut Ambiente,
    lote: anchor_lang::prelude::Pubkey,
) -> Result<litesvm::types::TransactionMetadata, solana_transaction_error::TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    let ix = amb.ix_coop_lote(
        &coop.pubkey(),
        lote,
        ecol_lote::instruction::CooperativaCancelAnuncio {},
    );
    amb.enviar(ix, &[&coop])
}

#[test]
fn lance_precisa_do_minimo_e_de_superar_o_maior() {
    let mut amb = Ambiente::novo();
    let (lote, outra) = leilao(&mut amb);
    let ind = amb.industria.insecure_clone();

    assert_erro(
        amb.lance_por(&ind, lote, MINIMO - 1, true),
        E::ValorAbaixoDoMinimo,
    );
    amb.lance_por(&ind, lote, MINIMO, true).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.lance_lider, ind.pubkey());
    assert_eq!(l.maior_lance_centavos, MINIMO);
    assert_eq!(l.qtd_lances, 1);
    let lance = amb.lance(lote, &ind.pubkey()).unwrap();
    assert_eq!(lance.valor_centavos, MINIMO);
    assert_eq!(lance.prazo_leilao, PRAZO);
    assert_eq!(lance.pagador, amb.patrocinador.pubkey());

    // Empatar não basta: precisa superar.
    assert_erro(
        amb.lance_por(&outra, lote, MINIMO, true),
        E::LanceAbaixoDoMaior,
    );
    amb.lance_por(&outra, lote, MINIMO + 500, true).unwrap();
    assert_eq!(amb.lote(lote).lance_lider, outra.pubkey());

    // A primeira indústria aumenta o próprio lance e volta a liderar.
    assert_erro(
        amb.lance_por(&ind, lote, MINIMO + 500, false),
        E::LanceAbaixoDoMaior,
    );
    amb.lance_por(&ind, lote, MINIMO + 1_000, false).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.lance_lider, ind.pubkey());
    assert_eq!(l.maior_lance_centavos, MINIMO + 1_000);
    assert_eq!(l.qtd_lances, 3);
    assert_eq!(
        amb.lance(lote, &ind.pubkey()).unwrap().valor_centavos,
        MINIMO + 1_000
    );
}

#[test]
fn so_industria_e_so_em_leilao_aberto() {
    let mut amb = Ambiente::novo();
    // Lote fechado, ainda não anunciado.
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    let ind = amb.industria.insecure_clone();
    assert_erro(
        amb.lance_por(&ind, lote, MINIMO, true),
        E::LoteNaoEstaEmLeilao,
    );

    // Venda direta não recebe lance.
    amb.anunciar_venda(lote, MINIMO).unwrap();
    assert_erro(
        amb.lance_por(&ind, lote, MINIMO, true),
        E::LoteEmVendaDireta,
    );

    // Carteira de outro papel não dá lance.
    let coop2 = Keypair::new();
    amb.registrar(coop2.pubkey(), Papel::Cooperativa).unwrap();
    let lote2 = lote_pda(&amb.cooperativa.pubkey(), 2);
    amb.criar_lote(2).unwrap();
    amb.anunciar(lote2).unwrap();
    assert!(amb.lance_por(&coop2, lote2, MINIMO, true).is_err());

    // Depois do prazo, não há mais lance.
    amb.set_tempo(PRAZO);
    assert_erro(amb.lance_por(&ind, lote2, MINIMO, true), E::LeilaoEncerrado);
}

#[test]
fn venda_com_lances_so_ao_lider_depois_do_prazo() {
    let mut amb = Ambiente::novo();
    let (lote, outra) = leilao(&mut amb);
    let ind = amb.industria.insecure_clone();
    let inter = amb.intermediador.insecure_clone();
    amb.lance_por(&ind, lote, MINIMO, true).unwrap();
    amb.lance_por(&outra, lote, MINIMO + 2_000, true).unwrap();

    // Antes do prazo, nem ao líder.
    assert_erro(
        amb.vender_com(&outra, &inter, lote, MINIMO + 2_000),
        E::LeilaoEmAndamento,
    );
    amb.set_tempo(PRAZO);
    // Outra indústria, ou outro valor, não.
    assert_erro(
        amb.vender_com(&ind, &inter, lote, MINIMO + 2_000),
        E::NaoEhLanceVencedor,
    );
    assert_erro(
        amb.vender_com(&outra, &inter, lote, MINIMO + 3_000),
        E::NaoEhLanceVencedor,
    );
    // Com lances, o operador não encerra "sem lance".
    assert_erro(amb.encerrar_sem_lance(lote), E::LeilaoComLances);

    amb.vender_com(&outra, &inter, lote, MINIMO + 2_000)
        .unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Vendido);
    assert_eq!(l.industria, outra.pubkey());
    assert_eq!(l.valor_centavos, MINIMO + 2_000);
}

#[test]
fn leilao_sem_lances_on_chain_segue_a_apuracao_off_chain() {
    let mut amb = Ambiente::novo();
    let (lote, _) = leilao(&mut amb);
    // Sem lances on-chain, a venda pode ser registrada antes do prazo, como antes da ADR 0013.
    amb.vender(lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Vendido);
}

#[test]
fn cooperativa_so_retira_leilao_com_lances_depois_do_prazo() {
    let mut amb = Ambiente::novo();
    let (lote, _) = leilao(&mut amb);
    let ind = amb.industria.insecure_clone();
    amb.lance_por(&ind, lote, MINIMO, true).unwrap();
    assert_erro(cancelar(&mut amb, lote), E::LeilaoComLances);

    amb.set_tempo(PRAZO + 10);
    cancelar(&mut amb, lote).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::SemLance);
    assert_eq!(l.qtd_lances, 0);
    assert_eq!(l.maior_lance_centavos, 0);
    assert_eq!(l.lance_lider, anchor_lang::prelude::Pubkey::default());
}

#[test]
fn nova_rodada_zera_os_lances_e_o_lance_antigo_fecha() {
    let mut amb = Ambiente::novo();
    let (lote, _) = leilao(&mut amb);
    let ind = amb.industria.insecure_clone();
    amb.lance_por(&ind, lote, MINIMO + 5_000, true).unwrap();
    // Na mesma rodada, o lance vale e não fecha.
    assert_erro(amb.fechar_lance(lote, &ind.pubkey()), E::LanceEmVigor);

    // A rodada acaba (prazo + retirada) e o lote é reanunciado com outro prazo.
    amb.set_tempo(PRAZO + 10);
    cancelar(&mut amb, lote).unwrap();
    let coop = amb.cooperativa.insecure_clone();
    amb.anunciar_por(&coop, lote, PRAZO + 7200, MINIMO).unwrap();
    assert_eq!(amb.lote(lote).qtd_lances, 0);

    // A conta antiga ainda existe: a indústria "aumenta" para entrar na rodada nova, mesmo com valor
    // menor que o da rodada anterior.
    amb.lance_por(&ind, lote, MINIMO, false).unwrap();
    let lance = amb.lance(lote, &ind.pubkey()).unwrap();
    assert_eq!(lance.prazo_leilao, PRAZO + 7200);
    assert_eq!(amb.lote(lote).maior_lance_centavos, MINIMO);

    // Vendido ao líder, o lance fecha e o rent volta a quem pagou.
    amb.set_tempo(PRAZO + 7200);
    let inter = amb.intermediador.insecure_clone();
    amb.vender_com(&ind, &inter, lote, MINIMO).unwrap();
    let antes = amb.svm.get_balance(&amb.patrocinador.pubkey()).unwrap();
    amb.fechar_lance(lote, &ind.pubkey()).unwrap();
    assert!(amb.lance(lote, &ind.pubkey()).is_none());
    assert!(amb.svm.get_balance(&amb.patrocinador.pubkey()).unwrap() > antes);
}

#[test]
fn venda_direta_zera_lances_de_um_leilao_retirado() {
    let mut amb = Ambiente::novo();
    let (lote, _) = leilao(&mut amb);
    let ind = amb.industria.insecure_clone();
    amb.lance_por(&ind, lote, MINIMO, true).unwrap();
    amb.set_tempo(PRAZO);
    cancelar(&mut amb, lote).unwrap();
    amb.anunciar_venda(lote, MINIMO * 2).unwrap();
    let l = amb.lote(lote);
    assert!(l.venda_direta);
    assert_eq!(l.qtd_lances, 0);
    // O lance do leilão retirado já pode ser fechado.
    amb.fechar_lance(lote, &ind.pubkey()).unwrap();
}
