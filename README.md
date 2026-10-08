# EcolChain

Reciclagem rastreável na Solana, com repartição da receita dos créditos de carbono entre quem participou da cadeia.

- **Estado:** 3 programas Anchor publicados na **devnet** com build verificável, governados por um Squads multisig 3 de 5.
- **Repositório:** https://github.com/ECOLchain/ecolchain-core-chains (programas Solana e clientes TypeScript). A interface web fica num repositório próprio.
- **Contexto e histórico:** [`CONTINUATION.md`](CONTINUATION.md)
- **Decisões de arquitetura:** [`docs/adr/`](docs/adr/README.md)
- **Auditoria:** [`docs/security-audit-2026-09-28.md`](docs/security-audit-2026-09-28.md)

---

## Sumário

1. [Visão geral](#1-visão-geral)
2. [Arquitetura](#2-arquitetura)
3. [Programas](#3-programas)
   - [ecol_lote](#31-ecol_lote)
   - [ecol_credito](#32-ecol_credito)
   - [ecol_reparticao](#33-ecol_reparticao)
4. [Papéis e chaves](#4-papéis-e-chaves)
5. [Fluxo completo, passo a passo](#5-fluxo-completo-passo-a-passo)
6. [Formatos off-chain que precisam bater com o on-chain](#6-formatos-off-chain-que-precisam-bater-com-o-on-chain)
7. [Estrutura do repositório](#7-estrutura-do-repositório)
8. [Ambiente de desenvolvimento](#8-ambiente-de-desenvolvimento)
9. [Build e testes](#9-build-e-testes)
10. [Clientes TypeScript](#10-clientes-typescript)
11. [Deploy na devnet](#11-deploy-na-devnet)
12. [Implantação atual na devnet](#12-implantação-atual-na-devnet)
13. [Governança e operação com o Squads](#13-governança-e-operação-com-o-squads)
14. [Verificação independente (auditores)](#14-verificação-independente-auditores)
15. [Custos](#15-custos)
16. [Segurança](#16-segurança)
17. [Solução de problemas](#17-solução-de-problemas)
18. [Pendências antes da mainnet](#18-pendências-antes-da-mainnet)

---

## 1. Visão geral

**Fluxo físico:** origem do material (coletor, triagem própria, doação, compra ou catador avulso) → Cooperativa (pesa cada **lote de origem**, junta vários num **lote de venda** e pesa de novo) → Transportador → Indústria.

**Marketplace de lotes:** a cooperativa anuncia o lote, as indústrias dão lances e o pagamento fica retido em escrow até a entrega.

**Crédito de carbono:** vários lotes reciclados são agregados num crédito. O crédito é validado e emitido no registro oficial (Verra ou Gold Standard) e depois vendido. A receita da venda é repartida entre os participantes.

| Participante | Parte da receita do crédito | Forma de pagamento |
|---|---|---|
| Coletor | 5% (fixo) | cupons Zupy; além disso recebe R$ via Pix na entrega à cooperativa |
| Cooperativa | 10% (fixo) | cupons Zupy |
| Transportador | 5% (fixo) | cupons Zupy |
| Indústria | 65% a 79% (negociado por crédito) | cupons Zupy |
| EcolChain | o restante: 1% a 15%, mais o resíduo de arredondamento | R$ / stablecoin |

Exemplo do diagrama: um crédito vendido por US$ 50,00 (5.000 centavos) com a indústria em 65% dá **250 / 500 / 250 / 3.250** em cupons e **750** para a EcolChain. Com a indústria em 79%, a indústria recebe 3.950 e a EcolChain 50.

### Princípios de projeto (resumo dos ADRs)

| Decisão | O que significa na prática |
|---|---|
| Escrow em R$ off-chain (ADR 0001) | O dinheiro fica com um **intermediador** (Pix / conta garantia). On-chain ficam só as atestações do intermediador (hash da referência do depósito, da liberação e do reembolso). |
| Sem token próprio (ADR 0002) | A parte da EcolChain é paga em R$ / stablecoin. Não há mint, o que evita questões de CVM. |
| Core asset + PDA de estado (ADR 0003) | Cada lote tem uma PDA `Lote` (a máquina de estados, fonte da verdade) e um **Metaplex Core asset** congelado que serve de recibo visível em carteiras. |
| Carteira embutida + fee payer (ADR 0004) | Cada participante assina com a própria carteira (embutida/MPC). A EcolChain paga as taxas (`payer` separado de quem assina o ato). |
| Crédito só como registro (ADR 0005) | O crédito **não é tokenizado**. On-chain ficam o número de série, os lotes que o originaram, a venda e a prova de aposentadoria. |
| Coletor recebe R$ na entrega (ADR 0006) | Pagamento imediato via Pix, fora da blockchain, mais os 5% em cupons depois. |
| Direito on-chain, Zupy resgata (ADR 0007) | Os direitos de cada participante são publicados como **árvore Merkle**. A Zupy só registra os resgates, e cada resgate fica on-chain. |
| Governança da EcolChain (ADR 0009, substitui a 0008) | Upgrade, configs e árbitro ficam num **Squads multisig 3 de 5**, com as 5 chaves da própria EcolChain. Timelock 0 na devnet e 24 h na mainnet. |
| Lote de origem e lote de venda (ADR 0010) | Cada pesagem na cooperativa é um **lote de origem**, com ou sem coletor. Vários lotes de origem do mesmo material formam um **lote de venda**, pesado de novo e só então anunciado. A união pode ser desfeita antes da venda. O teto dos coletores na repartição vale só sobre o peso que veio de coletores. |
| Importador, Clean Tech, carteiras e variações (ADR 0011) | **Importador de vinhos** registra garrafas postas no mercado (`Distribuicao`) e coletas (`Coleta`) que entrega a uma cooperativa ou **Clean Tech** (papel que opera como a cooperativa); a coleta vira lote de origem `Importador` e, com o lote reciclado, recebe o hash da NF. Cada participante pode ter **várias carteiras com nome** (`Carteira`, PDA `["carteira", endereço]`) que assinam por ele; a identidade continua sendo a carteira titular. Materiais ganham **variações** (`MaterialVariacao`: Transparente, Verde, Marrom no vidro) e **garrafas aproximadas**; o lote de venda não mistura cores. |

**LGPD:** nenhum dado pessoal vai on-chain. A origem de cada lote aparece como `origem_ref` (hash da carteira do coletor, ou do texto da nota, recibo ou CNPJ), o cadastro como `kyc_hash`, e as fotos, tickets e notas do lote de venda como `evidencias_hash`, com os arquivos off-chain. O **nome** do participante e da balança é público (até 32 bytes): para coletores, use apelido ou código.

---

## 2. Arquitetura

### Camadas

```
 ┌──────────────────────────────┐   ┌──────────────────────────────┐   ┌─────────────────────────────┐
 │ Interface web (repo próprio) │   │ Backend / scripts EcolChain  │   │ Balanças (keypair Ed25519)  │
 │ React 19, carteira do        │   │ operador, árvore de direitos,│   │ assinam cada pesagem        │
 │ participante, trilha pública │   │ ops do Squads (onchain/ops)  │   │                             │
 └──────────────┬───────────────┘   └──────────────┬───────────────┘   └──────────────┬──────────────┘
                └───────────────┬──────────────────┴───────────────────────────────────┘
                                ▼
               ┌────────────────────────────────────┐
               │ onchain/clients (TypeScript)       │  clientes Codama (@solana/kit 8), PDAs,
               │                                    │  pesagem Ed25519, referências, árvore Merkle
               └────────────────┬───────────────────┘
                                ▼  RPC Solana
 ┌──────────────────────────────────────────────────────────────────────────────────────────────┐
 │                                   Programas Anchor (este repo)                               │
 └──────────────────────────────────────────────────────────────────────────────────────────────┘
```

A interface web importa os clientes deste repositório (`onchain/clients/src`, alias `@clientes`). Na mesma máquina, as duas pastas ficam lado a lado.

### Programas

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                 Squads multisig (3 de 5)                │
                    │   vault 0 = upgrade authority + admin    vault 1 = árbitro│
                    └──────────────┬───────────────────────────────┬──────────┘
                                   │ admin                          │ árbitro
       ┌───────────────────────────▼──────┐                         │
       │            ecol_lote             │◄────────────────────────┘
       │  GlobalConfig, Participante,     │
       │  Balanca, Material,              │
       │  Entrega (lote de origem),       │
       │  Lote (lote de venda, + asset)   │
       └───────────────▲──────────────────┘
                       │ CPI credito_mark_agregado / _desagregado
                       │ (assinada pela PDA ["autoridade_credito"])
       ┌───────────────┴──────────────────┐
       │           ecol_credito           │
       │  CreditoConfig, Credito,         │
       │  SerialRegistro                  │
       └───────────────▲──────────────────┘
                       │ lê o Credito vendido (valor e pesos)
       ┌───────────────┴──────────────────┐
       │          ecol_reparticao         │
       │  ReparticaoConfig, Distribuicao  │──► lê Participante do ecol_lote no resgate
       └──────────────────────────────────┘

  Off-chain: backend EcolChain (operador), intermediador de pagamento, registrador
  (Verra / GS), Zupy, balanças com keypair próprio, armazenamento de evidências.
```

### Do material ao crédito

```
 Lote de origem (Entrega)          Lote de venda (Lote)                              Crédito
 ┌───────────────────────┐
 │ Coletor | Triagem     │  união  ┌──────────────────────────┐  leilão, venda,  ┌───────────────┐
 │ Doação | Compra |     ├────────►│ EmMontagem ─► pesagem do │  retirada,       │ Credito       │
 │ Avulso; pesado pela   │ (mesma  │ consolidado ─► Criado    ├─────────────────►│ (vários lotes │
 │ balança da cooperativa│  coop., │ (Core asset nasce aqui)  │  recebimento,    │  do mesmo     │
 └───────────────────────┘ mesmo   └──────────────────────────┘  reciclado       │  material)    │
                           material)        │ desfazer antes da venda             └───────────────┘
                                            ▼
                                  EmDesmontagem ─► Desfeito (origens voltam a ficar disponíveis)
```

A trilha é navegável nos dois sentidos: do crédito aos lotes de venda (`lotes_hash`), deles aos lotes de origem (`Entrega.lote`, `entregas_hash`) e à origem (`origem_ref`); e de um lote de origem até o crédito (`Lote.credito`).

- **Estado canônico em PDAs.** O Core asset é derivado do lote (`["asset", lote]`) e não carrega estado.
- **Todo evento tem número de sequência** (`seq`) por lote, crédito ou distribuição. Uma lacuna na sequência é detectável por auditores.
- **Cada chave de config tem um único papel.** Os programas rejeitam configs em que a mesma chave acumula dois papéis ou é nula, e uma chave de papel não pode ser cadastrada como participante.
- **Layout compatível.** Campos novos ocupam o espaço reservado das contas (zero significa o comportamento antigo) e variantes de enum só são acrescentadas no fim, para que contas existentes continuem legíveis depois de um upgrade.

---

## 3. Programas

| Programa | Program ID | Instruções |
|---|---|---|
| `ecol_lote` | `AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD` | 33 |
| `ecol_credito` | `FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ` | 15 |
| `ecol_reparticao` | `HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K` | 10 |

**Convenção de nomes:** `<papel>_<ação>`. O prefixo indica quem assina: `admin_`, `operador_`, `cooperativa_`, `industria_`, `transportador_`, `intermediador_`, `arbitro_`, `registrador_`, `zupy_`, `parte_` (qualquer parte do lote), `credito_` (só via CPI do `ecol_credito`) e `public_` (cranks que qualquer um pode chamar).

**Inicialização protegida:** `admin_init_*` exige a assinatura da **upgrade authority** do programa, para ninguém se adiantar ao deploy e criar a config.

**Guardião (nos 3 programas):** `admin_set_guardiao(guardiao)` define quem pode pausar sem timelock; `guardiao_pausar` só **restringe** (no `ecol_lote`: Normal → SomenteDisputas → Pausado; nos outros: pausado = true). Voltar ao normal é sempre do admin. Detalhes na seção 13.

**Troca de admin em dois passos:** `admin_propose_admin(novo)` e depois `admin_accept_admin` assinado pelo novo admin. Propor `Pubkey::default()` cancela uma proposta pendente.

### 3.1 `ecol_lote`

Registra o material desde a origem: os lotes de origem pesados na cooperativa, a união deles em lotes de venda e cada etapa da venda, da logística e do consumo, com quem assinou.

#### Contas (PDAs)

| Conta | Seeds | Conteúdo |
|---|---|---|
| `GlobalConfig` | `["global"]` | admin, árbitro, operador, intermediador, guardião, `credito_autoridade`, prazo de entrega, tolerância de peso, faixa de peso da montagem (`perda_montagem_bps`, `excesso_montagem_bps`; zero = padrão −10% / +2%), modo, admin pendente |
| `Participante` | `["part", carteira]` | papel (Coletor, Cooperativa, Transportador, Indústria), ativo, `kyc_hash`, nome (até 32 bytes UTF-8, público). **Uma carteira tem um único papel, que não muda.** |
| `Balanca` | `["balanca", dispositivo]` | dono (cooperativa ou indústria), chave Ed25519 do equipamento, ativa, nome (até 32 bytes) |
| `Material` | `["material", codigo (u16 LE)]` | código, nome (até 32 bytes), ativo. **Cadastrado pelo operador**; lotes e créditos guardam só o código |
| `Entrega` | `["entrega", cooperativa, entrega_id]` | **lote de origem**: `origem` (Coletor, TriagemPropria, Doacao, Compra, Avulso; contas antigas = Coletor), `origem_ref` (hash, LGPD), material, peso assinado pela balança, balança, horário e o lote de venda em que entrou (vazio enquanto disponível) |
| `Lote` | `["lote", cooperativa, lote_id]` | **lote de venda**: material, peso consolidado, lotes de origem vinculados (quantidade, soma dos pesos, `peso_sem_coletor_g`, hash encadeado), evidências (nota ou recibo), partes, valor, hashes do escrow, prazo, estado, `asset_queimavel`, `seq`... |
| Core asset | `["asset", lote]` | recibo do lote de venda (Metaplex Core), criado no fechamento |
| Autoridade do asset | `["autoridade_asset"]` | PDA sem dados; é update authority e dona dos plugins permanentes do asset |

**Materiais:** cadastro on-chain feito pelo operador (`operador_create_material`), sem lista fixa no código. Material inativo não entra em lotes nem créditos novos; lotes existentes não são afetados.

#### Máquina de estados do lote

```
EmMontagem ──► Criado ──► Anunciado ──► Vendido ──► EmTransporte ──► Recebido ──► Reciclado ──► Agregado
    │            │          │   ▲          │             │               │                        │
    │            │          ▼   │          └─────────────┴───────┬───────┘                        │
    │            │        SemLance                               ▼                                ▼
    │            │          │                               EmDisputa ──► (etapa de origem)    Reciclado
    ▼            ▼          ▼                                    │   ──► Reciclado (só se origem = Recebido)
    └──────► EmDesmontagem ─► Desfeito (terminal)                └─────► Reembolsado (terminal)
```

| Transição | Instrução | Quem assina | Condições |
|---|---|---|---|
| (lote de origem) | `cooperativa_register_entrega(dados)` | cooperativa + payer | `dados`: `entrega_id`, `origem`, `origem_ref` (não nulo), material ativo, `peso_g`, `ts_pesagem`. A balança **ativa da própria cooperativa** assina `(domínio, Entrega, entrega, peso, ts)` na instrução Ed25519 anterior. |
| → `EmMontagem` | `cooperativa_create_lote(lote_id, material, evidencias_hash)` | cooperativa + payer | material cadastrado e **ativo**; sem pesagem e sem asset ainda |
| (montagem, em `EmMontagem`) | `cooperativa_add_entregas` | cooperativa | até 10 lotes de origem por transação em `remaining_accounts` (graváveis): da mesma cooperativa, do mesmo material e ainda não vinculados. Atualiza a quantidade, a soma dos pesos, `peso_sem_coletor_g` e `entregas_hash = sha256(h ‖ entrega ‖ peso_g)`. |
| `EmMontagem` → `Criado` | `cooperativa_fechar_lote(peso_g, ts_pesagem, uri)` | cooperativa + payer | pelo menos um lote de origem; pesagem Ed25519 do **consolidado** pela balança ativa da cooperativa (timestamp a no máximo 15 min); peso entre **soma − 10%** e **soma + 2%** (`PesoConsolidadoForaDaFaixa`; limites no `GlobalConfig`). Cria o Core asset congelado e queimável em nome da cooperativa. |
| `EmMontagem`/`Criado`/`SemLance` → `EmDesmontagem` → `Desfeito` | `cooperativa_desmontar_lote` | cooperativa | até 10 lotes de origem por transação em `remaining_accounts`, que voltam a ficar disponíveis. Com o último, o lote vai a `Desfeito` e o Core asset é queimado. Nunca depois da venda. |
| `Criado`/`SemLance` → `Anunciado` | `cooperativa_list_lote(prazo_leilao, preco_minimo_centavos)` | cooperativa | preço > 0; prazo no futuro e no máximo 90 dias |
| `Anunciado` → `SemLance` | `cooperativa_cancel_anuncio` | cooperativa | a qualquer momento (reprecificar ou desistir) |
| `Anunciado` → `SemLance` | `operador_close_leilao_sem_lance` | operador | só depois do `prazo_leilao` |
| `Anunciado` → `Vendido` | `industria_accept_venda(valor_centavos, escrow_ref_hash, lances_root)` | **indústria + operador + intermediador** | valor ≥ preço mínimo; o operador atesta o vencedor e a raiz Merkle dos lances; o intermediador atesta o depósito do escrow. Fixa o prazo de entrega e a tolerância de peso vigentes. |
| `Vendido` → `EmTransporte` | `transportador_pickup_lote` | **transportador + cooperativa** | passagem de custódia; o Core asset vai para o transportador |
| `EmTransporte` → `Recebido` | `industria_confirm_recebimento(peso_recebido_g, ts_pesagem?)` | indústria | com `ts_pesagem`, exige a pesagem Ed25519 de uma balança da indústria. Se o peso divergir além da tolerância, vai **direto para `EmDisputa`** (`DivergenciaPeso`). O asset vai para a indústria. |
| `Recebido` → `Reciclado` | `intermediador_confirm_liberacao(liberacao_ref_hash)` | intermediador | atesta que liberou o escrow à cooperativa (consumo total do lote de venda) |
| `Vendido`/`EmTransporte`/`Recebido` → `EmDisputa` | `parte_open_disputa(motivo)` | cooperativa, indústria ou transportador do lote | não pode reabrir disputa numa etapa já julgada com "prosseguir" |
| `Vendido`/`EmTransporte` → `EmDisputa` | `public_crank_prazo_expirado` | qualquer um | prazo de entrega vencido; só em modo `Normal` e depois de 48 h de carência se o programa saiu de uma pausa |
| `EmDisputa` → origem / `Reciclado` / `Reembolsado` | `arbitro_resolve_disputa(decisao)` | árbitro (+ intermediador quando há dinheiro) | ver decisões abaixo |
| `Reciclado` → `Agregado` | `credito_mark_agregado(credito)` | PDA `["autoridade_credito"]` do `ecol_credito` (CPI) | — |
| `Agregado` → `Reciclado` | `credito_mark_desagregado(credito)` | idem (CPI) | lote pertence a esse crédito e o crédito foi rejeitado ou abandonado |

**Decisões do árbitro (`DecisaoDisputa`):**
- `Prosseguir { novo_peso_g }`: volta à etapa de origem, opcionalmente reclassificando o peso. Renova o prazo se a disputa foi antes da entrega. Essa etapa não pode ser disputada de novo.
- `Liberar { valor_liberado_centavos, novo_peso_g, ref_hash }`: só para disputa aberta no recebimento. Encerra liberando `valor_liberado_centavos` à cooperativa (total ou parcial) e reembolsando o restante à indústria. O intermediador co-assina.
- `Reembolsar { ref_hash }`: encerra em `Reembolsado`. O intermediador co-assina atestando a devolução.

**Motivos de disputa:** `DivergenciaPeso`, `ClassificacaoContestada`, `FalhaEntrega`, `PrazoExpirado`, `Outro`.

#### Cadastro e administração

| Instrução | Quem assina | Efeito |
|---|---|---|
| `admin_init_global(admin, params)` | payer + upgrade authority | cria o `GlobalConfig` |
| `admin_update_global(params)` | admin | altera árbitro, operador, intermediador, `credito_autoridade`, prazo de entrega (≤ 180 dias) e tolerância de peso (≤ 20%) |
| `admin_set_modo(modo)` | admin | `Normal`, `SomenteDisputas` (só disputas, crank, árbitro, liberação do escrow e desagregação) ou `Pausado` (nenhuma transição de lote) |
| `admin_set_tolerancia_montagem(perda_bps, excesso_bps)` | admin | faixa aceita no fechamento do lote de venda (cada limite > 0 e ≤ 30%) |
| `admin_set_guardiao(guardiao)` / `guardiao_pausar(modo)` | admin / guardião | pausa de emergência (seção 13) |
| `admin_propose_admin` / `admin_accept_admin` | admin / novo admin | troca de admin em dois passos |
| `operador_register_participante(carteira, papel, kyc_hash, nome)` | operador + payer | cadastra a carteira com um papel e um nome |
| `operador_set_participante_nome(nome)` | operador | preenche ou corrige o nome (cadastros anteriores ao campo ficam sem nome) |
| `operador_set_participante_ativo(ativo)` | operador | participante inativo não assume novos compromissos |
| `operador_register_balanca(dispositivo, nome)` | operador + payer | balança de uma cooperativa ou indústria ativa |
| `operador_set_balanca_nome(nome)` | operador | preenche ou corrige o nome da balança |
| `operador_set_balanca_ativa(ativa)` | operador | desativa equipamento trocado, chave comprometida ou aferição vencida |
| `operador_create_material(codigo, nome)` | operador + payer | cadastra um tipo de material; o código é único e não muda |
| `operador_update_material(nome, ativo)` | operador | renomeia, ativa ou desativa um material |

#### Core asset (recibo)

- Criado no **fechamento do lote de venda** com `PermanentFreezeDelegate` (congelado), `PermanentTransferDelegate` e `PermanentBurnDelegate`, todos com a PDA `["autoridade_asset"]` como autoridade. Os lotes de origem não têm asset (só PDAs, mais baratos).
- **O dono não consegue transferir.** Só o programa move o asset, acompanhando a custódia: cooperativa → transportador (retirada) → indústria (recebimento).
- Ao desfazer a união, o programa queima o asset (funciona mesmo congelado). Lotes fechados antes da ADR 0010 não têm o plugin de queima (`asset_queimavel = false`).
- Sem plugin `Attributes`: o estado fica na PDA `Lote`.

### 3.2 `ecol_credito`

Registra o crédito de carbono e a ligação dele com os lotes físicos. **Não tokeniza.**

#### Contas (PDAs)

| Conta | Seeds | Conteúdo |
|---|---|---|
| `CreditoConfig` | `["credito_config"]` | admin, operador, registrador, intermediador, pausado |
| `Credito` | `["credito", credito_id (u64 LE)]` | padrão (Verra, GoldStandard), material, vintage, hash da metodologia, quantidade e peso dos lotes, `peso_sem_coletor_g`, `lotes_hash`, tCO2e × 1000, comprador, valor, hash do pagamento, prazo de formação, prova de aposentadoria, serial, estado, `seq` |
| `SerialRegistro` | `["serial", padrão, sha256(serial)]` | garante que um serial do registro é usado por um único crédito (**contra dupla contagem**) |
| Autoridade | `["autoridade_credito"]` | PDA que assina as CPIs no `ecol_lote`. Deve ser igual à `credito_autoridade` do `GlobalConfig`. |

#### Máquina de estados do crédito

```
EmFormacao ──► EmValidacao ──► Emitido ──► Vendido ──► Aposentado
    │                │
    ▼                ▼
Abandonado       Rejeitado          (nesses dois estados os lotes podem ser liberados)
```

| Transição | Instrução | Quem assina | Condições |
|---|---|---|---|
| → `EmFormacao` | `operador_create_credito(credito_id, padrao, material, vintage, metodologia_hash)` | operador + payer | material cadastrado e ativo; vintage ≥ 2000; prazo de formação = agora + 180 dias |
| (adiciona lotes) | `operador_add_lotes` | operador | até **10 lotes por transação** em `remaining_accounts` (graváveis), todos `Reciclado` e **do mesmo material** do crédito. Cada lote vira `Agregado` via CPI. Atualiza `lotes_hash = sha256(h ‖ lote ‖ peso_g)` e soma o peso sem coletor de cada lote, proporcional ao peso consolidado: `floor(peso_g × peso_sem_coletor_g / peso_entregas_g)`. |
| `EmFormacao` → `EmValidacao` | `operador_submit_credito` | operador | pelo menos 1 lote |
| `EmValidacao` → `Emitido` | `registrador_record_emissao(serial, serial_hash, tco2e_milli)` | registrador + payer | serial ≤ 64 bytes, `serial_hash = sha256(serial)`, tCO2e > 0; cria o `SerialRegistro` |
| `EmValidacao` → `Rejeitado` | `registrador_record_rejeicao(motivo_hash)` | registrador | — |
| `Emitido` → `Vendido` | `operador_record_venda(valor_centavos, pagamento_ref_hash)` | **operador + comprador + intermediador** | o intermediador atesta o recebimento do valor, que é a base da repartição |
| `Vendido` → `Aposentado` | `registrador_record_aposentadoria(prova_hash)` | registrador | aposentado no registro em nome do comprador |
| `EmFormacao` → `Abandonado` e liberação | `public_release_lotes` | qualquer um | se o crédito está `EmFormacao` há mais de 180 dias, é declarado `Abandonado`. Em `Rejeitado`/`Abandonado`, devolve os lotes passados em `remaining_accounts` a `Reciclado` via CPI. Assim a EcolChain não consegue prender lotes indefinidamente. |

**Administração:** `admin_init_config(admin, operador, registrador, intermediador)`, `admin_update_config(operador, registrador, intermediador)`, `admin_set_pausado`, `admin_propose_admin` / `admin_accept_admin`.

### 3.3 `ecol_reparticao`

Para cada crédito vendido, publica **quanto cada participante tem direito a receber** e registra cada resgate em cupons.

#### Contas (PDAs)

| Conta | Seeds | Conteúdo |
|---|---|---|
| `ReparticaoConfig` | `["rep_config"]` | admin, operador, zupy, intermediador, percentuais (bps), pausado |
| `Distribuicao` | `["dist", credito]` | receita, cópia dos percentuais, % da indústria, **teto por papel**, resgatado por papel, parte da EcolChain, raiz Merkle, nº de folhas, estado, hashes dos repasses, bitmap de resgates |

**Percentuais (basis points, 10.000 = 100%):** coletor 500, cooperativa 1.000, transportador 500, indústria de 6.500 a 7.900. A config é validada on-chain: os três fixos precisam ser maiores que zero, e `fixos + indústria_max + 1%` não pode passar de 100%, ou seja, a EcolChain nunca fica abaixo de 1%. Mudar os percentuais **não afeta distribuições já criadas**, porque cada uma guarda uma cópia.

#### Cálculo dos tetos

```
teto(papel)   = floor(receita × bps(papel) / 10.000)     para cooperativa, transportador, indústria
teto(coletor) = floor(receita × bps(coletor) × (peso_total − peso_sem_coletor) / (10.000 × peso_total))
EcolChain     = receita − soma dos 4 tetos                  (inclui o resíduo de arredondamento)
```

O teto dos coletores vale só sobre o peso que veio de coletores; a parte do material sem coletor (triagem, doação, compra, avulso) fica com a EcolChain. Créditos sem pesos (anteriores à ADR 0010) mantêm a fórmula integral.

A soma sempre fecha exatamente com a receita. Isso é verificado por testes de propriedade com 5.000 casos.

#### Estados e instruções

`Pendente → Financiada`

| Instrução | Quem assina | Efeito |
|---|---|---|
| `operador_create_distribuicao(bps_industria, merkle_root, num_folhas)` | operador + payer | exige crédito `Vendido`; a receita vem do próprio `Credito`. Calcula os tetos. Máximo de 4.096 folhas. |
| `intermediador_record_repasse(zupy_ref_hash, ecolchain_ref_hash)` | intermediador | atesta que repassou a parte dos cupons à Zupy e a parte da EcolChain → `Financiada` |
| `zupy_record_resgate(indice, papel, valor_centavos, prova, recibo_hash)` | **Zupy + beneficiário** | só em `Financiada`. A folha é provada contra a raiz (até 16 níveis). O beneficiário precisa estar cadastrado no `ecol_lote` **com o papel da folha**. Cada folha resgata uma vez (bitmap) e o total do papel não passa do teto. |

**Administração:** `admin_init_config(admin, params)`, `admin_update_config(params)`, `admin_set_pausado`, `admin_propose_admin` / `admin_accept_admin`.

---

## 4. Papéis e chaves

| Papel | Onde está configurado | O que faz |
|---|---|---|
| **admin** | as 3 configs | altera configs, pausa, troca papéis. Na devnet é o **vault 0** do Squads. |
| **árbitro** | `GlobalConfig` | julga disputas de lote. Na devnet é o **vault 1** do Squads. |
| **guardião** | as 3 configs | **só pausa**, na hora, sem timelock. É o vault de um segundo Squads **1 de 5** com os mesmos membros: qualquer pessoa da EcolChain pausa sozinha. Não despausa nem muda nada. |
| **operador** | as 3 configs | **administração do dia a dia** (menu "Administração" na interface): cadastra participantes, balanças e materiais, encerra leilões, monta créditos, registra vendas, publica árvores de direitos. Uma chave só |
| **intermediador** | as 3 configs | intermediador de pagamento: atesta depósito, liberação e reembolso de escrow, recebimento da venda do crédito e repasses |
| **registrador** | `CreditoConfig` | atesta o que aconteceu no registro oficial: emissão, rejeição, aposentadoria |
| **zupy** | `ReparticaoConfig` | plataforma de cupons: co-assina os resgates |
| **credito_autoridade** | `GlobalConfig` | PDA `["autoridade_credito"]` do `ecol_credito` |
| Coletor, Cooperativa, Clean Tech, Transportador, Indústria, Importador | `Participante` | participantes cadastrados, identificados pela carteira titular; outras carteiras (`Carteira`, com nome) podem assinar por eles (ADR 0011) |
| Balança | `Balanca` | equipamento com keypair Ed25519 próprio, que assina cada pesagem |
| payer | — | paga taxas e rent; separado de quem assina o ato (a EcolChain pode pagar por todos) |

---

## 5. Fluxo completo, passo a passo

Esse é o ciclo que o E2E (`onchain/clients/e2e/ciclo-completo.test.ts`) executa no Surfpool.

**Preparação (uma vez)**
1. Deploy dos 3 programas e `admin_init_*` de cada um (feito pelo script de deploy).
2. `operador_register_participante` para cada cooperativa, transportador, indústria e coletor.
3. `operador_register_balanca` para as balanças das cooperativas e indústrias.
4. `operador_create_material` para cada tipo de material aceito.

Na interface web, os passos 2 a 4 ficam no menu **Administração** (Participantes, Balanças, Materiais).

**Lotes de origem**
5. O material chega à cooperativa: de um coletor (que recebe em R$ via Pix, fora da blockchain, e mostra o QR Code da carteira na interface), de triagem própria, doação, compra ou catador avulso. A balança pesa e assina `(domínio, Entrega, entrega, peso_g, ts)`; o cliente monta `[Ed25519, cooperativa_register_entrega]`. Cada pesagem é um lote de origem.

**Lote de venda**
6. `cooperativa_create_lote` abre o lote de venda em `EmMontagem`.
7. `cooperativa_add_entregas` vincula os lotes de origem do mesmo material (até 10 por transação). A balança pesa o consolidado e assina `(domínio, Origem, lote, peso_g, ts)`; `[Ed25519, cooperativa_fechar_lote]` fecha o lote (`Criado`) e cria o asset. Até a venda, `cooperativa_desmontar_lote` desfaz a união.
8. `cooperativa_list_lote`. Estado: `Anunciado`.
9. O leilão é apurado off-chain. `industria_accept_venda` com as três assinaturas (indústria, operador e intermediador). Na interface, cada um assina no seu aparelho, e o código passa por QR ou copiar e colar; a transação usa um nonce durável do operador, então não expira enquanto circula. Estado: `Vendido`.
10. `transportador_pickup_lote`, assinado pela cooperativa e pelo transportador. Na interface, a cooperativa assina e mostra um QR Code; o aparelho do transportador remonta a transação, confere a assinatura, assina e envia. Estado: `EmTransporte`, com o asset passando para o transportador.
11. `industria_confirm_recebimento`, com pesagem assinada pela balança da indústria. Estado: `Recebido` (ou `EmDisputa` se o peso divergir), com o asset passando para a indústria.
12. `intermediador_confirm_liberacao`. Estado: `Reciclado`.

**Crédito**
13. `operador_create_credito`, depois `operador_add_lotes` (em lotes de até 10) e `operador_submit_credito`.
14. `registrador_record_emissao`. Estado: `Emitido`.
15. `operador_record_venda` (operador + comprador + intermediador). Estado: `Vendido`.
16. `registrador_record_aposentadoria`. Estado: `Aposentado`.

**Repartição**
17. O backend gera a árvore de direitos com `montarDireitos` (`clients/src/arvore.ts`) e chama `operador_create_distribuicao`.
18. O intermediador repassa os valores à Zupy e à EcolChain e chama `intermediador_record_repasse`. Estado: `Financiada`.
19. Cada beneficiário resgata com a Zupy: `zupy_record_resgate` com a prova Merkle.

---

## 6. Formatos off-chain que precisam bater com o on-chain

### Mensagem assinada pela balança (Ed25519)

66 bytes, assinados pela chave do equipamento:

| Bytes | Campo |
|---|---|
| 0..17 | domínio `"ECOLCHAIN:PESO:v1"` |
| 17 | tipo: `0` = Origem (lote de venda consolidado, no fechamento), `1` = Recebimento (indústria), `2` = Entrega (lote de origem, cooperativa) |
| 18..50 | endereço do `Lote` (ou da `Entrega`, no tipo 2) |
| 50..58 | `peso_g` (u64 LE) |
| 58..66 | `ts` (i64 LE, unix) |

A instrução do programa Ed25519 (uma assinatura, dados dentro da própria instrução) deve vir **imediatamente antes** da instrução do `ecol_lote`. O timestamp precisa estar a no máximo 15 min do relógio do cluster, para mais ou para menos. Implementação no cliente: `onchain/clients/src/pesagem.ts`.

### Árvore Merkle de direitos

```
folha = sha256(0x00 ‖ indice (u32 LE) ‖ beneficiario (32) ‖ papel (u8) ‖ valor_centavos (u64 LE))
nó    = sha256(0x01 ‖ menor ‖ maior)            (pares ordenados: a prova não precisa de bits de direção)
nível ímpar: o último nó sobe sem par
papel: 0 = Coletor, 1 = Cooperativa, 2 = Transportador, 3 = Indústria
```

- **Rust:** `ecol_reparticao::merkle`
- **TypeScript:** `onchain/clients/src/arvore.ts` (há um vetor de teste compartilhado que garante hashes iguais)

**Divisão dentro de cada papel (`montarDireitos`):** coletores na proporção do peso dos **lotes de origem com coletor** vinculados aos lotes do crédito; cooperativa, transportador e indústria na proporção do peso dos **lotes**. Arredonda para baixo e distribui os centavos restantes pelo maior resto; no empate, vence o endereço menor. As folhas são ordenadas por papel e endereço, então qualquer auditor reconstrói a raiz a partir dos lotes, dos lotes de origem e do mapeamento `origem_ref → carteira` dos coletores.

### Referência da origem (lote de origem) e do lote de venda

Implementação: `onchain/clients/src/coletor.ts`.

- **Com coletor:** `origem_ref = sha256("ECOLCHAIN:COLETOR:v1" ‖ carteira_do_coletor)` (`coletorRef`). Determinística: a repartição liga cada lote de origem à carteira que recebe os cupons sem tabela extra. O cadastro do coletor (nome civil, CPF) fica off-chain, com só o hash do KYC no `Participante`. A carteira é pseudônima e já apareceria de qualquer forma nos resgates.
- **Sem coletor (doação, compra, triagem, avulso):** `origem_ref = sha256("ECOLCHAIN:ORIGEM:v1" ‖ texto normalizado)` (`origemRef`), onde o texto é a nota, o recibo ou o CNPJ em maiúsculas, sem espaços nas pontas e com espaços internos únicos. Sem texto, a interface usa `<tipo>:<cooperativa>:<id>`.
- **Lote de venda:** com nota fiscal ou recibo, `evidencias_hash = origemRef(texto)`, o mesmo formato.

O texto nunca vai on-chain. Quem o tem (o doador, com o recibo) recalcula o hash e encontra o lote na trilha pública, por `memcmp` em `Entrega.origem_ref` (offset 48) ou `Lote.evidencias_hash` (offset 110).

### Hash encadeado das entregas do lote

`entregas_hash` começa em zero e, a cada entrega vinculada, vira `sha256(entregas_hash ‖ endereço_da_entrega ‖ peso_g (u64 LE))`. Dá para reconstruir a partir dos eventos `EntregaNoLote`. O lote guarda também `peso_entregas_g` (soma dos lotes de origem) ao lado de `peso_g` (pesagem do consolidado); a diferença mostra perdas na triagem e é limitada pela faixa do fechamento. Ao desfazer a união, cada origem devolvida emite `EntregaDevolvida`.

### Hash encadeado dos lotes do crédito

`lotes_hash` começa em zero e, a cada lote agregado, vira `sha256(lotes_hash ‖ endereço_do_lote ‖ peso_g (u64 LE))`. Dá para reconstruir a partir dos eventos `LoteAgregado`.

---

## 7. Estrutura do repositório

```
README.md                       este arquivo
CONTINUATION.md                 contexto, decisões e histórico das sessões
docs/
  Proposta_03.excalidraw        diagrama original
  Proposta_04.excalidraw        diagrama com as decisões de 2026-09-28 (abrir em https://excalidraw.com)
  adr/                          10 decisões de arquitetura
  security-audit-2026-09-28.md  auditoria (checklists do solana-ai-kit) e achados corrigidos
onchain/                        workspace Anchor
  Anchor.toml                   program IDs (localnet e devnet)
  rust-toolchain.toml           Rust 1.89.0
  programs/
    ecol_lote/src/
      lib.rs                    ponto de entrada (instruções)
      state/                    GlobalConfig, Participante, Balanca, Material, Entrega, Lote (+ testes da máquina de estados)
      instructions/             admin, participantes, balancas, materiais, entregas, lote_criacao (abrir, fechar, desfazer), marketplace, logistica, disputa, credito
      pesagem.rs                verificação da assinatura Ed25519 da balança
      ativo.rs                  Core asset (criar, transferir e queimar)
      events.rs, error.rs, constants.rs
      tests/                    testes LiteSVM
    ecol_credito/               state/, instructions/ (admin, agregacao, ciclo), tests/ (+ tests/cenario/mod.rs reutilizável)
    ecol_reparticao/            state/, instructions/ (admin, direitos), merkle.rs, tests/
  tests/fixtures/mpl_core.so    programa Metaplex Core carregado nos testes LiteSVM
  clients/                      clientes TypeScript (ver seção 10)
    src/generated/              clientes gerados pelo Codama (não editar)
    src/pdas.ts, pesagem.ts, arvore.ts, coletor.ts, nome.ts
    scripts/gerar-clientes.ts   geração Codama
    scripts/init-devnet.ts      inicialização das configs na devnet
    scripts/cadastrar-participante.ts   cadastro de uma carteira na devnet
    scripts/rede-local.ts       rede local de demonstração (Surfpool) para testar a interface
    test/                       testes do gerador da árvore
    e2e/                        ciclo completo no Surfpool
  ops/                          operação da devnet (SDK do Squads, web3.js v1 isolado aqui)
    comum.ts                    chaves de teste, RPC
    chaves-devnet.ts            gera ou reaproveita as chaves de teste
    squads-devnet.ts            cria o multisig
    proposta.ts, propor-upgrade.ts, trocar-operador.ts, pausar.ts, retomar.ts, ...
  scripts/deploy-devnet.sh      deploy verificável completo
  .devnet/                      chaves de teste e squads.json (FORA DO GIT)
  target/deploy/*-keypair.json  keypairs dos programas (FORA DO GIT)
web/                            interface web: repositório próprio (FORA DESTE GIT)
```

---

## 8. Ambiente de desenvolvimento

| Ferramenta | Versão usada | Instalação / observação |
|---|---|---|
| Rust | 1.89.0 | fixado em `onchain/rust-toolchain.toml` (o `rustup` instala sozinho) |
| Solana CLI (Agave) | 3.1.10 | `sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"` |
| Anchor CLI | 1.1.2 | via `avm` |
| Node.js | ≥ 20.18 (testado com 24.10) | clientes e scripts de operação |
| Surfpool | 1.6.0 | só para o E2E (o pacote npm `@solana/surfpool` embarca o binário) |
| Docker | 29.x testado | necessário para `anchor build --verifiable` |
| `solana-verify` | — | `cargo install solana-verify` |

**WSL:** o Docker Desktop precisa da integração ativada para a distro: *Settings → Resources → WSL integration*.

**Carteira local:** `~/.config/solana/id.json` (crie com `solana-keygen new` se não existir). Para ver o endereço, use `solana address`.

**Carteira dos participantes (interface web): use a [Solflare](https://solflare.com).** Ela assina sem alterar a transação e permite assinar sem enviar, o que a retirada do lote exige: a cooperativa e o transportador assinam a mesma transação em aparelhos diferentes. Carteiras que acrescentam instruções ao assinar invalidam a assinatura da outra parte. Detalhes no README do [repositório da web](https://github.com/ECOLchain/ecolchain-core-web).

A variável `NO_DNA=1` usada nos comandos do Anchor só desliga a telemetria e as mensagens interativas do CLI.

---

## 9. Build e testes

```bash
cd onchain

NO_DNA=1 anchor build                        # compila os 3 programas; IDLs em target/idl, .so em target/deploy
NO_DNA=1 anchor test                         # = cargo test (skip_local_validator = true)
cargo test -p ecol_lote                      # só um programa
cargo test -p ecol_credito -- venda          # filtrar por nome
cargo clippy --all-targets -- -D warnings    # lint estrito (deve ficar limpo)
cargo fmt --check
```

Os testes LiteSVM carregam os `.so` de `target/deploy` e o `tests/fixtures/mpl_core.so`, então **rode `anchor build` antes**.

| Tipo | Onde | O que cobre |
|---|---|---|
| Unitários | `programs/*/src/state/*.rs` | matriz completa de transições (todo par estado × transição), divergência de peso na fronteira, exemplo US$ 50, resíduo de arredondamento, bitmap e teto |
| Propriedade (proptest, 5.000 casos) | idem | sequências aleatórias respeitam o grafo de estados; tolerância simétrica e monotônica; repartição sempre fecha e respeita os percentuais |
| Integração LiteSVM | `programs/*/tests/` | fluxo, autorização (cada papel errado falha), pesagem Ed25519 e asset, regressões da auditoria, compute units, CPI entre os 3 programas, tempo via `Clock` |
| TS | `clients/test/` | gerador da árvore (vetor compartilhado com o Rust) |
| E2E | `clients/e2e/` | ciclo completo no Surfpool, com time travel e crank |

Total atual: **146 testes Rust, 6 TS e 2 E2E**.

---

## 10. Clientes TypeScript

Clientes `@solana/kit` 8 gerados pelo [Codama](https://github.com/codama-idl/codama) a partir das IDLs.

```bash
cd onchain/clients
npm install
npm run gerar       # regenera src/generated a partir de ../target/idl (rodar após anchor build)
npm run typecheck
npm test            # gerador da árvore
npm run e2e         # Surfpool embarcado: lote → crédito → repartição → resgates
```

| Módulo | Uso |
|---|---|
| `src/generated/<programa>` | builders de instrução, decoders de conta e tipos (não editar) |
| `src/pdas.ts` | derivação de todas as PDAs (espelha as seeds do Rust) |
| `src/pesagem.ts` | monta a instrução Ed25519 da balança e anexa as contas graváveis |
| `src/arvore.ts` | totais por papel (`calcularTotais`, com o teto proporcional dos coletores) e árvore de direitos (`montarDireitos`) |
| `src/coletor.ts` | referências da origem: `coletorRef`, `origemRef`, `normalizarReferencia` |
| `src/nome.ts` | nome fixo de 32 bytes do participante e da balança (`bytesDoNome`, `lerNomeFixo`) |

**Transações v0:** o `@solana/kit-plugin-rpc` 0.18, exigido pelo `@solana/surfpool` 1.6, ainda não monta v1. Quando ele passar a montar, dá para migrar para v1.

**Após mudar um programa:** rode `anchor build`, depois `npm run gerar`, `npm run typecheck` e os testes.

---

## 11. Deploy na devnet

O script `onchain/scripts/deploy-devnet.sh` faz o deploy completo e aborta na primeira falha (`set -euo pipefail`).

### O que o script faz, em ordem

| # | Etapa | Detalhe |
|---|---|---|
| 1 | Checagens | recusa RPC com "mainnet"; exige Docker, `solana-verify`, carteira e **≥ 11 SOL** |
| 2 | Confirmação | pergunta `Publicar na devnet? [s/N]` (pule com `CONFIRMAR=1`) |
| 3 | Build verificável | `anchor build --verifiable` na imagem `quay.io/ottersec/anchor:v1.1.2` → `target/verifiable/*.so` |
| 4 | Chaves de teste | `ops/chaves-devnet.ts`: operador, intermediador, registrador, zupy e 5 membros do Squads em `onchain/.devnet/` |
| 5 | Squads | `ops/squads-devnet.ts`: multisig v4, 5 membros, quórum 3, chaves da EcolChain, **sem config authority** (mudar membros também exige 3 de 5), timelock 0 (ADR 0009) |
| 6 | Deploy | `solana program deploy` dos 3 `.so` verificáveis |
| 7 | Configs | `clients/scripts/init-devnet.ts`: `admin_init_*` dos 3 programas, **antes** de transferir a upgrade authority (a inicialização exige a assinatura dela) |
| 8 | Verificação | hash do executável on-chain = hash do build verificável, para cada programa |
| 9 | Upgrade authority | `solana program set-upgrade-authority` dos 3 para o vault 0 |

Os passos 4, 5 e 7 são **idempotentes**: se o script parar no meio, dá para rodar de novo. Mas, depois do passo 9, um novo deploy com esse script falha, porque a carteira local deixou de ser upgrade authority. Upgrades passam a ser feitos pelo Squads (seção 13).

### Parâmetros iniciais das configs (devnet)

| Config | Valor |
|---|---|
| `admin` (3 programas) | vault 0 do Squads |
| `arbitro` | vault 1 do Squads |
| `credito_autoridade` | PDA `["autoridade_credito"]` do `ecol_credito` |
| prazo de entrega | 7 dias |
| tolerância de peso | 200 bps (2%) |
| percentuais | coletor 500, cooperativa 1.000, transportador 500, indústria 6.500 a 7.900 |

### Como rodar

```bash
# 1. saldo da carteira que paga o deploy (padrão: ~/.config/solana/id.json)
solana balance -u devnet                   # precisa de ≥ 11 SOL
# faucet: https://faucet.solana.com  (ou: solana airdrop 2 -u devnet, que tem limite de pedidos)

# 2. backup das keypairs dos programas (definem os endereços; fora do git)
mkdir -p ~/backup-ecolchain && cp onchain/target/deploy/*-keypair.json ~/backup-ecolchain/

# 3. deploy (num terminal interativo, respondendo "s")
cd onchain
scripts/deploy-devnet.sh

# ou sem a pergunta (ex.: dentro do Claude Code com "!", que não tem teclado)
CONFIRMAR=1 scripts/deploy-devnet.sh
```

| Variável | Padrão | Uso |
|---|---|---|
| `RPC_DEVNET` | `https://api.devnet.solana.com` | RPC da devnet. Um RPC dedicado (Helius, Triton...) evita limites de taxa. |
| `CARTEIRA` | `~/.config/solana/id.json` | pagador e upgrade authority inicial |
| `CONFIRMAR` | — | `1` pula a pergunta de confirmação |

A primeira execução baixa a imagem Docker (alguns GB) e leva de 5 a 15 minutos.

---

## 12. Implantação atual na devnet

Deploy inicial em **2026-09-28**; upgrades pelo Squads no mesmo dia (materiais, guardião, entregas, nomes de participante e balança, a ADR 0010 nas propostas #15 a #17 e a ADR 0011 na #18, em 2026-10-07), todos com build verificável e hashes conferidos.

### Programas

| Programa | Program ID | Hash do executável | Tamanho |
|---|---|---|---|
| `ecol_lote` | [`AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD`](https://explorer.solana.com/address/AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD?cluster=devnet) | `9d99a79e947be4a66f728b5771a05a06d31a79a2aec29112fbfc582b032efc01` | 846.296 bytes |
| `ecol_credito` | [`FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ`](https://explorer.solana.com/address/FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ?cluster=devnet) | `4b73d878ce5ce3bf261637a486ba18a0556b94c33bd93230b5ba1cc7cee67332` | 484.456 bytes |
| `ecol_reparticao` | [`HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K`](https://explorer.solana.com/address/HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K?cluster=devnet) | `0fe26d24fef78369defdecb436548be095ceb0ac61be291754fc79a05ae5d62f` | 390.496 bytes |

### Configs

| Config | Endereço |
|---|---|
| `GlobalConfig` (`ecol_lote`) | `4LWZsMqmTujQiJeFcCczwfbm1WTfPi4pVXnmRUCfvpzm` |
| `CreditoConfig` (`ecol_credito`) | `5XK3ybpnPy6dpWZoSdN6Ryv9XmvSoi15v67btdCiCoKu` |
| `ReparticaoConfig` (`ecol_reparticao`) | `Da2rhTSvZvX94pGFswBApMcqqeUnXzKUJQyxfVLwQcif` |

### Governança (Squads v4)

| Item | Endereço |
|---|---|
| Multisig (3 de 5, chaves da EcolChain, timelock 0) | `9W2HFHrGQ4sTtCaRCSeZ2ydiyuGAzg1Exb5jYFw3ZFjd` |
| **Vault 0**: upgrade authority dos 3 programas + `admin` das 3 configs | `5QLoaHGatRQnhiJxFBXhvkJ9GdDbzhCvySXwNCKiwXHj` |
| **Vault 1**: `arbitro` | `BKgC9T4fCyMA1AMe3fNJTquQjvufgjQcdCkXjjV1yh7G` |

Membros (todos da EcolChain; na devnet são chaves **de teste**, em `onchain/.devnet/`):

| Membro | Endereço |
|---|---|
| ecolchain-diretoria | `HU6fB42zKw1aooH7AG6g7d6RhheEwmJaPP7GeBF7gjuX` |
| ecolchain-tecnologia | `3gTZMLVADFd4zmPd1KxoUYtD7vRCYgWJ7ZaUjP73zkQU` |
| ecolchain-operacoes | `4x9mnBjUvCZUomBeBEHBnXi64DtjSwBZFVgaCmZtbnQ3` |
| ecolchain-financeiro | `6fHCPsYdKQFevbTcmW4ct8SAGFdHE7wAVDxF3yyuCi9W` |
| ecolchain-reserva | `7bePYaoQiyqmjqD7Y5gyCCU8zbFkcCt963dJcbmn2UPL` |

### Chaves de papel (teste)

| Papel | Endereço |
|---|---|
| operador (administração) | `EhY5n8NeRo8j3F8RsV2oZeC9HyzdMib9SXS475P6ec8X` (Solflare do responsável; até 2026-09-28 era a chave de teste `AURz…5ZGa`) |
| intermediador | `39J3HAavjhvt2vFLnTGNjh6L6F5PtD3UupKuveRq82Ez` |
| registrador | `p2e29Efu4TcPGqjeA9gzhW6mTqCtF8MzmZQjeiBYmYz` |
| zupy | `GTKWcFhronTQFAyE6sBwgtdFd12VJX4gjBAYE6RCH9cb` |

### Conferir o estado on-chain

```bash
solana program show AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD -u devnet   # Authority deve ser o vault 0
solana-verify get-program-hash -u devnet AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD
cat onchain/.devnet/squads.json
```

---

## 13. Governança e operação com o Squads

Depois do deploy, **nenhuma chave individual** altera os programas ou as configs. Toda mudança de governança é uma proposta no Squads, aprovada por 3 dos 5 membros e executada depois do timelock (0 na devnet, 24 h na mainnet). **Os 5 membros são chaves da própria EcolChain** (ADR 0009): cooperativas e indústrias são clientes e não votam; a confiança delas vem da transparência on-chain e do build verificável. A operação do dia a dia não passa por aí: fica com a carteira de operador.

Interface web: app do Squads v4 (https://app.squads.so) com a carteira de um membro, apontando para a devnet. Também dá para usar o SDK `@sqds/multisig` em `onchain/ops/`.

### O que passa pelo Squads e o que não passa

| Tipo de ação | Quem assina | Espera |
|---|---|---|
| **Dia a dia**: cadastrar participantes, balanças e materiais, encerrar leilões, montar créditos, publicar a repartição | carteira de **operador** (menu Administração da interface), uma assinatura | nenhuma: vale na hora |
| **Governança**: upgrade dos programas, percentuais da repartição, prazos e tolerâncias, troca de operador, intermediador, registrador ou árbitro, pausa | **Squads**: 3 das 5 chaves da EcolChain aprovam | **timelock** (0 na devnet, 24 h na mainnet) |
| **Disputas de lote**: `arbitro_resolve_disputa` | vault 1 do Squads (3 de 5) | timelock |

### Timelock: o que é e por que existe

O timelock é um **prazo de espera obrigatório entre a aprovação e a execução** de uma proposta. Mesmo com os 3 votos, a transação só pode ser executada depois que esse prazo passar, contado a partir da última aprovação.

**Ciclo de uma proposta:**

```
criada ──► ativa ──► aprovada (3 de 5) ──[ timelock ]──► executável ──► executada
                 │                           │
                 └──► rejeitada              └──► cancelada (os membros desistem antes de executar)
```

**Por que existe.** A aprovação 3 de 5 impede que **uma** pessoa da EcolChain aja sozinha, mas não protege se **três chaves forem comprometidas** (roubo, vazamento) ou se três pessoas combinarem algo indevido. O timelock cobre esse caso: toda mudança aprovada fica **pública on-chain** antes de valer, e nesse intervalo:

- as outras pessoas da EcolChain, as cooperativas, as indústrias e os auditores podem ver exatamente o que vai mudar;
- se for indevida, os membros **cancelam** a proposta antes da execução;
- quem discordar de uma regra nova (ex.: percentuais da repartição) tem tempo de se retirar antes de ela valer.

Exemplo: alguém rouba 3 chaves e aprova um upgrade que desvia a repartição. Sem timelock, o upgrade vale no mesmo segundo. Com timelock, há uma janela para detectar e cancelar.

**O custo.** Toda mudança de governança leva no mínimo o tempo do timelock. Isso é intencional e afeta só as ações raras e sensíveis; o dia a dia (tabela acima) não espera.

**Valores:**

| Rede | Timelock | Motivo |
|---|---|---|
| devnet | **0** (era 1 h até 2026-09-28) | os programas ainda mudam com frequência; a proteção fica na aprovação 3 de 5 |
| mainnet | **24 h** | toda mudança aprovada fica pública um dia antes de valer |

**Como acompanhar.** `npx tsx proposta.ts status` (em `onchain/ops/`) lista cada proposta, a situação e **a partir de quando pode ser executada**. Uma proposta aprovada **não executa sozinha**: depois do prazo, alguém precisa rodar `npx tsx proposta.ts executar <índice>` (ou executar pelo app do Squads).

**Como cancelar.** Enquanto não for executada, uma proposta aprovada pode ser cancelada pelos membros (3 de 5) no app do Squads (`proposalCancel` no SDK).

**Como mudar o timelock.** O multisig foi criado **sem config authority**, então nem o timelock muda sem votação: é uma *config transaction* do Squads (`npx tsx proposta.ts timelock <segundos>`), aprovada por 3 de 5 e sujeita ao timelock **atual** (a mudança só vale depois de esperar o prazo vigente uma última vez). Foi assim que a devnet passou de 1 h para 0 em 2026-09-28. Na devnet, sem timelock, não se testa o "esperar e cancelar"; isso fica para a mainnet.

### Guardião: pausa de emergência por qualquer membro

Com timelock na mainnet (24 h), até uma pausa esperaria um dia. Por isso existe um segundo caminho, só para **pausar**:

| | Squads principal (admin) | Squads do guardião |
|---|---|---|
| Membros | as 5 chaves da EcolChain | **as mesmas 5** |
| Quórum | 3 de 5 | **1 de 5** |
| Timelock | 0 (devnet) / 24 h (mainnet) | **0** |
| Pode | tudo (upgrade, configs, papéis, pausar e **despausar**) | **só pausar** (`guardiao_pausar`) |

- Os programas guardam o vault do Squads do guardião no campo `guardiao` das configs. `guardiao_pausar` só aceita deixar o modo **mais restritivo**; qualquer outra coisa é recusada (`GuardiaoSoRestringe`, `NaoEhGuardiao`).
- Um guardião comprometido, no pior caso, pausa a plataforma: incômodo, mas sem perda de fundos nem de dados, e o admin (3 de 5) desfaz.
- A lista de membros do Squads do guardião deve acompanhar a do principal. Ele também não tem config authority: com quórum 1, qualquer membro altera os membros, então essa troca deve ser combinada internamente.

```bash
cd onchain/ops
npx tsx pausar.ts ecolchain-tecnologia            # qualquer membro: pausa os 3 programas na hora
npx tsx pausar.ts ecolchain-tecnologia --somente-disputas   # ecol_lote em SomenteDisputas (disputas e escrow seguem)
npx tsx retomar.ts                                # proposta do admin (3 de 5) para tirar a pausa
npx tsx proposta.ts executar <índice>
```

Na devnet: Squads do guardião `6JpBi3MouEaouoUKH4HwBttREpWMvCNf2srDBUkTYYsq`, vault (guardião) `7tyuT6DDGUrTARsPzjgfPUEdEkV6SQVkwCRNs52B9GWz`. Criado por `ops/guardiao-devnet.ts` e definido nos programas por `ops/definir-guardiao.ts`.

### Scripts de proposta (devnet)

Na devnet, as 5 chaves de membro estão nesta máquina, então os scripts em `onchain/ops/` criam **e aprovam** (3 de 5) as propostas sozinhos; só falta esperar o timelock e executar:

```bash
cd onchain/ops
npx tsx proposta.ts status                  # propostas, situação e quando podem ser executadas
npx tsx trocar-operador.ts <carteira>       # troca o operador (administração) nos 3 programas
npx tsx propor-upgrade.ts <programa>=<buffer> [...]   # upgrade (amplia a conta do programa se preciso)
npx tsx proposta.ts executar <índice>       # depois do timelock
```

O membro que cria a proposta paga o rent dela (~0,0035 SOL), então precisa de um pouco de saldo.

**Programa maior que o espaço reservado.** Se o binário novo for maior, a conta do programa precisa ser ampliada antes do upgrade. Essa ampliação **não pode ir dentro da proposta**: o runtime recusa `ExtendProgram`/`ExtendProgramChecked` chamados pelo Squads (erro `not supported by inner instructions`). Como o `ExtendProgram` simples é permissionless (mínimo de 10.240 bytes), o `propor-upgrade.ts` amplia a conta **antes**, pagando com a carteira local, e a proposta leva só o `Upgrade`, que o Squads pode executar. Buffers que sobrarem de uma proposta que não será executada são devolvidos com `npx tsx fechar-buffers.ts <buffer> [...]`.

### Upgrade de um programa

```bash
cd onchain
NO_DNA=1 anchor build --verifiable

# 1. envia o novo binário para um buffer (pago pela carteira local)
solana program write-buffer target/verifiable/ecol_lote.so -u devnet
#    → anota o endereço do BUFFER

# 2. entrega o buffer ao vault 0 (é ele quem assina o upgrade)
solana program set-buffer-authority <BUFFER> \
  --new-buffer-authority 5QLoaHGatRQnhiJxFBXhvkJ9GdDbzhCvySXwNCKiwXHj -u devnet

# 3. no Squads: nova proposta de upgrade do programa usando o <BUFFER>;
#    3 membros aprovam; depois do timelock, qualquer membro executa

# 4. conferir
solana-verify get-executable-hash target/verifiable/ecol_lote.so
solana-verify get-program-hash -u devnet AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD
```

### Mudar uma config, pausar ou trocar papéis

1. Monte a instrução com o cliente gerado (ex.: `getAdminSetModoInstruction` do `ecol_lote`), usando o **vault 0** como `admin`.
2. Crie uma proposta de transação no Squads (vault 0) com essa instrução.
3. Três membros aprovam e, depois do timelock, a proposta é executada.

### Emergência

- **Qualquer membro da EcolChain**: `npx tsx pausar.ts <membro>` pausa os 3 programas na hora, pelo Squads do guardião (1 de 5, sem timelock).
- Pelo admin (3 de 5, com timelock): `admin_set_modo(SomenteDisputas | Pausado)` no `ecol_lote` e `admin_set_pausado(true)` no `ecol_credito` e no `ecol_reparticao`.
- Tirar a pausa é **sempre** do admin: `npx tsx retomar.ts`.
- Mesmo em `SomenteDisputas`, disputas, liberação do escrow e desagregação continuam funcionando, para não travar dinheiro retido.
- Ao voltar para `Normal`, o crank de prazo expirado espera **48 h** antes de derrubar lotes.

### Resolver uma disputa (árbitro = vault 1)

1. Monte `arbitro_resolve_disputa` com o **vault 1** como `arbitro`.
2. Se a decisão for `Liberar` ou `Reembolsar`, o intermediador também assina, então a proposta precisa incluir a assinatura dele.
3. Aprovação 3 de 5 e execução.

---

## 14. Verificação independente (auditores)

- **Código = binário:** qualquer pessoa pode recompilar com `anchor build --verifiable` (mesma imagem Docker) e comparar `solana-verify get-executable-hash` com `solana-verify get-program-hash`. Com o repositório no GitHub, `solana-verify verify-from-repo` pode automatizar isso (exige acesso público ao repositório).
- **Trilha:** todo evento carrega `seq`, e uma lacuna indica evento perdido.
- **Crédito × lotes:** reconstrua `lotes_hash` a partir dos eventos `LoteAgregado` e compare com a conta `Credito`.
- **Repartição:** reconstrua a raiz Merkle com `montarDireitos` a partir dos pesos dos lotes e do mapeamento `origem_ref → carteira` dos coletores. Os tetos por papel estão na conta `Distribuicao` e a soma fecha com a receita.
- **Pesagens:** cada lote de origem e cada lote de venda guardam a balança e o `ts_pesagem` assinados por ela.

---

## 15. Custos

| Item | Valor |
|---|---|
| Deploy dos 3 programas + Squads + configs (devnet) | ≈ 7,5 SOL, quase todo em rent dos programas |
| Por lote (PDA `Lote` + Core asset + transações) | ≈ 0,0084 SOL (medido; ver ADR 0003) |
| Taxa por assinatura | 5.000 lamports (transações multi-assinatura custam mais) |

O rent é um depósito recuperável, não uma despesa, se as contas forem fechadas (ver achado 13 na seção 18). A taxa do Metaplex Core (~0,0015 SOL por asset) ainda precisa ser confirmada.

---

## 16. Segurança

- **Nunca versionar:** `onchain/.devnet/` (chaves de teste e `squads.json`), `onchain/target/` (inclui as keypairs dos programas) e `~/.config/solana/id.json`. Os dois primeiros estão no `.gitignore`.
- **Backup das keypairs dos programas:** `onchain/target/deploy/*-keypair.json` definem os endereços dos programas. Hoje há uma cópia em `~/backup-ecolchain/`; guarde outra fora da máquina.
- **As chaves do Squads na devnet são de teste**, todas na mesma máquina. Na mainnet, as 5 chaves continuam sendo da EcolChain, mas com **pessoas e locais de guarda diferentes** (de preferência hardware wallets, uma delas offline), e timelock de 24 h.
- **SDK do Squads isolado:** `@sqds/multisig` depende de `@solana/web3.js` v1, que tem 4 vulnerabilidades altas transitivas no `npm audit`. Por isso fica só em `onchain/ops/` (uso operacional) e nunca entra nos clientes nem no backend.
- **Práticas seguidas** ([solana-ai-kit](https://github.com/solanabr/solana-ai-kit)): aritmética checada, sem `unwrap` em produção, sem `init_if_needed`, validação de owner, discriminator e seeds, e auditoria antes de fechar cada fase. Achados e correções em [`docs/security-audit-2026-09-28.md`](docs/security-audit-2026-09-28.md).

---

## 17. Solução de problemas

| Sintoma | Causa / solução |
|---|---|
| `Docker indisponível no WSL` | Docker Desktop → Settings → Resources → WSL integration → ative a distro e reinicie o terminal |
| `instale o solana-verify` | `cargo install solana-verify` |
| `saldo X SOL < 11 SOL` | complete em https://faucet.solana.com |
| Script sai logo depois de listar os programas | a pergunta `[s/N]` não recebeu resposta (ex.: `!` no Claude Code). Use `CONFIRMAR=1`. |
| Build verificável demora muito na primeira vez | download da imagem `quay.io/ottersec/anchor:v1.1.2`; as próximas execuções usam o cache |
| `hash on-chain ≠ build` | o `.so` publicado não é o de `target/verifiable/`. Refaça o build verificável e o upgrade. |
| Testes LiteSVM falham ao carregar o programa | falta `anchor build` (os testes leem `target/deploy/*.so`) |
| Cliente TS com tipos desatualizados | `anchor build && cd clients && npm run gerar` |
| `ProgramaPausado` | o modo não é `Normal` (ou a config está pausada); veja a config on-chain |
| `EmCarencia` no crank | o programa voltou ao modo `Normal` há menos de 48 h |
| Erro de pesagem no `register_entrega` ou no `fechar_lote` | a instrução Ed25519 não está imediatamente antes, a balança está inativa ou é de outro dono, ou o timestamp está fora da janela de 15 min |
| `PesoConsolidadoForaDaFaixa` | o peso do lote de venda está fora de soma − 10% a soma + 2% dos lotes de origem; confira a pesagem ou desfaça a união |
| `airdrop` falha | limite do faucet público; use o site ou outro RPC |
| Deploy de novo depois do primeiro falha | a upgrade authority agora é o vault 0; faça o upgrade pelo Squads (seção 13) |

---

## 18. Pendências antes da mainnet

| # | Pendência |
|---|---|
| Achado 13 | `close` das contas em estado terminal (recuperar o rent) |
| R1 | **reduzido:** os coletores são divididos pelo peso dos lotes de origem registrados on-chain; a árvore continua sendo publicada pelo operador e verificada off-chain (reproduzível por qualquer auditor). |
| R2 | o percentual da indústria (`bps_industria`) é informado só pelo operador |
| R3 | direitos não resgatados não expiram |
| Custos | confirmar a taxa do Metaplex Core (~0,0015 SOL) |
| CI | repositório no GitHub desde 2026-09-28; falta a CI com build, testes, clippy e `solana-verify verify-from-repo` |
| Governança | definir quais pessoas da EcolChain guardam as 5 chaves da mainnet (quórum 3 de 5, timelock de 24 h e guardião 1 de 5 já decididos, ADR 0009) |
| Regulatório | enquadramento de cupons, créditos de carbono e LGPD |
| Diagrama | redesenhar a faixa blockchain do `Proposta_04.excalidraw` com a máquina de estados |
