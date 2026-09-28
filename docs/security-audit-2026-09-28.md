# Auditoria de segurança — `ecol_lote` (2026-09-28)

Auditoria interna seguindo o procedimento `/audit-solana` do [solana-ai-kit](https://github.com/solanabr/solana-ai-kit)
(checklists: `security.md` do solana-dev, `safe-solana-builder`, `solana-vulnerability-scanner` da Trail of Bits).
Escopo: commit `da74c80` (F1) e as mudanças da F2 (balança Ed25519, Core asset). Não substitui auditoria externa.

## Resumo

| Severidade | Encontrados | Corrigidos | Abertos |
|---|---|---|---|
| Crítica | 0 | — | — |
| Alta | 2 | 2 | 0 |
| Média | 4 | 4 | 0 |
| Baixa | 4 | 4 | 0 |
| Info | 4 | 1 | 3 |

Sem bugs clássicos (owner, signer, PDA, type confusion, CPI arbitrária, aritmética): todas as contas de estado são
`Account<T>`, toda ação privilegiada tem signer amarrado ao estado, bumps canônicos armazenados, init protegido pela
upgrade authority. Os achados são de **lógica de negócio**. Seis foram confirmados com PoC em LiteSVM antes da correção.

## Achados

Caminhos relativos a `onchain/programs/ecol_lote/src/`. Cada correção tem teste de regressão em `tests/auditoria.rs`.

| # | Sev. | Título | Correção | Teste |
|---|---|---|---|---|
| 1 | Alta | Disputa perpétua trava a liberação do escrow: após "prosseguir", a parte reabria a disputa na mesma etapa indefinidamente | `Lote.etapa_julgada`; partes não reabrem etapa julgada (`DisputaJaJulgada`). Nova decisão `Liberar` resolve e liquida atomicamente | `etapa_julgada_nao_pode_ser_redisputada_pelas_partes`, `arbitro_libera_parcialmente_com_peso_reclassificado` |
| 2 | Alta | Chave do operador era fee payer **e** autoridade: co-assinar como pagador de uma transação montada pelo cliente autorizava cadastros | Fee payer (patrocinador) é chave separada, sem papel on-chain; ADR 0004 atualizada; harness de testes usa as duas chaves | `fee_payer_nao_age_como_operador` |
| 3 | Média | Venda registrada só com indústria + intermediador, sem apuração do leilão | `industria_accept_venda` exige co-assinatura do `operador` (apurador) | `venda_exige_operador_apurador` |
| 4 | Média | Pausa não congelava prazos; crank derrubava lotes em massa na retomada | Crank só em `Normal`; carência de 48 h após voltar a `Normal` (`GlobalConfig.retomado_em`) | `crank_bloqueado_fora_do_normal_e_com_carencia_na_retomada` |
| 5 | Média | Participante desativado seguia anunciando, vendendo e entregando | `ativo` exigido da cooperativa em anúncio, venda e retirada. Recebimento, liberação e disputa continuam (dinheiro já depositado) — política intencional | `cooperativa_inativa_nao_anuncia_vende_nem_entrega` |
| 6 | Média | Reclassificação de peso sem ajuste de valor; liberação sempre do valor cheio | `DecisaoDisputa::Liberar { valor_liberado_centavos, .. }` com reembolso do restante; `Lote.valor_liberado_centavos` | idem #1 |
| 7 | Baixa | Chaves de papel podiam coincidir (árbitro = intermediador) ou virar participante | Chaves do config distintas e não nulas; carteira reservada não vira participante nem compra | `chaves_de_config_distintas_e_reservadas` |
| 8 | Baixa | Troca de admin em um passo; `prazo_entrega_s` sem teto | `admin_propose_admin` + `admin_accept_admin`; prazo ≤ 180 dias | `troca_de_admin_em_dois_passos` |
| 9 | Baixa | Liberação bloqueada em `SomenteDisputas`, reembolso não | `intermediador_confirm_liberacao` permitido em `SomenteDisputas` | `liberacao_permitida_em_somente_disputas` |
| 10 | Baixa | Anúncio sem prazo máximo, sem cancelamento, preço mínimo zero | Prazo ≤ 90 dias, preço > 0, `cooperativa_cancel_anuncio` | `anuncio_limitado_e_cancelavel` |
| — | — | Tolerância de peso lida do config no recebimento (admin mudava resultado de lotes em trânsito) | Fixada no lote na venda | `tolerancia_fixada_na_venda` |
| 11 | Info | Corrida entre encerrar leilão e vender | Backend/intermediador checam estado antes de aceitar depósito | — (operacional) |
| 12 | Info | `eh_parte` aceitava `Pubkey::default()` | Ignora chave nula | coberto pela matriz |
| 13 | Info | Rent de `Lote` nunca recuperado | **Aberto**: instrução `close` para lotes terminais após período de retenção | — |
| 14 | Info | `credito_mark_agregado` confia inteiramente na PDA do `ecol_credito` | **Corrigido na F3**: a PDA `["autoridade_credito"]` só assina em `operador_add_lotes`, depois de validar estado do crédito, owner/discriminator e material de cada lote; o estado do lote é relido após a CPI | `ecol_credito/tests/credito.rs` |

## Achados da F2 (balança e Core asset)

- **Mesmo equipamento em duas cooperativas:** seeds da balança eram `(dono, dispositivo)`. Corrigido para `["balanca", dispositivo]` (`equipamento_nao_pode_ter_dois_donos`).
- **Introspecção Ed25519** (padrão 4.6 da Trail of Bits): índice relativo `-1`, e os três `instruction_index` precisam ser `u16::MAX` (dados na própria instrução). Mensagem com domínio, tipo de pesagem, PDA do lote, peso e timestamp (janela de 15 min). Testes cobrem: ausência, instrução intermediária, outra chave, peso divergente, outro lote, tipo trocado, timestamp fora da janela, índice explícito, balança inativa/de outro dono.
- **Core asset:** `PermanentFreezeDelegate(frozen)` + `PermanentTransferDelegate`, ambos com autoridade na PDA do programa. Teste confirma que o dono não transfere nem queima (erro 9 do Core), com teste de controle provando que a instrução estava bem formada.

## Precisa verificação

- Participantes com carteira embutida e SOL próprio podem enviar transações sem o backend: os controles acima não dependem do backend, mas vale confirmar o modelo final de envio.
- Front-run do crank logo após `prazo_entrega`: força disputa que o árbitro resolve com "prosseguir". Avaliar carência também aqui.
- Taxa não recuperável de ~0,0015 SOL por Core asset (diferença entre lamports do asset e o rent de 234 bytes): confirmar se é a taxa de protocolo do Metaplex Core.

## Testes e ferramentas

- `NO_DNA=1 anchor test`: 46 testes (5 unitários + 41 LiteSVM) passando.
- Mutações de controle: desligar a checagem de peso e a trava de etapa julgada faz os testes correspondentes falharem.
- `cargo clippy --lib -- -W clippy::arithmetic_side_effects -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic -D warnings`: limpo.
- Compute units (`tests/compute_units.rs`): criação 61,8k CU (Ed25519 + Core create); demais < 33k CU.
- `cargo audit` (v0.22.2): 2 vulnerabilidades — RUSTSEC-2022-0093 (`ed25519-dalek` 1.0.1) e RUSTSEC-2024-0344 (`curve25519-dalek` 3.2.0) — **ambas só em dev-dependency** (`litesvm` → `agave-precompiles`, usado nos testes); o programa on-chain usa `curve25519-dalek` 4.1.3. 6 avisos de crates sem manutenção (`bincode`, `paste`, `derivative`, `ansi_term`, `libsecp256k1`, `rand` 0.7), todos transitivos do ecossistema Solana/Anchor. Nenhuma ação no programa.
- Fuzz: em vez de Trident (os programas não guardam fundos; o dinheiro é off-chain), testes de propriedade com `proptest` (F5): repartição fecha a receita e respeita percentuais (5.000 casos), percentual fora da faixa rejeitado, provas Merkle válidas/adulteradas (500 árvores aleatórias), sequências aleatórias de transições do lote respeitam o grafo (5.000 casos), tolerância de peso simétrica e monotônica. Trident fica recomendado antes de mainnet.
- E2E no Surfpool (F5): ciclo completo com os três programas + Metaplex Core via clientes Codama; time travel + crank de prazo.

## Adendo F3 — `ecol_credito`

Revisão com o mesmo checklist. Sem achados técnicos: CPI só para o `ecol_lote` (`Program<EcolLote>` + ID fixo),
seeds da PDA nunca repassadas a outro programa, `remaining_accounts` com owner + discriminator checados e limite de 10,
estado relido após a CPI, aritmética checada, init restrito à upgrade authority, admin em dois passos,
serial único por PDA `["serial", padrão, sha256(serial)]`.

**Decisões de negócio (resolvidas em 2026-09-28):**

| # | Sev. | Tema | Situação |
|---|---|---|---|
| C1 | Média | O operador sozinho pode agregar lotes a um crédito que nunca é submetido ou que é rejeitado: os lotes ficariam `Agregado` para sempre | **Corrigido** (decisão do usuário): prazo de 180 dias em `EmFormacao`; crank público `public_release_lotes` declara `Abandonado` e, em `Rejeitado`/`Abandonado`, devolve os lotes a `Reciclado` via `credito_mark_desagregado` (mesma PDA, só para o crédito ao qual o lote está vinculado) |
| C2 | Média | O valor da venda do crédito (base da repartição) era declarado por operador + comprador | **Corrigido** (decisão do usuário): o intermediador de pagamento co-assina a venda com `pagamento_ref_hash` |

## Adendo F4 — `ecol_reparticao`

Revisão com o mesmo checklist. Sem achados técnicos:
- **Merkle:** folha (`0x00`) e nó (`0x01`) com prefixos distintos e pares ordenados; o programa sempre calcula a folha a partir de (índice, beneficiário, papel, valor), então um nó interno nunca é aceito como folha (segunda pré-imagem); prova limitada a 16 níveis.
- **Tetos por papel calculados on-chain** a partir de `valor_venda_centavos` do `Credito` (owner `ecol_credito` + discriminator checados; venda atestada pelo intermediador desde a C2) e dos percentuais; resíduo de arredondamento para a EcolChain; uma árvore que prometa mais que o teto de um papel é barrada no resgate (`arvore_gananciosa_nao_passa_do_teto_do_papel`; mutação de controle confirmada).
- **Resgate:** Zupy + beneficiário assinam; beneficiário com cadastro no `ecol_lote` (`seeds::program`) e papel igual ao da folha; bitmap impede resgate duplo; distribuição só depois do repasse atestado pelo intermediador.
- Uma distribuição por crédito (PDA `["dist", credito]`); percentuais copiados na distribuição (mudança posterior do config não afeta); init pela upgrade authority; admin em dois passos; chaves distintas; aritmética checada.

**Riscos residuais (confiança/negócio):**

| # | Sev. | Tema | Mitigação atual / opção |
|---|---|---|---|
| R1 | Média | A divisão **dentro** de cada papel (qual coletor, qual cooperativa recebe quanto) depende da árvore publicada pelo operador; o programa só garante o teto do papel | Árvore gerada por código determinístico (`ecol_reparticao::merkle`) a partir dos lotes do crédito, publicada para auditoria. Opção: co-assinatura de um auditor/validador na criação |
| R2 | Média | O percentual negociado da indústria (65–79%) é informado só pelo operador | Opção: exigir assinatura da(s) indústria(s) do crédito, ou gravar o acordo por indústria |
| R3 | Info | Direitos não resgatados não expiram; o saldo fica com a Zupy | Definir prazo de resgate e destino dos valores não resgatados (contrato com a Zupy) |

## Veredito

Pronto para seguir para F3/F4. **Antes de mainnet:** auditoria externa, fuzzing, `close` de contas (#13), revisão da F3 (#14) e build verificável.
