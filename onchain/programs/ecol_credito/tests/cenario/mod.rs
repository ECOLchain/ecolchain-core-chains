//! Cenário com `ecol_lote` + `ecol_credito` carregados e configurados (compartilhado pelos testes
//! do `ecol_credito` e do `ecol_reparticao`). Exige `mod common;` (harness do `ecol_lote`) na raiz do teste.
#![allow(dead_code)]

use {
    crate::common::*,
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{
            instruction::{AccountMeta, Instruction},
            system_program,
        },
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    ecol_credito::{
        accounts as ctas, error::EcolCreditoError as C, instruction as ixs, Credito,
        PadraoRegistro, SEED_AUTORIDADE, SEED_CONFIG, SEED_CREDITO, SEED_SERIAL,
    },
    ecol_lote::Material,
    litesvm::types::TransactionMetadata,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction::InstructionError,
    solana_transaction_error::TransactionError,
};

pub fn cpda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ecol_credito::id()).0
}
pub fn config_credito() -> Pubkey {
    cpda(&[SEED_CONFIG])
}
pub fn credito_pda(id: u64) -> Pubkey {
    cpda(&[SEED_CREDITO, &id.to_le_bytes()])
}
pub fn autoridade() -> Pubkey {
    cpda(&[SEED_AUTORIDADE])
}
pub fn event_authority_credito() -> Pubkey {
    cpda(&[b"__event_authority"])
}
pub fn sha256(b: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[b]).to_bytes()
}
pub fn serial_pda(padrao: PadraoRegistro, serial: &str) -> Pubkey {
    cpda(&[SEED_SERIAL, &[padrao as u8], &sha256(serial.as_bytes())])
}

#[track_caller]
pub fn assert_erro_credito(res: Result<TransactionMetadata, TransactionError>, e: C) {
    let esperado: u32 = e.into();
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(c, esperado)
        }
        outro => panic!("esperava {esperado}, veio {outro:?}"),
    }
}

pub struct Cenario {
    pub amb: Ambiente,
    pub registrador: Keypair,
    pub comprador: Keypair,
}

