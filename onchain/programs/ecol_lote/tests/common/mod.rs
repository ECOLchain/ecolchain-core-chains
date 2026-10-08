//! Ambiente LiteSVM compartilhado pelos testes do `ecol_lote`.
#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{
            instruction::{AccountMeta, Instruction},
            system_program,
        },
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    ecol_lote::{
        accounts as ctas, instruction as ixs,
        pesagem::{mensagem, TipoPesagem},
        Carteira, Coleta, ConfigParams, DadosColeta, DadosDistribuicao, DadosEntrega,
        DadosFechamento, DadosLote, DecisaoDisputa, Distribuicao, Entrega, EstadoLote,
        GlobalConfig, Lote, Material, MaterialVariacao, ModoOperacao, ModoRetirada, MotivoDisputa,
        OrigemEntrega, Papel, Participante, SEED_ASSET, SEED_AUTORIDADE_ASSET, SEED_BALANCA,
        SEED_CARTEIRA, SEED_COLETA, SEED_DISTRIBUICAO, SEED_ENTREGA, SEED_GLOBAL, SEED_LOTE,
        SEED_MATERIAL, SEED_PARTICIPANTE, SEED_VARIACAO,
    },
    litesvm::{types::TransactionMetadata, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, InstructionError},
    solana_transaction_error::TransactionError,
};

pub const PRAZO_ENTREGA_S: i64 = 7 * 24 * 3600;
pub const TOLERANCIA_BPS: u16 = 200; // 2%
pub const T0: i64 = 1_790_000_000;
pub const PESO_G: u64 = 1_000_000; // 1 t
/// Material cadastrado no `novo()` e usado nos lotes de teste.
pub const PLASTICO: u16 = 1;
/// Material com variações de cor e garrafas, criado por `preparar_vidro` (ADR 0011).
pub const VIDRO: u16 = 3;
pub const TRANSPARENTE: u8 = 1;
pub const VERDE: u8 = 2;
pub const MARROM: u8 = 3;

pub struct Ambiente {
    pub svm: LiteSVM,
    /// Fee payer da EcolChain (Kora/relayer). Sem papel on-chain: pagar a taxa não autoriza nada.
    pub patrocinador: Keypair,
    pub upgrade_authority: Keypair,
    pub admin: Keypair,
    pub arbitro: Keypair,
    pub operador: Keypair,
    pub intermediador: Keypair,
    pub credito_autoridade: Keypair,
    pub cooperativa: Keypair,
    pub industria: Keypair,
    pub transportador: Keypair,
    /// Keypair do equipamento de pesagem instalado na cooperativa.
    pub balanca_coop: Keypair,
    /// Keypair do equipamento de pesagem instalado na indústria.
    pub balanca_ind: Keypair,
}

pub fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ecol_lote::id()).0
}

pub fn config_pda() -> Pubkey {
    pda(&[SEED_GLOBAL])
}

pub fn participante_pda(carteira: &Pubkey) -> Pubkey {
    pda(&[SEED_PARTICIPANTE, carteira.as_ref()])
}

pub fn lote_pda(cooperativa: &Pubkey, lote_id: u64) -> Pubkey {
    pda(&[SEED_LOTE, cooperativa.as_ref(), &lote_id.to_le_bytes()])
}

pub fn entrega_pda(cooperativa: &Pubkey, entrega_id: u64) -> Pubkey {
    pda(&[
        SEED_ENTREGA,
        cooperativa.as_ref(),
        &entrega_id.to_le_bytes(),
    ])
}

pub fn material_pda(codigo: u16) -> Pubkey {
    pda(&[SEED_MATERIAL, &codigo.to_le_bytes()])
}

pub fn variacao_pda(codigo: u16, indice: u8) -> Pubkey {
    pda(&[SEED_VARIACAO, &codigo.to_le_bytes(), &[indice]])
}

pub fn carteira_pda(endereco: &Pubkey) -> Pubkey {
    pda(&[SEED_CARTEIRA, endereco.as_ref()])
}

pub fn distribuicao_pda(importador: &Pubkey, id: u64) -> Pubkey {
    pda(&[SEED_DISTRIBUICAO, importador.as_ref(), &id.to_le_bytes()])
}

pub fn coleta_pda(importador: &Pubkey, id: u64) -> Pubkey {
    pda(&[SEED_COLETA, importador.as_ref(), &id.to_le_bytes()])
}

pub fn balanca_pda(dispositivo: &Pubkey) -> Pubkey {
    pda(&[SEED_BALANCA, dispositivo.as_ref()])
}

pub fn asset_pda(lote: &Pubkey) -> Pubkey {
    pda(&[SEED_ASSET, lote.as_ref()])
}

pub fn autoridade_asset() -> Pubkey {
    pda(&[SEED_AUTORIDADE_ASSET])
}

/// Dono gravado no Core asset (layout BaseAssetV1: key u8 | owner Pubkey | ...).
pub fn dono_do_asset(svm: &LiteSVM, asset: &Pubkey) -> Pubkey {
    let conta = svm.get_account(asset).expect("asset não existe");
    assert_eq!(
        conta.owner,
        mpl_core::ID,
        "asset não pertence ao Metaplex Core"
    );
    Pubkey::try_from(&conta.data[1..33]).unwrap()
}

/// Instrução Ed25519 com a balança assinando a pesagem, como o app/firmware fará.
pub fn ix_pesagem(
    balanca: &Keypair,
    tipo: TipoPesagem,
    lote: &Pubkey,
    peso_g: u64,
    ts: i64,
) -> Instruction {
    let msg = mensagem(tipo, lote, peso_g, ts);
    let assinatura = balanca.sign_message(&msg);
    let assinatura: [u8; 64] = assinatura.as_ref().try_into().unwrap();
    solana_ed25519_program::new_ed25519_instruction_with_signature(
        &msg,
        &assinatura,
        &balanca.pubkey().to_bytes(),
    )
}

