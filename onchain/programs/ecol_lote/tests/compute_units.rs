//! Orçamento de compute units das instruções mais pesadas (Ed25519 + CPI ao Metaplex Core).
//! Falha se alguma passar do limite padrão de 200k CU por instrução, com folga.
mod common;

use {common::*, ecol_lote::pesagem::TipoPesagem, solana_signer::Signer};

const LIMITE_CU: u64 = 150_000;

#[test]
fn orcamento_de_compute_units() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    let mut medidas = vec![];

    let r = amb.criar_lote(1).unwrap();
    medidas.push(("cooperativa_create_lote (Ed25519 + Core create)", r.compute_units_consumed));
    medidas.push(("cooperativa_list_lote", amb.anunciar(lote).unwrap().compute_units_consumed));
    medidas.push(("industria_accept_venda", amb.vender(lote).unwrap().compute_units_consumed));
    medidas.push(("transportador_pickup_lote (Core transfer)", amb.retirar(lote).unwrap().compute_units_consumed));

    let ind = amb.industria.insecure_clone();
    let pesagem = ix_pesagem(&amb.balanca_ind, TipoPesagem::Recebimento, &lote, PESO_G, T0);
    let ix = amb.ix_receber(&ind.pubkey(), lote, PESO_G, Some(balanca_pda(&amb.balanca_ind.pubkey())), Some(T0));
    let r = amb.enviar_varias(&[pesagem, ix], &[&ind]).unwrap();
    medidas.push(("industria_confirm_recebimento (Ed25519 + Core transfer)", r.compute_units_consumed));
    medidas.push(("intermediador_confirm_liberacao", amb.liberar(lote).unwrap().compute_units_consumed));

    for (nome, cu) in &medidas {
        println!("{cu:>8} CU  {nome}");
        assert!(*cu < LIMITE_CU, "{nome} consumiu {cu} CU");
    }

    // Rent (depósito) das contas criadas por lote.
    for (nome, conta) in [("Lote (PDA)", lote), ("Core asset", asset_pda(&lote))] {
        let a = amb.svm.get_account(&conta).unwrap();
        println!("{:>8} bytes  {:.6} SOL  {nome}", a.data.len(), a.lamports as f64 / 1e9);
    }
}