impl Cenario {
    pub fn novo() -> Self {
        let mut amb = Ambiente::novo();
        let bytes = include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/ecol_credito.so"
        ));
        amb.svm.add_program(ecol_credito::id(), bytes).unwrap();
        // Upgrade authority no ProgramData (o LiteSVM cria vazia).
        let pd = Pubkey::find_program_address(
            &[ecol_credito::id().as_ref()],
            &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
        )
        .0;
        let mut conta = amb.svm.get_account(&pd).unwrap();
        conta.data[12] = 1;
        conta.data[13..45].copy_from_slice(amb.upgrade_authority.pubkey().as_ref());
        amb.svm.set_account(pd, conta).unwrap();

        // O ecol_lote passa a aceitar a PDA do ecol_credito como autoridade de agregação.
        let admin = amb.admin.insecure_clone();
        let mut p = amb.params();
        p.credito_autoridade = autoridade();
        amb.update_global(&admin, p).unwrap();

        let mut c = Cenario {
            amb,
            registrador: Keypair::new(),
            comprador: Keypair::new(),
        };
        let ua = c.amb.upgrade_authority.insecure_clone();
        c.init(&ua).unwrap();
        c
    }

    pub fn ix(&self, data: impl InstructionData, contas: impl ToAccountMetas) -> Instruction {
        Instruction::new_with_bytes(
            ecol_credito::id(),
            &data.data(),
            contas.to_account_metas(None),
        )
    }

    pub fn init(&mut self, ua: &Keypair) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminInitConfig {
                admin: self.amb.admin.pubkey(),
                operador: self.amb.operador.pubkey(),
                registrador: self.registrador.pubkey(),
                intermediador: self.amb.intermediador.pubkey(),
            },
            ctas::AdminInitConfig {
                payer: self.amb.patrocinador.pubkey(),
                upgrade_authority: ua.pubkey(),
                config: config_credito(),
                este_programa: ecol_credito::id(),
                program_data: Pubkey::find_program_address(
                    &[ecol_credito::id().as_ref()],
                    &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
                )
                .0,
                system_program: system_program::ID,
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[ua])
    }

    pub fn criar_credito_por(
        &mut self,
        operador: &Keypair,
        id: u64,
        material: Material,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorCreateCredito {
                credito_id: id,
                padrao: PadraoRegistro::Verra,
                material,
                vintage: 2026,
                metodologia_hash: h("metodologia-plastico"),
            },
            ctas::OperadorCreateCredito {
                payer: self.amb.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                system_program: system_program::ID,
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[operador])
    }

    pub fn criar_credito(&mut self, id: u64) {
        let op = self.amb.operador.insecure_clone();
        self.criar_credito_por(&op, id, Material::Plastico).unwrap();
    }

    pub fn ix_add_lotes(&self, id: u64, lotes: &[Pubkey]) -> Instruction {
        let mut ix = self.ix(
            ixs::OperadorAddLotes {},
            ctas::OperadorAddLotes {
                operador: self.amb.operador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                autoridade: autoridade(),
                lote_config: config_pda(),
                lote_event_authority: event_authority(),
                ecol_lote_program: ecol_lote::id(),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        ix.accounts
            .extend(lotes.iter().map(|l| AccountMeta::new(*l, false)));
        ix
    }

    pub fn add_lotes(
        &mut self,
        id: u64,
        lotes: &[Pubkey],
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix_add_lotes(id, lotes);
        let op = self.amb.operador.insecure_clone();
        self.amb.enviar(ix, &[&op])
    }

    pub fn submeter(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let op = self.amb.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorSubmitCredito {},
            ctas::OperadorSubmitCredito {
                operador: op.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[&op])
    }

    pub fn emitir_com(
        &mut self,
        registrador: &Keypair,
        id: u64,
        serial: &str,
        serial_hash: [u8; 32],
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::RegistradorRecordEmissao {
                serial: serial.to_string(),
                serial_hash,
                tco2e_milli: 1_000,
            },
            ctas::RegistradorRecordEmissao {
                payer: self.amb.patrocinador.pubkey(),
                registrador: registrador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                serial_registro: cpda(&[SEED_SERIAL, &[PadraoRegistro::Verra as u8], &serial_hash]),
                system_program: system_program::ID,
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[registrador])
    }

    pub fn emitir(
        &mut self,
        id: u64,
        serial: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let r = self.registrador.insecure_clone();
        self.emitir_com(&r, id, serial, sha256(serial.as_bytes()))
    }

    pub fn contas_registrador(
        &self,
        registrador: &Keypair,
        id: u64,
    ) -> ctas::RegistradorRecordRejeicao {
        ctas::RegistradorRecordRejeicao {
            registrador: registrador.pubkey(),
            config: config_credito(),
            credito: credito_pda(id),
            event_authority: event_authority_credito(),
            program: ecol_credito::id(),
        }
    }

    pub fn rejeitar(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let r = self.registrador.insecure_clone();
        let ix = self.ix(
            ixs::RegistradorRecordRejeicao {
                motivo_hash: h("adicionalidade"),
            },
            self.contas_registrador(&r, id),
        );
        self.amb.enviar(ix, &[&r])
    }

    pub fn aposentar(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let r = self.registrador.insecure_clone();
        let ix = self.ix(
            ixs::RegistradorRecordAposentadoria {
                prova_hash: h("certificado-aposentadoria"),
            },
            self.contas_registrador(&r, id),
        );
        self.amb.enviar(ix, &[&r])
    }

    pub fn vender_com(
        &mut self,
        comprador: &Keypair,
        id: u64,
        valor: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let inter = self.amb.intermediador.insecure_clone();
        self.vender_atestado(comprador, &inter, id, valor)
    }

    pub fn vender_atestado(
        &mut self,
        comprador: &Keypair,
        intermediador: &Keypair,
        id: u64,
        valor: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let op = self.amb.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorRecordVenda {
                valor_centavos: valor,
                pagamento_ref_hash: h("pix-venda-credito"),
            },
            ctas::OperadorRecordVenda {
                operador: op.pubkey(),
                comprador: comprador.pubkey(),
                intermediador: intermediador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[&op, comprador, intermediador])
    }

    pub fn vender(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let c = self.comprador.insecure_clone();
        self.vender_com(&c, id, 5_000)
    }

    pub fn liberar_lotes(
        &mut self,
        id: u64,
        lotes: &[Pubkey],
    ) -> Result<TransactionMetadata, TransactionError> {
        let cranker = Keypair::new();
        let mut ix = self.ix(
            ixs::PublicReleaseLotes {},
            ctas::PublicReleaseLotes {
                cranker: cranker.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                autoridade: autoridade(),
                lote_config: config_pda(),
                lote_event_authority: event_authority(),
                ecol_lote_program: ecol_lote::id(),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        ix.accounts
            .extend(lotes.iter().map(|l| AccountMeta::new(*l, false)));
        self.amb.enviar(ix, &[&cranker])
    }

    pub fn credito(&self, id: u64) -> Credito {
        let conta = self.amb.svm.get_account(&credito_pda(id)).unwrap();
        Credito::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    /// Lote `lote_id` da cooperativa principal até `Reciclado`.
    pub fn lote_reciclado(&mut self, lote_id: u64) -> Pubkey {
        let lote = self.amb.lote_ate_recebido(lote_id);
        self.amb.liberar(lote).unwrap();
        lote
    }
}