pub fn event_authority() -> Pubkey {
    pda(&[b"__event_authority"])
}

pub fn h(tag: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    let b = tag.as_bytes();
    out[..b.len().min(32)].copy_from_slice(&b[..b.len().min(32)]);
    out
}

/// Código numérico de um erro do programa (6000 + índice).
pub fn codigo(e: ecol_lote::error::EcolLoteError) -> u32 {
    e.into()
}

impl Ambiente {
    /// SVM com o programa carregado e a upgrade authority gravada no ProgramData
    /// (o LiteSVM a cria vazia), sem inicializar a configuração.
    pub fn novo_sem_config() -> Self {
        let mut svm = LiteSVM::new();
        let bytes = include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/ecol_lote.so"
        ));
        svm.add_program(ecol_lote::id(), bytes).unwrap();
        let core = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/mpl_core.so"
        ));
        svm.add_program(mpl_core::ID, core).unwrap();

        let upgrade_authority = Keypair::new();
        let program_data = Pubkey::find_program_address(
            &[ecol_lote::id().as_ref()],
            &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
        )
        .0;
        let mut conta = svm.get_account(&program_data).unwrap();
        // Layout ProgramData: tag u32 | slot u64 | Option<Pubkey> (1 + 32).
        conta.data[12] = 1;
        conta.data[13..45].copy_from_slice(upgrade_authority.pubkey().as_ref());
        svm.set_account(program_data, conta).unwrap();

        let mut amb = Ambiente {
            svm,
            patrocinador: Keypair::new(),
            upgrade_authority,
            admin: Keypair::new(),
            arbitro: Keypair::new(),
            operador: Keypair::new(),
            intermediador: Keypair::new(),
            credito_autoridade: Keypair::new(),
            cooperativa: Keypair::new(),
            industria: Keypair::new(),
            transportador: Keypair::new(),
            balanca_coop: Keypair::new(),
            balanca_ind: Keypair::new(),
        };
        // Só o patrocinador (fee payer) precisa de SOL.
        let patrocinador = amb.patrocinador.pubkey();
        amb.svm.airdrop(&patrocinador, 10_000_000_000).unwrap();
        amb.set_tempo(T0);
        amb
    }

    /// Configuração inicializada e cooperativa, indústria e transportador cadastrados.
    pub fn novo() -> Self {
        let mut amb = Self::novo_sem_config();
        let ua = amb.upgrade_authority.insecure_clone();
        amb.init_global(&ua).unwrap();
        for (carteira, papel) in [
            (amb.cooperativa.pubkey(), Papel::Cooperativa),
            (amb.industria.pubkey(), Papel::Industria),
            (amb.transportador.pubkey(), Papel::Transportador),
        ] {
            amb.registrar(carteira, papel).unwrap();
        }
        let (coop, bc) = (amb.cooperativa.pubkey(), amb.balanca_coop.pubkey());
        amb.registrar_balanca(coop, bc).unwrap();
        let (ind, bi) = (amb.industria.pubkey(), amb.balanca_ind.pubkey());
        amb.registrar_balanca(ind, bi).unwrap();
        amb.criar_material(PLASTICO, "Plástico").unwrap();
        amb
    }

    pub fn criar_material_por(
        &mut self,
        operador: &Keypair,
        codigo: u16,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorCreateMaterial {
                codigo,
                nome: nome.to_string(),
            },
            ctas::OperadorCreateMaterial {
                payer: self.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_pda(),
                material: material_pda(codigo),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[operador])
    }

    pub fn criar_material(
        &mut self,
        codigo: u16,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        self.criar_material_por(&operador, codigo, nome)
    }

    pub fn atualizar_material(
        &mut self,
        codigo: u16,
        nome: &str,
        ativo: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorUpdateMaterial {
                nome: nome.to_string(),
                ativo,
            },
            ctas::OperadorUpdateMaterial {
                operador: operador.pubkey(),
                config: config_pda(),
                material: material_pda(codigo),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn material(&self, codigo: u16) -> Material {
        let conta = self.svm.get_account(&material_pda(codigo)).unwrap();
        Material::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn registrar_balanca(
        &mut self,
        dono: Pubkey,
        dispositivo: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        self.registrar_balanca_com_nome(dono, dispositivo, "Balança")
    }

    pub fn registrar_balanca_com_nome(
        &mut self,
        dono: Pubkey,
        dispositivo: Pubkey,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorRegisterBalanca {
                dispositivo,
                nome: nome.into(),
            },
            ctas::OperadorRegisterBalanca {
                payer: self.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_pda(),
                dono_part: participante_pda(&dono),
                balanca: balanca_pda(&dispositivo),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn set_balanca_ativa(
        &mut self,
        balanca: Pubkey,
        ativa: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorSetBalancaAtiva { ativa },
            ctas::OperadorSetBalancaAtiva {
                operador: operador.pubkey(),
                config: config_pda(),
                balanca,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn agora(&self) -> i64 {
        self.svm.get_sysvar::<Clock>().unix_timestamp
    }

    pub fn dados(&self, lote_id: u64) -> DadosLote {
        DadosLote {
            lote_id,
            material: PLASTICO,
            evidencias_hash: h("fotos+ticket"),
            variacao: 0,
        }
    }

    pub fn dados_fechamento(&self, peso_g: u64) -> DadosFechamento {
        DadosFechamento {
            peso_g,
            ts_pesagem: self.agora(),
            uri: "https://arweave.net/lote-metadata.json".to_string(),
        }
    }

    /// Abre o lote de venda para montagem (sem pesagem).
    pub fn ix_criar_lote(&self, cooperativa: &Pubkey, dados: DadosLote) -> Instruction {
        let lote = lote_pda(cooperativa, dados.lote_id);
        let material_cadastro = material_pda(dados.material);
        let variacao = (dados.variacao > 0).then(|| variacao_pda(dados.material, dados.variacao));
        self.ix(
            ixs::CooperativaCreateLote { dados },
            ctas::CooperativaCreateLote {
                payer: self.patrocinador.pubkey(),
                cooperativa: *cooperativa,
                cooperativa_assinante: *cooperativa,
                cooperativa_carteira: None,
                cooperativa_part: participante_pda(cooperativa),
                config: config_pda(),
                material_cadastro,
                variacao_cadastro: variacao,
                lote,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    /// Instrução de fechamento crua, sem a pesagem; os testes de ataque montam a transação à mão.
    pub fn ix_fechar_lote(
        &self,
        cooperativa: &Pubkey,
        balanca: Pubkey,
        lote: Pubkey,
        dados: DadosFechamento,
    ) -> Instruction {
        self.ix(
            ixs::CooperativaFecharLote { dados },
            ctas::CooperativaFecharLote {
                payer: self.patrocinador.pubkey(),
                cooperativa: *cooperativa,
                cooperativa_assinante: *cooperativa,
                cooperativa_carteira: None,
                cooperativa_part: participante_pda(cooperativa),
                config: config_pda(),
                balanca,
                lote,
                asset: asset_pda(&lote),
                autoridade_asset: autoridade_asset(),
                mpl_core_program: mpl_core::ID,
                instrucoes_sysvar: solana_instructions_sysvar::ID,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    pub fn abrir_lote(
        &mut self,
        cooperativa: &Keypair,
        dados: DadosLote,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix_criar_lote(&cooperativa.pubkey(), dados);
        self.enviar(ix, &[cooperativa])
    }

    /// Pesagem do consolidado (assinada pela `balanca`) + fechamento.
    pub fn fechar_lote(
        &mut self,
        cooperativa: &Keypair,
        balanca: &Keypair,
        lote: Pubkey,
        peso_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let dados = self.dados_fechamento(peso_g);
        let pesagem = ix_pesagem(
            balanca,
            TipoPesagem::Origem,
            &lote,
            peso_g,
            dados.ts_pesagem,
        );
        let fechar = self.ix_fechar_lote(
            &cooperativa.pubkey(),
            balanca_pda(&balanca.pubkey()),
            lote,
            dados,
        );
        self.enviar_varias(&[pesagem, fechar], &[cooperativa])
    }

    pub fn desmontar(
        &mut self,
        cooperativa: &Keypair,
        lote: Pubkey,
        entregas: &[Pubkey],
    ) -> Result<TransactionMetadata, TransactionError> {
        let mut ix = self.ix(
            ixs::CooperativaDesmontarLote {},
            ctas::CooperativaDesmontarLote {
                payer: self.patrocinador.pubkey(),
                cooperativa: cooperativa.pubkey(),
                cooperativa_assinante: cooperativa.pubkey(),
                cooperativa_carteira: None,
                config: config_pda(),
                lote,
                asset: asset_pda(&lote),
                autoridade_asset: autoridade_asset(),
                mpl_core_program: mpl_core::ID,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        ix.accounts
            .extend(entregas.iter().map(|e| AccountMeta::new(*e, false)));
        self.enviar(ix, &[cooperativa])
    }

    pub fn custodia(&self, lote: &Pubkey) -> ctas::ContasCustodia {
        ctas::ContasCustodia {
            payer: self.patrocinador.pubkey(),
            asset: asset_pda(lote),
            autoridade_asset: autoridade_asset(),
            mpl_core_program: mpl_core::ID,
            system_program: system_program::ID,
        }
    }

    pub fn set_tempo(&mut self, unix_timestamp: i64) {
        let mut clock: Clock = self.svm.get_sysvar();
        clock.unix_timestamp = unix_timestamp;
        self.svm.set_sysvar(&clock);
    }

    /// Envia com o patrocinador como fee payer, como a EcolChain fará em produção.
    pub fn enviar(
        &mut self,
        ix: Instruction,
        assinantes: &[&Keypair],
    ) -> Result<TransactionMetadata, TransactionError> {
        self.enviar_varias(&[ix], assinantes)
    }

    pub fn enviar_varias(
        &mut self,
        ixs: &[Instruction],
        assinantes: &[&Keypair],
    ) -> Result<TransactionMetadata, TransactionError> {
        self.svm.expire_blockhash();
        let payer = self.patrocinador.insecure_clone();
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
        let mut todos: Vec<&Keypair> = vec![&payer];
        for a in assinantes {
            if a.pubkey() != payer.pubkey() {
                todos.push(a);
            }
        }
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &todos).unwrap();
        self.svm.send_transaction(tx).map_err(|f| f.err)
    }

    pub fn ix(&self, data: impl InstructionData, contas: impl ToAccountMetas) -> Instruction {
        Instruction::new_with_bytes(ecol_lote::id(), &data.data(), contas.to_account_metas(None))
    }

    pub fn params(&self) -> ConfigParams {
        ConfigParams {
            arbitro: self.arbitro.pubkey(),
            operador: self.operador.pubkey(),
            intermediador: self.intermediador.pubkey(),
            credito_autoridade: self.credito_autoridade.pubkey(),
            prazo_entrega_s: PRAZO_ENTREGA_S,
            tolerancia_peso_bps: TOLERANCIA_BPS,
        }
    }

    pub fn init_global(
        &mut self,
        autoridade: &Keypair,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminInitGlobal {
                admin: self.admin.pubkey(),
                params: self.params(),
            },
            ctas::AdminInitGlobal {
                payer: self.patrocinador.pubkey(),
                upgrade_authority: autoridade.pubkey(),
                config: config_pda(),
                este_programa: ecol_lote::id(),
                program_data: Pubkey::find_program_address(
                    &[ecol_lote::id().as_ref()],
                    &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
                )
                .0,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[autoridade])
    }

    pub fn update_global(
        &mut self,
        admin: &Keypair,
        params: ConfigParams,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminUpdateGlobal { params },
            ctas::AdminUpdateGlobal {
                admin: admin.pubkey(),
                config: config_pda(),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[admin])
    }

    pub fn set_modo(
        &mut self,
        modo: ModoOperacao,
    ) -> Result<TransactionMetadata, TransactionError> {
        let admin = self.admin.insecure_clone();
        let ix = self.ix(
            ixs::AdminSetModo { modo },
            ctas::AdminUpdateGlobal {
                admin: admin.pubkey(),
                config: config_pda(),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&admin])
    }

    pub fn registrar(
        &mut self,
        carteira: Pubkey,
        papel: Papel,
    ) -> Result<TransactionMetadata, TransactionError> {
        self.registrar_com_nome(carteira, papel, "Participante")
    }

    pub fn registrar_com_nome(
        &mut self,
        carteira: Pubkey,
        papel: Papel,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorRegisterParticipante {
                carteira,
                papel,
                kyc_hash: h("kyc"),
                nome: nome.into(),
            },
            ctas::OperadorRegisterParticipante {
                payer: self.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_pda(),
                participante: participante_pda(&carteira),
                vinculo: carteira_pda(&carteira),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn set_ativo(
        &mut self,
        carteira: Pubkey,
        ativo: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorSetParticipanteAtivo { ativo },
            ctas::OperadorSetParticipanteAtivo {
                operador: operador.pubkey(),
                config: config_pda(),
                participante: participante_pda(&carteira),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn set_balanca_nome_por(
        &mut self,
        operador: &Keypair,
        dispositivo: Pubkey,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorSetBalancaNome { nome: nome.into() },
            ctas::OperadorSetBalancaNome {
                operador: operador.pubkey(),
                config: config_pda(),
                balanca: balanca_pda(&dispositivo),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[operador])
    }

    pub fn set_nome_por(
        &mut self,
        operador: &Keypair,
        carteira: Pubkey,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorSetParticipanteNome { nome: nome.into() },
            ctas::OperadorSetParticipanteNome {
                operador: operador.pubkey(),
                config: config_pda(),
                participante: participante_pda(&carteira),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[operador])
    }

    /// Criação normal: pesagem assinada pela `balanca` + instrução do programa.
    pub fn criar_lote_com(
        &mut self,
        cooperativa: &Keypair,
        balanca: &Keypair,
        lote_id: u64,
        peso_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        // Lote de venda com um lote de origem de coletor do mesmo peso (id da entrega = id do lote);
        // o primeiro passo que falhar devolve o erro.
        let dados = self.dados(lote_id);
        let lote = lote_pda(&cooperativa.pubkey(), lote_id);
        self.abrir_lote(cooperativa, dados)?;
        self.registrar_entrega(cooperativa, balanca, lote_id, peso_g, PLASTICO)?;
        let entrega = entrega_pda(&cooperativa.pubkey(), lote_id);
        self.add_entregas(cooperativa, lote, &[entrega])?;
        self.fechar_lote(cooperativa, balanca, lote, peso_g)
    }

    pub fn dados_entrega(&self, entrega_id: u64, peso_g: u64, material: u16) -> DadosEntrega {
        DadosEntrega {
            entrega_id,
            origem: OrigemEntrega::Coletor,
            origem_ref: h(&format!("coletor-{entrega_id}+salt")),
            material,
            peso_g,
            ts_pesagem: self.agora(),
            variacao: 0,
            qtd_garrafas: 0,
        }
    }

    pub fn ix_registrar_entrega(
        &self,
        cooperativa: &Pubkey,
        balanca: Pubkey,
        dados: DadosEntrega,
    ) -> Instruction {
        self.ix_registrar_entrega_por(cooperativa, cooperativa, None, balanca, dados, None)
    }

    /// Registro do lote de origem com assinante, carteira vinculada e coleta explícitos.
    pub fn ix_registrar_entrega_por(
        &self,
        cooperativa: &Pubkey,
        assinante: &Pubkey,
        carteira: Option<Pubkey>,
        balanca: Pubkey,
        dados: DadosEntrega,
        coleta: Option<Pubkey>,
    ) -> Instruction {
        let entrega = entrega_pda(cooperativa, dados.entrega_id);
        let material_cadastro = material_pda(dados.material);
        let variacao = (dados.variacao > 0).then(|| variacao_pda(dados.material, dados.variacao));
        self.ix(
            ixs::CooperativaRegisterEntrega { dados },
            ctas::CooperativaRegisterEntrega {
                payer: self.patrocinador.pubkey(),
                cooperativa: *cooperativa,
                cooperativa_assinante: *assinante,
                cooperativa_carteira: carteira,
                cooperativa_part: participante_pda(cooperativa),
                config: config_pda(),
                balanca,
                material_cadastro,
                variacao_cadastro: variacao,
                coleta,
                entrega,
                instrucoes_sysvar: solana_instructions_sysvar::ID,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    /// Entrega pesada e assinada pela `balanca` da cooperativa.
    pub fn registrar_entrega(
        &mut self,
        cooperativa: &Keypair,
        balanca: &Keypair,
        entrega_id: u64,
        peso_g: u64,
        material: u16,
    ) -> Result<TransactionMetadata, TransactionError> {
        let dados = self.dados_entrega(entrega_id, peso_g, material);
        let entrega = entrega_pda(&cooperativa.pubkey(), entrega_id);
        let pesagem = ix_pesagem(
            balanca,
            TipoPesagem::Entrega,
            &entrega,
            peso_g,
            dados.ts_pesagem,
        );
        let ix =
            self.ix_registrar_entrega(&cooperativa.pubkey(), balanca_pda(&balanca.pubkey()), dados);
        self.enviar_varias(&[pesagem, ix], &[cooperativa])
    }

    pub fn add_entregas(
        &mut self,
        cooperativa: &Keypair,
        lote: Pubkey,
        entregas: &[Pubkey],
    ) -> Result<TransactionMetadata, TransactionError> {
        let mut ix = self.ix(
            ixs::CooperativaAddEntregas {},
            ctas::CooperativaAddEntregas {
                cooperativa: cooperativa.pubkey(),
                cooperativa_assinante: cooperativa.pubkey(),
                cooperativa_carteira: None,
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        ix.accounts
            .extend(entregas.iter().map(|e| AccountMeta::new(*e, false)));
        self.enviar(ix, &[cooperativa])
    }

    /// Criação com a balança da cooperativa principal.
    pub fn criar_lote_por(
        &mut self,
        cooperativa: &Keypair,
        lote_id: u64,
        peso_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let balanca = self.balanca_coop.insecure_clone();
        self.criar_lote_com(cooperativa, &balanca, lote_id, peso_g)
    }

    pub fn criar_lote(&mut self, lote_id: u64) -> Result<TransactionMetadata, TransactionError> {
        let coop = self.cooperativa.insecure_clone();
        self.criar_lote_por(&coop, lote_id, PESO_G)
    }

    pub fn anunciar_por(
        &mut self,
        cooperativa: &Keypair,
        lote: Pubkey,
        prazo_leilao: i64,
        preco_minimo_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::CooperativaListLote {
                prazo_leilao,
                preco_minimo_centavos,
            },
            ctas::CooperativaListLote {
                cooperativa: cooperativa.pubkey(),
                cooperativa_assinante: cooperativa.pubkey(),
                cooperativa_carteira: None,
                cooperativa_part: participante_pda(&cooperativa.pubkey()),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[cooperativa])
    }

    pub fn anunciar(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let coop = self.cooperativa.insecure_clone();
        self.anunciar_por(&coop, lote, T0 + 3600, 25_000)
    }

    pub fn encerrar_sem_lance(
        &mut self,
        lote: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorCloseLeilaoSemLance {},
            ctas::OperadorCloseLeilaoSemLance {
                operador: operador.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn vender_com(
        &mut self,
        industria: &Keypair,
        intermediador: &Keypair,
        lote: Pubkey,
        valor_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        self.vender_completo(industria, &operador, intermediador, lote, valor_centavos)
    }

    pub fn vender_completo(
        &mut self,
        industria: &Keypair,
        operador: &Keypair,
        intermediador: &Keypair,
        lote: Pubkey,
        valor_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let cooperativa = self.lote(lote).cooperativa;
        let ix = self.ix(
            ixs::IndustriaAcceptVenda {
                valor_centavos,
                escrow_ref_hash: h("escrow-123"),
                lances_root: h("lances"),
            },
            ctas::IndustriaAcceptVenda {
                industria: industria.pubkey(),
                industria_assinante: industria.pubkey(),
                industria_carteira: None,
                industria_part: participante_pda(&industria.pubkey()),
                cooperativa_part: participante_pda(&cooperativa),
                operador: operador.pubkey(),
                intermediador: intermediador.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[industria, operador, intermediador])
    }

    pub fn vender(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let (ind, inter) = (
            self.industria.insecure_clone(),
            self.intermediador.insecure_clone(),
        );
        self.vender_com(&ind, &inter, lote, 25_000)
    }

    pub fn retirar_com(
        &mut self,
        transportador: &Keypair,
        cooperativa: &Keypair,
        lote: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::TransportadorPickupLote {},
            ctas::TransportadorPickupLote {
                transportador: transportador.pubkey(),
                transportador_assinante: transportador.pubkey(),
                transportador_carteira: None,
                transportador_part: participante_pda(&transportador.pubkey()),
                cooperativa: cooperativa.pubkey(),
                cooperativa_assinante: cooperativa.pubkey(),
                cooperativa_carteira: None,
                cooperativa_part: participante_pda(&cooperativa.pubkey()),
                config: config_pda(),
                lote,
                custodia: self.custodia(&lote),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[transportador, cooperativa])
    }

    pub fn retirar(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let (t, c) = (
            self.transportador.insecure_clone(),
            self.cooperativa.insecure_clone(),
        );
        self.retirar_com(&t, &c, lote)
    }

    pub fn ix_receber(
        &self,
        industria: &Pubkey,
        lote: Pubkey,
        peso_recebido_g: u64,
        balanca: Option<Pubkey>,
        ts_pesagem: Option<i64>,
    ) -> Instruction {
        self.ix(
            ixs::IndustriaConfirmRecebimento {
                peso_recebido_g,
                ts_pesagem,
            },
            ctas::IndustriaConfirmRecebimento {
                industria: *industria,
                industria_assinante: *industria,
                industria_carteira: None,
                balanca,
                instrucoes_sysvar: solana_instructions_sysvar::ID,
                config: config_pda(),
                lote,
                custodia: self.custodia(&lote),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    /// Recebimento sem pesagem atestada.
    pub fn receber_por(
        &mut self,
        industria: &Keypair,
        lote: Pubkey,
        peso_recebido_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix_receber(&industria.pubkey(), lote, peso_recebido_g, None, None);
        self.enviar(ix, &[industria])
    }

    pub fn receber(
        &mut self,
        lote: Pubkey,
        peso_recebido_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ind = self.industria.insecure_clone();
        self.receber_por(&ind, lote, peso_recebido_g)
    }

    pub fn liberar_por(
        &mut self,
        intermediador: &Keypair,
        lote: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::IntermediadorConfirmLiberacao {
                liberacao_ref_hash: h("pix-liberacao"),
            },
            ctas::IntermediadorConfirmLiberacao {
                intermediador: intermediador.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[intermediador])
    }

    pub fn liberar(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let inter = self.intermediador.insecure_clone();
        self.liberar_por(&inter, lote)
    }

    pub fn abrir_disputa(
        &mut self,
        parte: &Keypair,
        lote: Pubkey,
        motivo: MotivoDisputa,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ParteOpenDisputa { motivo },
            ctas::ParteOpenDisputa {
                parte: parte.pubkey(),
                parte_assinante: parte.pubkey(),
                parte_carteira: None,
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[parte])
    }

    pub fn crank_prazo(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let cranker = Keypair::new();
        let ix = self.ix(
            ixs::PublicCrankPrazoExpirado {},
            ctas::PublicCrankPrazoExpirado {
                cranker: cranker.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&cranker])
    }

    pub fn resolver(
        &mut self,
        arbitro: &Keypair,
        intermediador: Option<&Keypair>,
        lote: Pubkey,
        decisao: DecisaoDisputa,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ArbitroResolveDisputa { decisao },
            ctas::ArbitroResolveDisputa {
                arbitro: arbitro.pubkey(),
                intermediador: intermediador.map(|k| k.pubkey()),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        let mut assinantes = vec![arbitro];
        if let Some(i) = intermediador {
            assinantes.push(i);
        }
        self.enviar(ix, &assinantes)
    }

    pub fn agregar_por(
        &mut self,
        autoridade: &Keypair,
        lote: Pubkey,
        credito: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::CreditoMarkAgregado { credito },
            ctas::CreditoMarkAgregado {
                credito_autoridade: autoridade.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[autoridade])
    }

    pub fn lote(&self, lote: Pubkey) -> Lote {
        let conta = self.svm.get_account(&lote).unwrap();
        Lote::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn config(&self) -> GlobalConfig {
        let conta = self.svm.get_account(&config_pda()).unwrap();
        GlobalConfig::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn participante(&self, carteira: &Pubkey) -> Participante {
        let conta = self.svm.get_account(&participante_pda(carteira)).unwrap();
        Participante::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    /// Cria o lote `lote_id` e o leva até `Recebido`.
    /// Lote de venda com duas origens que somam `PESO_G`: `sem_coletor_g` de doação e o resto de
    /// coletor (ids das entregas: `lote_id * 100 + 1` e `+ 2`), fechado com `PESO_G`.
    pub fn criar_lote_misto(&mut self, lote_id: u64, sem_coletor_g: u64) -> Pubkey {
        let coop = self.cooperativa.insecure_clone();
        let balanca = self.balanca_coop.insecure_clone();
        let lote = lote_pda(&coop.pubkey(), lote_id);
        let dados = self.dados(lote_id);
        self.abrir_lote(&coop, dados).unwrap();
        let (id_col, id_doa) = (lote_id * 100 + 1, lote_id * 100 + 2);
        self.registrar_entrega(&coop, &balanca, id_col, PESO_G - sem_coletor_g, PLASTICO)
            .unwrap();
        let mut dados = self.dados_entrega(id_doa, sem_coletor_g, PLASTICO);
        dados.origem = OrigemEntrega::Doacao;
        let alvo = entrega_pda(&coop.pubkey(), id_doa);
        let pesagem = ix_pesagem(
            &balanca,
            TipoPesagem::Entrega,
            &alvo,
            sem_coletor_g,
            dados.ts_pesagem,
        );
        let ix = self.ix_registrar_entrega(&coop.pubkey(), balanca_pda(&balanca.pubkey()), dados);
        self.enviar_varias(&[pesagem, ix], &[&coop]).unwrap();
        let entregas = [entrega_pda(&coop.pubkey(), id_col), alvo];
        self.add_entregas(&coop, lote, &entregas).unwrap();
        self.fechar_lote(&coop, &balanca, lote, PESO_G).unwrap();
        lote
    }

    pub fn lote_misto_ate_recebido(&mut self, lote_id: u64, sem_coletor_g: u64) -> Pubkey {
        let lote = self.criar_lote_misto(lote_id, sem_coletor_g);
        self.anunciar(lote).unwrap();
        self.vender(lote).unwrap();
        self.retirar(lote).unwrap();
        self.receber(lote, PESO_G).unwrap();
        lote
    }

    pub fn lote_ate_recebido(&mut self, lote_id: u64) -> Pubkey {
        let lote = lote_pda(&self.cooperativa.pubkey(), lote_id);
        self.criar_lote(lote_id).unwrap();
        self.anunciar(lote).unwrap();
        self.vender(lote).unwrap();
        self.retirar(lote).unwrap();
        self.receber(lote, PESO_G).unwrap();
        assert_eq!(self.lote(lote).estado, EstadoLote::Recebido);
        lote
    }

    // ---- ADR 0012: venda direta e lote vendido fora da plataforma ----

    /// Instrução da cooperativa sobre o lote (contas de `CooperativaListLote`), assinada pelo titular.
    pub fn ix_coop_lote(
        &self,
        cooperativa: &Pubkey,
        lote: Pubkey,
        data: impl InstructionData,
    ) -> Instruction {
        self.ix(
            data,
            ctas::CooperativaListLote {
                cooperativa: *cooperativa,
                cooperativa_assinante: *cooperativa,
                cooperativa_carteira: None,
                cooperativa_part: participante_pda(cooperativa),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    pub fn anunciar_venda_por(
        &mut self,
        cooperativa: &Keypair,
        lote: Pubkey,
        preco_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix_coop_lote(
            &cooperativa.pubkey(),
            lote,
            ixs::CooperativaAnunciarVenda { preco_centavos },
        );
        self.enviar(ix, &[cooperativa])
    }

    pub fn anunciar_venda(
        &mut self,
        lote: Pubkey,
        preco_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let coop = self.cooperativa.insecure_clone();
        self.anunciar_venda_por(&coop, lote, preco_centavos)
    }

    pub fn set_vendido_fora(
        &mut self,
        lote: Pubkey,
        vendido: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let coop = self.cooperativa.insecure_clone();
        let ix = self.ix_coop_lote(
            &coop.pubkey(),
            lote,
            ixs::CooperativaSetVendidoFora { vendido },
        );
        self.enviar(ix, &[&coop])
    }

    pub fn confirmar_pagamento_por(
        &mut self,
        cooperativa: &Keypair,
        lote: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix_coop_lote(
            &cooperativa.pubkey(),
            lote,
            ixs::CooperativaConfirmarPagamento {
                comprovante_hash: h("pix-da-industria"),
            },
        );
        self.enviar(ix, &[cooperativa])
    }

    pub fn comprar_por(
        &mut self,
        industria: &Keypair,
        lote: Pubkey,
        valor_centavos: u64,
        modo_retirada: ModoRetirada,
    ) -> Result<TransactionMetadata, TransactionError> {
        let cooperativa = self.lote(lote).cooperativa;
        let ix = self.ix(
            ixs::IndustriaComprarLote {
                valor_centavos,
                modo_retirada,
            },
            ctas::IndustriaComprarLote {
                industria: industria.pubkey(),
                industria_assinante: industria.pubkey(),
                industria_carteira: None,
                industria_part: participante_pda(&industria.pubkey()),
                cooperativa_part: participante_pda(&cooperativa),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[industria])
    }

    pub fn comprar(
        &mut self,
        lote: Pubkey,
        valor_centavos: u64,
        modo_retirada: ModoRetirada,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ind = self.industria.insecure_clone();
        self.comprar_por(&ind, lote, valor_centavos, modo_retirada)
    }

    // ---- ADR 0011: variações, carteiras vinculadas e importador ----

    pub fn criar_variacao(
        &mut self,
        codigo: u16,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let indice = self.material(codigo).qtd_variacoes + 1;
        let ix = self.ix(
            ixs::OperadorCreateVariacao { nome: nome.into() },
            ctas::OperadorCreateVariacao {
                payer: self.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_pda(),
                material: material_pda(codigo),
                variacao: variacao_pda(codigo, indice),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn atualizar_variacao(
        &mut self,
        codigo: u16,
        indice: u8,
        nome: &str,
        ativa: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorUpdateVariacao {
                nome: nome.into(),
                ativa,
            },
            ctas::OperadorUpdateVariacao {
                operador: operador.pubkey(),
                config: config_pda(),
                variacao: variacao_pda(codigo, indice),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn set_garrafas(
        &mut self,
        codigo: u16,
        conta_garrafas: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorSetMaterialGarrafas { conta_garrafas },
            ctas::OperadorSetMaterialGarrafas {
                operador: operador.pubkey(),
                config: config_pda(),
                material: material_pda(codigo),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn variacao(&self, codigo: u16, indice: u8) -> MaterialVariacao {
        let conta = self.svm.get_account(&variacao_pda(codigo, indice)).unwrap();
        MaterialVariacao::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    /// Vidro (código `VIDRO`) com Transparente, Verde e Marrom e contagem de garrafas.
    pub fn preparar_vidro(&mut self) {
        self.criar_material(VIDRO, "Vidro").unwrap();
        for cor in ["Transparente", "Verde", "Marrom"] {
            self.criar_variacao(VIDRO, cor).unwrap();
        }
        self.set_garrafas(VIDRO, true).unwrap();
    }

    pub fn add_carteira_por(
        &mut self,
        autoridade: &Keypair,
        titular: Pubkey,
        endereco: Pubkey,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ParticipanteAddCarteira {
                endereco,
                nome: nome.into(),
            },
            ctas::ParticipanteAddCarteira {
                payer: self.patrocinador.pubkey(),
                autoridade: autoridade.pubkey(),
                config: config_pda(),
                participante: participante_pda(&titular),
                endereco_part: participante_pda(&endereco),
                carteira: carteira_pda(&endereco),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[autoridade])
    }

    pub fn update_carteira_por(
        &mut self,
        autoridade: &Keypair,
        titular: Pubkey,
        endereco: Pubkey,
        nome: &str,
        ativa: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ParticipanteUpdateCarteira {
                nome: nome.into(),
                ativa,
            },
            ctas::ParticipanteUpdateCarteira {
                autoridade: autoridade.pubkey(),
                config: config_pda(),
                participante: participante_pda(&titular),
                carteira: carteira_pda(&endereco),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[autoridade])
    }

    pub fn carteira(&self, endereco: &Pubkey) -> Carteira {
        let conta = self.svm.get_account(&carteira_pda(endereco)).unwrap();
        Carteira::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    /// Lote de origem pesado pela `balanca`, assinado por `assinante` em nome de `titular`.
    /// `carteira`: passa a conta `["carteira", assinante]` (quando o assinante não é o titular).
    pub fn registrar_entrega_com(
        &mut self,
        titular: &Pubkey,
        assinante: &Keypair,
        carteira: bool,
        balanca: &Keypair,
        dados: DadosEntrega,
        coleta: Option<Pubkey>,
    ) -> Result<TransactionMetadata, TransactionError> {
        let entrega = entrega_pda(titular, dados.entrega_id);
        let pesagem = ix_pesagem(
            balanca,
            TipoPesagem::Entrega,
            &entrega,
            dados.peso_g,
            dados.ts_pesagem,
        );
        let ix = self.ix_registrar_entrega_por(
            titular,
            &assinante.pubkey(),
            carteira.then(|| carteira_pda(&assinante.pubkey())),
            balanca_pda(&balanca.pubkey()),
            dados,
            coleta,
        );
        self.enviar_varias(&[pesagem, ix], &[assinante])
    }

    /// Dados de um lote de origem de vidro (coletor) com a cor e as garrafas.
    pub fn dados_entrega_vidro(
        &self,
        entrega_id: u64,
        peso_g: u64,
        cor: u8,
        garrafas: u32,
    ) -> DadosEntrega {
        let mut d = self.dados_entrega(entrega_id, peso_g, VIDRO);
        d.variacao = cor;
        d.qtd_garrafas = garrafas;
        d
    }

    pub fn entrega(&self, cooperativa: &Pubkey, entrega_id: u64) -> Entrega {
        let conta = self
            .svm
            .get_account(&entrega_pda(cooperativa, entrega_id))
            .unwrap();
        Entrega::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn criar_distribuicao_por(
        &mut self,
        importador: &Pubkey,
        assinante: &Keypair,
        carteira: bool,
        dados: DadosDistribuicao,
    ) -> Result<TransactionMetadata, TransactionError> {
        let variacao = (dados.variacao > 0).then(|| variacao_pda(dados.material, dados.variacao));
        let ix = self.ix(
            ixs::ImportadorCreateDistribuicao {
                dados: dados.clone(),
            },
            ctas::ImportadorCreateDistribuicao {
                payer: self.patrocinador.pubkey(),
                importador: *importador,
                importador_assinante: assinante.pubkey(),
                importador_carteira: carteira.then(|| carteira_pda(&assinante.pubkey())),
                importador_part: participante_pda(importador),
                config: config_pda(),
                material_cadastro: material_pda(dados.material),
                variacao_cadastro: variacao,
                distribuicao: distribuicao_pda(importador, dados.distrib_id),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[assinante])
    }

    pub fn registrar_coleta_por(
        &mut self,
        importador: &Pubkey,
        assinante: &Keypair,
        dados: DadosColeta,
    ) -> Result<TransactionMetadata, TransactionError> {
        let variacao = (dados.variacao > 0).then(|| variacao_pda(dados.material, dados.variacao));
        let ix = self.ix(
            ixs::ImportadorRegisterColeta {
                dados: dados.clone(),
            },
            ctas::ImportadorRegisterColeta {
                payer: self.patrocinador.pubkey(),
                importador: *importador,
                importador_assinante: assinante.pubkey(),
                importador_carteira: (assinante.pubkey() != *importador)
                    .then(|| carteira_pda(&assinante.pubkey())),
                importador_part: participante_pda(importador),
                config: config_pda(),
                material_cadastro: material_pda(dados.material),
                variacao_cadastro: variacao,
                coleta: coleta_pda(importador, dados.coleta_id),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[assinante])
    }

    pub fn entregar_coleta(
        &mut self,
        importador: &Keypair,
        coleta_id: u64,
        destino: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ImportadorEntregarColeta {},
            ctas::ImportadorEntregarColeta {
                importador: importador.pubkey(),
                importador_assinante: importador.pubkey(),
                importador_carteira: None,
                importador_part: participante_pda(&importador.pubkey()),
                config: config_pda(),
                coleta: coleta_pda(&importador.pubkey(), coleta_id),
                destino_part: participante_pda(&destino),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[importador])
    }

    pub fn cancelar_entrega_coleta(
        &mut self,
        importador: &Keypair,
        coleta_id: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ImportadorCancelarEntrega {},
            ctas::ImportadorCancelarEntrega {
                importador: importador.pubkey(),
                importador_assinante: importador.pubkey(),
                importador_carteira: None,
                config: config_pda(),
                coleta: coleta_pda(&importador.pubkey(), coleta_id),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[importador])
    }

    pub fn informar_reciclagem(
        &mut self,
        importador: &Keypair,
        coleta_id: u64,
        entrega: Pubkey,
        lote: Pubkey,
        nf: [u8; 32],
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ImportadorInformarReciclagem {
                nf_reciclagem_hash: nf,
            },
            ctas::ImportadorInformarReciclagem {
                importador: importador.pubkey(),
                importador_assinante: importador.pubkey(),
                importador_carteira: None,
                config: config_pda(),
                coleta: coleta_pda(&importador.pubkey(), coleta_id),
                entrega,
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[importador])
    }

    pub fn coleta(&self, importador: &Pubkey, coleta_id: u64) -> Coleta {
        let conta = self
            .svm
            .get_account(&coleta_pda(importador, coleta_id))
            .unwrap();
        Coleta::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn distribuicao(&self, importador: &Pubkey, id: u64) -> Distribuicao {
        let conta = self
            .svm
            .get_account(&distribuicao_pda(importador, id))
            .unwrap();
        Distribuicao::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }
}

/// Afirma que a transação falhou com o erro `esperado` do programa.
#[track_caller]
pub fn assert_erro(
    res: Result<TransactionMetadata, TransactionError>,
    esperado: ecol_lote::error::EcolLoteError,
) {
    let esperado_nome = format!("{esperado:?}");
    let codigo_esperado = codigo(esperado);
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(
                c, codigo_esperado,
                "esperava {esperado_nome} ({codigo_esperado}), veio {c}"
            )
        }
        outro => panic!("esperava {esperado_nome}, veio {outro:?}"),
    }
}

/// Afirma que a transação falhou com um erro do próprio Anchor (conta, seeds, discriminador).
#[track_caller]
pub fn assert_erro_anchor(
    res: Result<TransactionMetadata, TransactionError>,
    esperado: anchor_lang::error::ErrorCode,
) {
    let codigo_esperado: u32 = esperado.into();
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(c, codigo_esperado, "esperava {esperado:?}, veio {c}")
        }
        outro => panic!("esperava {esperado:?}, veio {outro:?}"),
    }
}
