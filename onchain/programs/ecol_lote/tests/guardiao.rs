//! Guardião: qualquer membro da EcolChain pausa na hora; só o admin volta ao Normal.
mod common;

use {
    anchor_lang::prelude::Pubkey,
    common::*,
    ecol_lote::{
        accounts as ctas, error::EcolLoteError as E, instruction as ixs, ModoOperacao, Papel,
    },
    litesvm::types::TransactionMetadata,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction_error::TransactionError,
};

fn set_guardiao(
    amb: &mut Ambiente,
    quem: &Keypair,
    guardiao: Pubkey,
) -> Result<TransactionMetadata, TransactionError> {
    let ix = amb.ix(
        ixs::AdminSetGuardiao { guardiao },
        ctas::AdminUpdateGlobal {
            admin: quem.pubkey(),
            config: config_pda(),
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    amb.enviar(ix, &[quem])
}

fn pausar(
    amb: &mut Ambiente,
    quem: &Keypair,
    modo: ModoOperacao,
) -> Result<TransactionMetadata, TransactionError> {
    let ix = amb.ix(
        ixs::GuardiaoPausar { modo },
        ctas::GuardiaoPausar {
            guardiao: quem.pubkey(),
            config: config_pda(),
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    amb.enviar(ix, &[quem])
}

/// Ambiente com guardião definido pelo admin.
fn com_guardiao() -> (Ambiente, Keypair) {
    let mut amb = Ambiente::novo();
    let guardiao = Keypair::new();
    let admin = amb.admin.insecure_clone();
    set_guardiao(&mut amb, &admin, guardiao.pubkey()).unwrap();
    assert_eq!(amb.config().guardiao, guardiao.pubkey());
    (amb, guardiao)
}

#[test]
fn sem_guardiao_ninguem_pausa_por_esse_caminho() {
    let mut amb = Ambiente::novo();
    assert_eq!(amb.config().guardiao, Pubkey::default());
    let qualquer = Keypair::new();
    assert_erro(
        pausar(&mut amb, &qualquer, ModoOperacao::Pausado),
        E::NaoEhGuardiao,
    );
}

#[test]
fn so_admin_define_guardiao_e_nao_pode_ser_o_proprio_admin() {
    let mut amb = Ambiente::novo();
    let intruso = Keypair::new();
    assert_erro(
        set_guardiao(&mut amb, &intruso, intruso.pubkey()),
        E::NaoEhAdmin,
    );
    let admin = amb.admin.insecure_clone();
    assert_erro(
        set_guardiao(&mut amb, &admin, admin.pubkey()),
        E::ChavesDeConfigInvalidas,
    );
}

#[test]
fn guardiao_so_restringe() {
    let (mut amb, guardiao) = com_guardiao();
    let outro = Keypair::new();
    assert_erro(
        pausar(&mut amb, &outro, ModoOperacao::Pausado),
        E::NaoEhGuardiao,
    );

    pausar(&mut amb, &guardiao, ModoOperacao::SomenteDisputas).unwrap();
    assert_eq!(amb.config().modo, ModoOperacao::SomenteDisputas);
    // Mesmo nível ou mais brando: recusado.
    assert_erro(
        pausar(&mut amb, &guardiao, ModoOperacao::SomenteDisputas),
        E::GuardiaoSoRestringe,
    );
    assert_erro(
        pausar(&mut amb, &guardiao, ModoOperacao::Normal),
        E::GuardiaoSoRestringe,
    );

    pausar(&mut amb, &guardiao, ModoOperacao::Pausado).unwrap();
    assert_erro(
        pausar(&mut amb, &guardiao, ModoOperacao::SomenteDisputas),
        E::GuardiaoSoRestringe,
    );
    assert_erro(amb.criar_lote(1), E::ProgramaPausado);

    // Só o admin volta ao Normal.
    amb.set_modo(ModoOperacao::Normal).unwrap();
    amb.criar_lote(1).unwrap();
}

#[test]
fn guardiao_nao_altera_config_nem_vira_participante() {
    let (mut amb, guardiao) = com_guardiao();
    let params = amb.params();
    assert_erro(amb.update_global(&guardiao, params), E::NaoEhAdmin);
    assert_erro(
        amb.registrar(guardiao.pubkey(), Papel::Cooperativa),
        E::CarteiraReservada,
    );
}

#[test]
fn admin_remove_guardiao() {
    let (mut amb, guardiao) = com_guardiao();
    let admin = amb.admin.insecure_clone();
    set_guardiao(&mut amb, &admin, Pubkey::default()).unwrap();
    assert_erro(
        pausar(&mut amb, &guardiao, ModoOperacao::Pausado),
        E::NaoEhGuardiao,
    );
}
