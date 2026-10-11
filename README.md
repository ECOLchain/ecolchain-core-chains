# EcolChain

Traceable recycling on Solana, with the revenue from carbon credits split among everyone who took part in the chain.

- **Status:** 3 Anchor programs deployed on **devnet** with a verifiable build, governed by a 3-of-5 Squads multisig.
- **Repository:** https://github.com/ECOLchain/ecolchain-core-chains (Solana programs and TypeScript clients). The web interface lives in its own repository.
- **Context and history:** [`CONTINUATION.md`](CONTINUATION.md)
- **Architecture decisions:** [`docs/adr/`](docs/adr/README.md)
- **Audit:** [`docs/security-audit-2026-09-28.md`](docs/security-audit-2026-09-28.md)

---

## Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Programs](#3-programs)
   - [ecol_lote](#31-ecol_lote)
   - [ecol_credito](#32-ecol_credito)
   - [ecol_reparticao](#33-ecol_reparticao)
4. [Roles and keys](#4-roles-and-keys)
5. [Complete flow, step by step](#5-complete-flow-step-by-step)
6. [Off-chain formats that must match on-chain](#6-off-chain-formats-that-must-match-on-chain)
7. [Repository structure](#7-repository-structure)
8. [Development environment](#8-development-environment)
9. [Build and tests](#9-build-and-tests)
10. [TypeScript clients](#10-typescript-clients)
11. [Devnet deploy](#11-devnet-deploy)
12. [Current devnet deployment](#12-current-devnet-deployment)
13. [Governance and operation with Squads](#13-governance-and-operation-with-squads)
14. [Independent verification (auditors)](#14-independent-verification-auditors)
15. [Costs](#15-costs)
16. [Security](#16-security)
17. [Troubleshooting](#17-troubleshooting)
18. [Open items before mainnet](#18-open-items-before-mainnet)

---

## 1. Overview

**Physical flow:** material origin (collector, the cooperative's own sorting, donation, purchase or independent waste picker) → Cooperative (weighs each **source lot**, combines several into a **sale lot** and weighs it again) → Carrier → Industry.

**Lot marketplace:** the cooperative lists the lot, industries place bids, and the payment is held in escrow until delivery.

**Carbon credit:** several recycled lots are aggregated into a credit. The credit is validated and issued in the official registry (Verra or Gold Standard) and then sold. The sale revenue is split among the participants.

| Participant | Share of the credit revenue | Form of payment |
|---|---|---|
| Collector | 5% (fixed) | Zupy coupons; also receives R$ via Pix on delivery to the cooperative |
| Cooperative | 10% (fixed) | Zupy coupons |
| Carrier | 5% (fixed) | Zupy coupons |
| Industry | 65% to 79% (negotiated per credit) | Zupy coupons |
| EcolChain | the remainder: 1% to 15%, plus the rounding residue | R$ / stablecoin |

Example from the diagram: a credit sold for US$ 50.00 (5,000 cents) with the industry at 65% yields **250 / 500 / 250 / 3,250** in coupons and **750** for EcolChain. With the industry at 79%, the industry receives 3,950 and EcolChain 50.

### Design principles (summary of the ADRs)

| Decision | What it means in practice |
|---|---|
| Off-chain R$ escrow (ADR 0001) | The money is held by a **payment intermediary** (Pix / escrow account). Only the intermediary's attestations go on-chain (hashes of the deposit, release and refund references). |
| No native token (ADR 0002) | EcolChain's share is paid in R$ / stablecoin. There is no mint, which avoids CVM (Brazilian securities regulator) issues. |
| Core asset + state PDA (ADR 0003) | Each lot has a `Lote` PDA (the state machine, source of truth) and a frozen **Metaplex Core asset** that serves as a receipt visible in wallets. |
| Embedded wallet + fee payer (ADR 0004) | Each participant signs with their own wallet (embedded/MPC). EcolChain pays the fees (`payer` separate from whoever signs the action). |
| Credit as a record only (ADR 0005) | The credit is **not tokenized**. What goes on-chain is the serial number, the lots that originated it, the sale and the proof of retirement. |
| Collector receives R$ on delivery (ADR 0006) | Immediate payment via Pix, off the blockchain, plus the 5% in coupons later. |
| Entitlement on-chain, Zupy redeems (ADR 0007) | Each participant's entitlements are published as a **Merkle tree**. Zupy only records the redemptions, and each redemption is on-chain. |
| EcolChain governance (ADR 0009, supersedes 0008) | Upgrades, configs and the arbiter sit in a **3-of-5 Squads multisig**, with all 5 keys held by EcolChain itself. Timelock 0 on devnet and 24 h on mainnet. |
| Source lot and sale lot (ADR 0010) | Each weighing at the cooperative is a **source lot**, with or without a collector. Several source lots of the same material form a **sale lot**, which is weighed again and only then listed. The combination can be undone before the sale. The collectors' cap in the revenue split applies only to the weight that came from collectors. |
| Importer, Clean Tech, wallets and variations (ADR 0011) | A **wine Importer** records bottles placed on the market (`Distribuicao`) and collections (`Coleta`) that it delivers to a cooperative or **Clean Tech** (a role that operates like the cooperative); the collection becomes an `Importador` source lot and, once the lot is recycled, receives the invoice (NF) hash. Each participant can have **several named wallets** (`Carteira`, PDA `["carteira", endereço]`) that sign on their behalf; the identity remains the holder wallet. Materials gain **variations** (`MaterialVariacao`: Transparente, Verde, Marrom for glass) and **approximate bottle counts**; a sale lot does not mix colors. |
| Direct sale and lot sold elsewhere (ADR 0012) | The cooperative puts the lot **up for sale at a fixed price** (`cooperativa_anunciar_venda`, `Anunciado` with no deadline) or marks it as **sold outside the platform** (`VendidoFora`). The industry buys on its own at the price or higher (`industria_comprar_lote`), choosing **pickup by a third-party carrier or its own pickup** (the industry signs the pickup). After receipt, the cooperative confirms the payment with the hash of the payment proof (`cooperativa_confirmar_pagamento`) and the lot moves to `Reciclado`. The three-signature auction remains valid. |
| On-chain bids in the auction (ADR 0013) | The industry bids in the auction from its own wallet (`industria_dar_lance`, then `industria_aumentar_lance`): one `Lance` account per industry and lot, amount ≥ minimum and above the highest bid, until the deadline. The lot stores the leader and the highest bid. With on-chain bids, the three-signature sale can go only to the leader, at the leader's amount, after the deadline; without them, the off-chain tally applies. The `public_close_lance` crank returns the rent at the end of the round. |
| MTR at pickup (ADR 0014) | Whoever picks up (the carrier, or the industry on its own pickup) provides the MTR number (1 to 999,999) in a code that the cooperative reads before signing; `transportador_pickup_lote(mtr)` requires the number and writes it to `Lote.mtr` (the last 4 reserved bytes). |

**LGPD:** no personal data goes on-chain. The origin of each lot appears as `origem_ref` (hash of the collector's wallet, or of the text of the invoice, receipt or CNPJ), the registration as `kyc_hash`, and the photos, tickets and invoices of the sale lot as `evidencias_hash`, with the files kept off-chain. The **name** of the participant and of the scale is public (up to 32 bytes): for collectors, use a nickname or code.

---

## 2. Architecture

### Layers

```
 ┌──────────────────────────────┐   ┌──────────────────────────────┐   ┌─────────────────────────────┐
 │ Web interface (own repo)     │   │ EcolChain backend / scripts  │   │ Scales (Ed25519 keypair)    │
 │ React 19, participant        │   │ operator, entitlements tree, │   │ sign each weighing          │
 │ wallet, public trail         │   │ Squads ops (onchain/ops)     │   │                             │
 └──────────────┬───────────────┘   └──────────────┬───────────────┘   └──────────────┬──────────────┘
                └───────────────┬──────────────────┴───────────────────────────────────┘
                                ▼
               ┌────────────────────────────────────┐
               │ onchain/clients (TypeScript)       │  Codama clients (@solana/kit 8), PDAs,
               │                                    │  Ed25519 weighing, references, Merkle tree
               └────────────────┬───────────────────┘
                                ▼  Solana RPC
 ┌──────────────────────────────────────────────────────────────────────────────────────────────┐
 │                                 Anchor programs (this repo)                                  │
 └──────────────────────────────────────────────────────────────────────────────────────────────┘
```

The web interface imports the clients from this repository (`onchain/clients/src`, alias `@clientes`). On the same machine, the two folders sit side by side.

### Programs

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                 Squads multisig (3 of 5)                │
                    │   vault 0 = upgrade authority + admin    vault 1 = arbiter│
                    └──────────────┬───────────────────────────────┬──────────┘
                                   │ admin                          │ arbiter
       ┌───────────────────────────▼──────┐                         │
       │            ecol_lote             │◄────────────────────────┘
       │  GlobalConfig, Participante,     │
       │  Balanca, Material,              │
       │  Entrega (source lot),           │
       │  Lote (sale lot, + asset)        │
       └───────────────▲──────────────────┘
                       │ CPI credito_mark_agregado / _desagregado
                       │ (signed by the PDA ["autoridade_credito"])
       ┌───────────────┴──────────────────┐
       │           ecol_credito           │
       │  CreditoConfig, Credito,         │
       │  SerialRegistro                  │
       └───────────────▲──────────────────┘
                       │ reads the sold Credito (amount and weights)
       ┌───────────────┴──────────────────┐
       │          ecol_reparticao         │
       │  ReparticaoConfig, Distribuicao  │──► reads ecol_lote's Participante on redemption
       └──────────────────────────────────┘

  Off-chain: EcolChain backend (operator), payment intermediary, registrar
  (Verra / GS), Zupy, scales with their own keypair, evidence storage.
```

### From material to credit

```
 Source lot (Entrega)              Sale lot (Lote)                                   Credit
 ┌───────────────────────┐
 │ Collector | Sorting   │ combine ┌──────────────────────────┐  auction, sale,  ┌───────────────┐
 │ Donation | Purchase | ├────────►│ EmMontagem ─► weighing of│  pickup,         │ Credito       │
 │ Independent; weighed  │ (same   │ consolidated ─► Criado   ├─────────────────►│ (several lots │
 │ by cooperative scale  │  coop., │ (Core asset born here)   │  receipt,        │  of the same  │
 └───────────────────────┘ same    └──────────────────────────┘  recycled        │  material)    │
                           material)        │ undo before the sale                └───────────────┘
                                            ▼
                                  EmDesmontagem ─► Desfeito (sources become available again)
```

The trail can be navigated in both directions: from the credit to the sale lots (`lotes_hash`), from them to the source lots (`Entrega.lote`, `entregas_hash`) and to the origin (`origem_ref`); and from a source lot up to the credit (`Lote.credito`).

- **Canonical state in PDAs.** The Core asset is derived from the lot (`["asset", lote]`) and carries no state.
- **Every event has a sequence number** (`seq`) per lot, credit or distribution. A gap in the sequence is detectable by auditors.
- **Each config key has a single role.** The programs reject configs in which the same key holds two roles or is null, and a role key cannot be registered as a participant.
- **Compatible layout.** New fields occupy the accounts' reserved space (zero means the old behavior) and enum variants are only appended at the end, so that existing accounts remain readable after an upgrade.

---

## 3. Programs

| Program | Program ID | Instructions |
|---|---|---|
| `ecol_lote` | `AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD` | 33 |
| `ecol_credito` | `FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ` | 15 |
| `ecol_reparticao` | `HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K` | 10 |

**Naming convention:** `<papel>_<ação>` (`<role>_<action>`). The prefix indicates who signs: `admin_`, `operador_`, `cooperativa_`, `industria_`, `transportador_`, `intermediador_`, `arbitro_`, `registrador_`, `zupy_`, `parte_` (any party to the lot), `credito_` (only via CPI from `ecol_credito`) and `public_` (cranks anyone can call).

**Protected initialization:** `admin_init_*` requires the signature of the program's **upgrade authority**, so nobody can front-run the deploy and create the config.

**Guardian (in all 3 programs):** `admin_set_guardiao(guardiao)` sets who can pause without a timelock; `guardiao_pausar` only **restricts** (in `ecol_lote`: `Normal` → `SomenteDisputas` → `Pausado`; in the others: paused = true). Returning to normal is always up to the admin. Details in section 13.

**Two-step admin change:** `admin_propose_admin(novo)` followed by `admin_accept_admin` signed by the new admin. Proposing `Pubkey::default()` cancels a pending proposal.

### 3.1 `ecol_lote`

Records the material from its origin: the source lots weighed at the cooperative, their combination into sale lots, and every stage of the sale, logistics and consumption, along with who signed each one.

#### Accounts (PDAs)

| Account | Seeds | Contents |
|---|---|---|
| `GlobalConfig` | `["global"]` | admin, arbiter, operator, payment intermediary, guardian, `credito_autoridade`, delivery deadline, weight tolerance, assembly weight range (`perda_montagem_bps`, `excesso_montagem_bps`; zero = default −10% / +2%), mode, pending admin |
| `Participante` | `["part", carteira]` | role (Collector, Cooperative, Carrier, Industry), active, `kyc_hash`, name (up to 32 bytes UTF-8, public). **A wallet has a single role, which never changes.** |
| `Balanca` | `["balanca", dispositivo]` | owner (cooperative or industry), the device's Ed25519 key, active, name (up to 32 bytes) |
| `Material` | `["material", codigo (u16 LE)]` | code, name (up to 32 bytes), active. **Registered by the operator**; lots and credits store only the code |
| `Entrega` | `["entrega", cooperativa, entrega_id]` | **source lot**: `origem` (`Coletor`, `TriagemPropria`, `Doacao`, `Compra`, `Avulso`; old accounts = `Coletor`), `origem_ref` (hash, LGPD), material, weight signed by the scale, scale, timestamp, and the sale lot it went into (empty while available) |
| `Lote` | `["lote", cooperativa, lote_id]` | **sale lot**: material, consolidated weight, linked source lots (count, sum of weights, `peso_sem_coletor_g`, chained hash), evidence (invoice or receipt), parties, amount, escrow hashes, deadline, state, `asset_queimavel`, `seq`... |
| Core asset | `["asset", lote]` | receipt for the sale lot (Metaplex Core), created at closing |
| Asset authority | `["autoridade_asset"]` | data-less PDA; it is the update authority and owner of the asset's permanent plugins |

**Materials:** on-chain registry maintained by the operator (`operador_create_material`), with no fixed list in the code. An inactive material cannot be used in new lots or credits; existing lots are not affected.

#### Lot state machine

```
EmMontagem ──► Criado ──► Anunciado ──► Vendido ──► EmTransporte ──► Recebido ──► Reciclado ──► Agregado
    │            │          │   ▲          │             │               │                        │
    │            │          ▼   │          └─────────────┴───────┬───────┘                        │
    │            │        SemLance                               ▼                                ▼
    │            │          │                               EmDisputa ──► (prior stage)        Reciclado
    ▼            ▼          ▼                                    │   ──► Reciclado (only if prior stage = Recebido)
    └──────► EmDesmontagem ─► Desfeito (terminal)                └─────► Reembolsado (terminal)
```

| Transition | Instruction | Who signs | Conditions |
|---|---|---|---|
| (source lot) | `cooperativa_register_entrega(dados)` | cooperative + payer | `dados`: `entrega_id`, `origem`, `origem_ref` (non-null), active material, `peso_g`, `ts_pesagem`. The **cooperative's own active scale** signs `(domain, Entrega, entrega, weight, ts)` in the preceding Ed25519 instruction. |
| → `EmMontagem` | `cooperativa_create_lote(lote_id, material, evidencias_hash)` | cooperative + payer | material registered and **active**; no weighing and no asset yet |
| (assembly, in `EmMontagem`) | `cooperativa_add_entregas` | cooperative | up to 10 source lots per transaction in `remaining_accounts` (writable): from the same cooperative, of the same material and not yet linked. Updates the count, the sum of weights, `peso_sem_coletor_g` and `entregas_hash = sha256(h ‖ entrega ‖ peso_g)`. |
| `EmMontagem` → `Criado` | `cooperativa_fechar_lote(peso_g, ts_pesagem, uri)` | cooperative + payer | at least one source lot; Ed25519 weighing of the **consolidated** lot by the cooperative's active scale (timestamp at most 15 min old); weight between **sum − 10%** and **sum + 2%** (`PesoConsolidadoForaDaFaixa`; limits in `GlobalConfig`). Creates the frozen, burnable Core asset in the cooperative's name. |
| `EmMontagem`/`Criado`/`SemLance` → `EmDesmontagem` → `Desfeito` | `cooperativa_desmontar_lote` | cooperative | up to 10 source lots per transaction in `remaining_accounts`, which become available again. With the last one, the lot goes to `Desfeito` and the Core asset is burned. Never after the sale. |
| `Criado`/`SemLance` → `Anunciado` | `cooperativa_list_lote(prazo_leilao, preco_minimo_centavos)` | cooperative | price > 0; deadline in the future and at most 90 days out |
| `Anunciado` → `SemLance` | `cooperativa_cancel_anuncio` | cooperative | at any time (to reprice or withdraw) |
| `Anunciado` → `SemLance` | `operador_close_leilao_sem_lance` | operator | only after `prazo_leilao` |
| `Anunciado` → `Vendido` | `industria_accept_venda(valor_centavos, escrow_ref_hash, lances_root)` | **industry + operator + payment intermediary** | amount ≥ minimum price; the operator attests the winner and the Merkle root of the bids; the payment intermediary attests the escrow deposit. Locks in the delivery deadline and weight tolerance in effect. |
| `Vendido` → `EmTransporte` | `transportador_pickup_lote` | **carrier + cooperative** | custody handover; the Core asset goes to the carrier |
| `EmTransporte` → `Recebido` | `industria_confirm_recebimento(peso_recebido_g, ts_pesagem?)` | industry | with `ts_pesagem`, requires the Ed25519 weighing from one of the industry's scales. If the weight diverges beyond the tolerance, it goes **straight to `EmDisputa`** (`DivergenciaPeso`). The asset goes to the industry. |
| `Recebido` → `Reciclado` | `intermediador_confirm_liberacao(liberacao_ref_hash)` | payment intermediary | attests that it released the escrow to the cooperative (full consumption of the sale lot) |
| `Vendido`/`EmTransporte`/`Recebido` → `EmDisputa` | `parte_open_disputa(motivo)` | the lot's cooperative, industry or carrier | cannot reopen a dispute at a stage already ruled on with "proceed" |
| `Vendido`/`EmTransporte` → `EmDisputa` | `public_crank_prazo_expirado` | anyone | delivery deadline passed; only in `Normal` mode, and after a 48 h grace period if the program has come out of a pause |
| `EmDisputa` → prior stage / `Reciclado` / `Reembolsado` | `arbitro_resolve_disputa(decisao)` | arbiter (+ payment intermediary when money is involved) | see decisions below |
| `Reciclado` → `Agregado` | `credito_mark_agregado(credito)` | `ecol_credito`'s `["autoridade_credito"]` PDA (CPI) | — |
| `Agregado` → `Reciclado` | `credito_mark_desagregado(credito)` | same (CPI) | the lot belongs to that credit and the credit was rejected or abandoned |

**Arbiter decisions (`DecisaoDisputa`):**
- `Prosseguir { novo_peso_g }`: returns to the prior stage, optionally reclassifying the weight. Renews the deadline if the dispute was opened before delivery. That stage cannot be disputed again.
- `Liberar { valor_liberado_centavos, novo_peso_g, ref_hash }`: only for a dispute opened at receipt. Closes by releasing `valor_liberado_centavos` to the cooperative (full or partial) and refunding the remainder to the industry. The payment intermediary co-signs.
- `Reembolsar { ref_hash }`: closes in `Reembolsado`. The payment intermediary co-signs, attesting the refund.

**Dispute reasons:** `DivergenciaPeso`, `ClassificacaoContestada`, `FalhaEntrega`, `PrazoExpirado`, `Outro`.

#### Registration and administration

| Instruction | Who signs | Effect |
|---|---|---|
| `admin_init_global(admin, params)` | payer + upgrade authority | creates the `GlobalConfig` |
| `admin_update_global(params)` | admin | changes the arbiter, operator, payment intermediary, `credito_autoridade`, delivery deadline (≤ 180 days) and weight tolerance (≤ 20%) |
| `admin_set_modo(modo)` | admin | `Normal`, `SomenteDisputas` (only disputes, crank, arbiter, escrow release and disaggregation) or `Pausado` (no lot transitions) |
| `admin_set_tolerancia_montagem(perda_bps, excesso_bps)` | admin | range accepted when closing the sale lot (each limit > 0 and ≤ 30%) |
| `admin_set_guardiao(guardiao)` / `guardiao_pausar(modo)` | admin / guardian | emergency pause (section 13) |
| `admin_propose_admin` / `admin_accept_admin` | admin / new admin | two-step admin change |
| `operador_register_participante(carteira, papel, kyc_hash, nome)` | operator + payer | registers the wallet with a role and a name |
| `operador_set_participante_nome(nome)` | operator | fills in or corrects the name (registrations made before the field existed have no name) |
| `operador_set_participante_ativo(ativo)` | operator | an inactive participant cannot take on new commitments |
| `operador_register_balanca(dispositivo, nome)` | operator + payer | scale belonging to an active cooperative or industry |
| `operador_set_balanca_nome(nome)` | operator | fills in or corrects the scale's name |
| `operador_set_balanca_ativa(ativa)` | operator | deactivates replaced equipment, a compromised key or an expired calibration |
| `operador_create_material(codigo, nome)` | operator + payer | registers a material type; the code is unique and never changes |
| `operador_update_material(nome, ativo)` | operator | renames, activates or deactivates a material |

#### Core asset (receipt)

- Created at **sale lot closing** with `PermanentFreezeDelegate` (frozen), `PermanentTransferDelegate` and `PermanentBurnDelegate`, all with the `["autoridade_asset"]` PDA as authority. Source lots have no asset (only PDAs, which are cheaper).
- **The owner cannot transfer it.** Only the program moves the asset, following custody: cooperative → carrier (pickup) → industry (receipt).
- When the combination is undone, the program burns the asset (this works even while frozen). Lots closed before ADR 0010 do not have the burn plugin (`asset_queimavel = false`).
- No `Attributes` plugin: the state lives in the `Lote` PDA.

### 3.2 `ecol_credito`

Records the carbon credit and its link to the physical lots. **It does not tokenize.**

#### Accounts (PDAs)

| Account | Seeds | Contents |
|---|---|---|
| `CreditoConfig` | `["credito_config"]` | admin, operator, registrar, payment intermediary, paused |
| `Credito` | `["credito", credito_id (u64 LE)]` | standard (Verra, GoldStandard), material, vintage, methodology hash, lot count and weight, `peso_sem_coletor_g`, `lotes_hash`, tCO2e × 1000, buyer, amount, payment hash, formation deadline, retirement proof, serial, state, `seq` |
| `SerialRegistro` | `["serial", padrão, sha256(serial)]` | ensures that a registry serial is used by a single credit (**prevents double counting**) |
| Authority | `["autoridade_credito"]` | PDA that signs the CPIs into `ecol_lote`. Must equal the `credito_autoridade` in `GlobalConfig`. |

#### Credit state machine

```
EmFormacao ──► EmValidacao ──► Emitido ──► Vendido ──► Aposentado
    │                │
    ▼                ▼
Abandonado       Rejeitado          (in these two states the lots can be released)
```

| Transition | Instruction | Who signs | Conditions |
|---|---|---|---|
| → `EmFormacao` | `operador_create_credito(credito_id, padrao, material, vintage, metodologia_hash)` | operator + payer | material registered and active; vintage ≥ 2000; formation deadline = now + 180 days |
| (adds lots) | `operador_add_lotes` | operator | up to **10 lots per transaction** in `remaining_accounts` (writable), all `Reciclado` and **of the same material** as the credit. Each lot becomes `Agregado` via CPI. Updates `lotes_hash = sha256(h ‖ lote ‖ peso_g)` and adds each lot's collector-free weight, proportional to the consolidated weight: `floor(peso_g × peso_sem_coletor_g / peso_entregas_g)`. |
| `EmFormacao` → `EmValidacao` | `operador_submit_credito` | operator | at least 1 lot |
| `EmValidacao` → `Emitido` | `registrador_record_emissao(serial, serial_hash, tco2e_milli)` | registrar + payer | serial ≤ 64 bytes, `serial_hash = sha256(serial)`, tCO2e > 0; creates the `SerialRegistro` |
| `EmValidacao` → `Rejeitado` | `registrador_record_rejeicao(motivo_hash)` | registrar | — |
| `Emitido` → `Vendido` | `operador_record_venda(valor_centavos, pagamento_ref_hash)` | **operator + buyer + payment intermediary** | the payment intermediary attests receipt of the amount, which is the basis for the revenue split |
| `Vendido` → `Aposentado` | `registrador_record_aposentadoria(prova_hash)` | registrar | retired in the registry on behalf of the buyer |
| `EmFormacao` → `Abandonado` and release | `public_release_lotes` | anyone | if the credit has been `EmFormacao` for more than 180 days, it is declared `Abandonado`. In `Rejeitado`/`Abandonado`, returns the lots passed in `remaining_accounts` to `Reciclado` via CPI. This way EcolChain cannot hold lots indefinitely. |

**Administration:** `admin_init_config(admin, operador, registrador, intermediador)`, `admin_update_config(operador, registrador, intermediador)`, `admin_set_pausado`, `admin_propose_admin` / `admin_accept_admin`.

### 3.3 `ecol_reparticao`

For each credit sold, publishes **how much each participant is entitled to receive** and records each redemption in coupons.

#### Accounts (PDAs)

| Account | Seeds | Contents |
|---|---|---|
| `ReparticaoConfig` | `["rep_config"]` | admin, operator, zupy, payment intermediary, percentages (bps), paused |
| `Distribuicao` | `["dist", credito]` | revenue, copy of the percentages, industry %, **cap per role**, amount redeemed per role, EcolChain's share, Merkle root, number of leaves, state, payout hashes, redemption bitmap |

**Percentages (basis points, 10,000 = 100%):** collector 500, cooperative 1,000, carrier 500, industry from 6,500 to 7,900. The config is validated on-chain: the three fixed ones must be greater than zero, and `fixos + indústria_max + 1%` cannot exceed 100%, meaning EcolChain never gets less than 1%. Changing the percentages **does not affect distributions already created**, because each one keeps a copy.

#### Cap calculation

```
teto(papel)   = floor(receita × bps(papel) / 10.000)     for cooperative, carrier, industry
teto(coletor) = floor(receita × bps(coletor) × (peso_total − peso_sem_coletor) / (10.000 × peso_total))
EcolChain     = receita − sum of the 4 caps                (includes the rounding remainder)
```

The collectors' cap applies only to the weight that came from collectors; the share for material without a collector (sorting, donation, purchase, one-off) stays with EcolChain. Credits without weights (prior to ADR 0010) keep the full formula.

The sum always adds up exactly to the revenue. This is verified by property-based tests with 5,000 cases.

#### States and instructions

`Pendente → Financiada`

| Instruction | Who signs | Effect |
|---|---|---|
| `operador_create_distribuicao(bps_industria, merkle_root, num_folhas)` | operator + payer | requires a `Vendido` credit; the revenue comes from the `Credito` itself. Calculates the caps. Maximum of 4,096 leaves. |
| `intermediador_record_repasse(zupy_ref_hash, ecolchain_ref_hash)` | payment intermediary | attests that it transferred the coupon share to Zupy and EcolChain's share → `Financiada` |
| `zupy_record_resgate(indice, papel, valor_centavos, prova, recibo_hash)` | **Zupy + beneficiary** | only in `Financiada`. The leaf is proven against the root (up to 16 levels). The beneficiary must be registered in `ecol_lote` **with the leaf's role**. Each leaf is redeemed once (bitmap) and the role's total does not exceed the cap. |

**Administration:** `admin_init_config(admin, params)`, `admin_update_config(params)`, `admin_set_pausado`, `admin_propose_admin` / `admin_accept_admin`.

---

## 4. Roles and keys

| Role | Where it is configured | What it does |
|---|---|---|
| **admin** | the 3 configs | changes configs, pauses, swaps roles. On devnet it is Squads **vault 0**. |
| **arbiter** | `GlobalConfig` | rules on lot disputes. On devnet it is Squads **vault 1**. |
| **guardian** | the 3 configs | **only pauses**, immediately, with no timelock. It is the vault of a second **1-of-5** Squads with the same members: anyone at EcolChain can pause on their own. It cannot unpause or change anything. |
| **operator** | the 3 configs | **day-to-day administration** ("Administration" menu in the UI): registers participants, scales and materials, closes auctions, assembles credits, records sales, publishes entitlement trees. A single key |
| **payment intermediary** | the 3 configs | payment intermediary: attests escrow deposit, release and refund, receipt of the credit sale proceeds, and payouts |
| **registrar** | `CreditoConfig` | attests what happened in the official registry: issuance, rejection, retirement |
| **zupy** | `ReparticaoConfig` | coupon platform: co-signs redemptions |
| **credito_autoridade** | `GlobalConfig` | `ecol_credito`'s `["autoridade_credito"]` PDA |
| Collector, Cooperative, Clean Tech, Carrier, Industry, Importer | `Participante` | registered participants, identified by their primary wallet; other wallets (`Carteira`, with a name) can sign on their behalf (ADR 0011) |
| Scale | `Balanca` | device with its own Ed25519 keypair, which signs each weighing |
| payer | — | pays fees and rent; separate from whoever signs the act (EcolChain can pay for everyone) |

---

## 5. Complete flow, step by step

This is the cycle that the E2E test (`onchain/clients/e2e/ciclo-completo.test.ts`) runs on Surfpool.

**Setup (once)**
1. Deploy the 3 programs and run each one's `admin_init_*` (done by the deploy script).
2. `operador_register_participante` for each cooperative, carrier, industry and collector.
3. `operador_register_balanca` for the cooperatives' and industries' scales.
4. `operador_create_material` for each accepted material type.

In the web interface, steps 2 to 4 are under the **Administration** menu (Participants, Scales, Materials).

**Source lots**
5. Material arrives at the cooperative: from a collector (who is paid in R$ via Pix, off the blockchain, and shows their wallet's QR Code in the interface), from the cooperative's own sorting, a donation, a purchase or an independent waste picker. The scale weighs it and signs `(domínio, Entrega, entrega, peso_g, ts)`; the client builds `[Ed25519, cooperativa_register_entrega]`. Each weighing is a source lot.

**Sale lot**
6. `cooperativa_create_lote` opens the sale lot in `EmMontagem`.
7. `cooperativa_add_entregas` links source lots of the same material (up to 10 per transaction). The scale weighs the consolidated load and signs `(domínio, Origem, lote, peso_g, ts)`; `[Ed25519, cooperativa_fechar_lote]` closes the lot (`Criado`) and creates the asset. Until the sale, `cooperativa_desmontar_lote` undoes the combination.
8. `cooperativa_list_lote`. State: `Anunciado`.
9. The auction is settled off-chain. `industria_accept_venda` with the three signatures (industry, operator and payment intermediary). In the interface, each party signs on their own device, and the code is passed along by QR or copy and paste; the transaction uses a durable nonce owned by the operator, so it does not expire while it circulates. State: `Vendido`.
10. `transportador_pickup_lote`, signed by the cooperative and the carrier. In the interface, the cooperative signs and shows a QR Code; the carrier's device rebuilds the transaction, checks the signature, signs and submits it. State: `EmTransporte`, with the asset passing to the carrier.
11. `industria_confirm_recebimento`, with a weighing signed by the industry's scale. State: `Recebido` (or `EmDisputa` if the weight diverges), with the asset passing to the industry.
12. `intermediador_confirm_liberacao`. State: `Reciclado`.

**Credit**
13. `operador_create_credito`, then `operador_add_lotes` (in batches of up to 10) and `operador_submit_credito`.
14. `registrador_record_emissao`. State: `Emitido`.
15. `operador_record_venda` (operator + buyer + payment intermediary). State: `Vendido`.
16. `registrador_record_aposentadoria`. State: `Aposentado`.

**Revenue split**
17. The backend builds the entitlements tree with `montarDireitos` (`clients/src/arvore.ts`) and calls `operador_create_distribuicao`.
18. The payment intermediary transfers the amounts to Zupy and EcolChain and calls `intermediador_record_repasse`. State: `Financiada`.
19. Each beneficiary redeems with Zupy: `zupy_record_resgate` with the Merkle proof.

---

## 6. Off-chain formats that must match on-chain

### Message signed by the scale (Ed25519)

66 bytes, signed by the device's key:

| Bytes | Field |
|---|---|
| 0..17 | domain `"ECOLCHAIN:PESO:v1"` |
| 17 | type: `0` = `Origem` (consolidated sale lot, at closing), `1` = `Recebimento` (industry), `2` = `Entrega` (source lot, cooperative) |
| 18..50 | address of the `Lote` (or of the `Entrega`, for type 2) |
| 50..58 | `peso_g` (u64 LE) |
| 58..66 | `ts` (i64 LE, unix) |

The Ed25519 program instruction (one signature, data inside the instruction itself) must come **immediately before** the `ecol_lote` instruction. The timestamp must be within 15 min of the cluster clock, either way. Client implementation: `onchain/clients/src/pesagem.ts`.

### Entitlements Merkle tree

```
folha = sha256(0x00 ‖ indice (u32 LE) ‖ beneficiario (32) ‖ papel (u8) ‖ valor_centavos (u64 LE))
nó    = sha256(0x01 ‖ menor ‖ maior)            (sorted pairs: the proof needs no direction bits)
odd level: the last node moves up without a pair
papel: 0 = Coletor, 1 = Cooperativa, 2 = Transportador, 3 = Indústria
```

- **Rust:** `ecol_reparticao::merkle`
- **TypeScript:** `onchain/clients/src/arvore.ts` (a shared test vector guarantees identical hashes)

**Split within each role (`montarDireitos`):** collectors in proportion to the weight of the **source lots with a collector** linked to the credit's lots; cooperative, carrier and industry in proportion to the weight of the **lots**. It rounds down and distributes the remaining cents by largest remainder; on a tie, the lower address wins. Leaves are sorted by role and address, so any auditor can rebuild the root from the lots, the source lots and the collectors' `origem_ref → carteira` mapping.

### Source reference (source lot) and sale lot reference

Implementation: `onchain/clients/src/coletor.ts`.

- **With a collector:** `origem_ref = sha256("ECOLCHAIN:COLETOR:v1" ‖ carteira_do_coletor)` (`coletorRef`). Deterministic: the revenue split links each source lot to the wallet that receives the coupons without an extra table. The collector's registration data (legal name, CPF) stays off-chain, with only the KYC hash in the `Participante`. The wallet is pseudonymous and would appear in the redemptions anyway.
- **Without a collector (donation, purchase, sorting, independent):** `origem_ref = sha256("ECOLCHAIN:ORIGEM:v1" ‖ texto normalizado)` (`origemRef`), where the text is the invoice, the receipt or the CNPJ in uppercase, trimmed at both ends and with single internal spaces. With no text, the interface uses `<tipo>:<cooperativa>:<id>`.
- **Sale lot:** with an invoice or receipt, `evidencias_hash = origemRef(texto)`, the same format.

The text never goes on-chain. Whoever holds it (the donor, with the receipt) recomputes the hash and finds the lot in the public trail, via `memcmp` on `Entrega.origem_ref` (offset 48) or `Lote.evidencias_hash` (offset 110).

### Chained hash of the lot's deliveries

`entregas_hash` starts at zero and, for each linked delivery, becomes `sha256(entregas_hash ‖ endereço_da_entrega ‖ peso_g (u64 LE))`. It can be rebuilt from the `EntregaNoLote` events. The lot also stores `peso_entregas_g` (sum of the source lots) alongside `peso_g` (weighing of the consolidated load); the difference shows sorting losses and is bounded by the closing range. When the grouping is undone, each returned source emits `EntregaDevolvida`.

### Chained hash of the credit's lots

`lotes_hash` starts at zero and, for each aggregated lot, becomes `sha256(lotes_hash ‖ endereço_do_lote ‖ peso_g (u64 LE))`. It can be rebuilt from the `LoteAgregado` events.

---

## 7. Repository structure

```
README.md                       this file
CONTINUATION.md                 context, decisions and session history
docs/
  Proposta_03.excalidraw        original diagram
  Proposta_04.excalidraw        diagram with the 2026-09-28 decisions (open at https://excalidraw.com)
  adr/                          10 architecture decisions
  security-audit-2026-09-28.md  audit (solana-ai-kit checklists) and fixed findings
onchain/                        Anchor workspace
  Anchor.toml                   program IDs (localnet and devnet)
  rust-toolchain.toml           Rust 1.89.0
  programs/
    ecol_lote/src/
      lib.rs                    entry point (instructions)
      state/                    GlobalConfig, Participante, Balanca, Material, Entrega, Lote (+ state machine tests)
      instructions/             admin, participantes, balancas, materiais, entregas, lote_criacao (open, close, undo), marketplace, logistica, disputa, credito
      pesagem.rs                verification of the scale's Ed25519 signature
      ativo.rs                  Core asset (create, transfer and burn)
      events.rs, error.rs, constants.rs
      tests/                    LiteSVM tests
    ecol_credito/               state/, instructions/ (admin, agregacao, ciclo), tests/ (+ reusable tests/cenario/mod.rs)
    ecol_reparticao/            state/, instructions/ (admin, direitos), merkle.rs, tests/
  tests/fixtures/mpl_core.so    Metaplex Core program loaded in the LiteSVM tests
  clients/                      TypeScript clients (see section 10)
    src/generated/              Codama-generated clients (do not edit)
    src/pdas.ts, pesagem.ts, arvore.ts, coletor.ts, nome.ts
    scripts/gerar-clientes.ts   Codama generation
    scripts/init-devnet.ts      config initialization on devnet
    scripts/cadastrar-participante.ts   registers a wallet on devnet
    scripts/rede-local.ts       local demo network (Surfpool) for testing the interface
    test/                       tree generator tests
    e2e/                        full cycle on Surfpool
  ops/                          devnet operations (Squads SDK, web3.js v1 isolated here)
    comum.ts                    test keys, RPC
    chaves-devnet.ts            generates or reuses the test keys
    squads-devnet.ts            creates the multisig
    proposta.ts, propor-upgrade.ts, trocar-operador.ts, pausar.ts, retomar.ts, ...
  scripts/deploy-devnet.sh      full verifiable deploy
  .devnet/                      test keys and squads.json (NOT IN GIT)
  target/deploy/*-keypair.json  program keypairs (NOT IN GIT)
web/                            web interface: separate repository (NOT IN THIS GIT REPO)
```

---

## 8. Development environment

| Tool | Version used | Installation / notes |
|---|---|---|
| Rust | 1.89.0 | pinned in `onchain/rust-toolchain.toml` (`rustup` installs it automatically) |
| Solana CLI (Agave) | 3.1.10 | `sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"` |
| Anchor CLI | 1.1.2 | via `avm` |
| Node.js | ≥ 20.18 (tested with 24.10) | clients and operations scripts |
| Surfpool | 1.6.0 | only for the E2E (the `@solana/surfpool` npm package bundles the binary) |
| Docker | 29.x tested | required for `anchor build --verifiable` |
| `solana-verify` | — | `cargo install solana-verify` |

**WSL:** Docker Desktop needs integration enabled for the distro: *Settings → Resources → WSL integration*.

**Local wallet:** `~/.config/solana/id.json` (create it with `solana-keygen new` if it does not exist). To see the address, use `solana address`.

**Participants' wallet (web interface): use [Solflare](https://solflare.com).** It signs without altering the transaction and allows signing without sending, which the lot pickup requires: the cooperative and the carrier sign the same transaction on different devices. Wallets that add instructions when signing invalidate the other party's signature. Details in the README of the [web repository](https://github.com/ECOLchain/ecolchain-core-web3).

The `NO_DNA=1` variable used in the Anchor commands only turns off the CLI's telemetry and interactive prompts.

---

## 9. Build and tests

```bash
cd onchain

NO_DNA=1 anchor build                        # compiles the 3 programs; IDLs in target/idl, .so in target/deploy
NO_DNA=1 anchor test                         # = cargo test (skip_local_validator = true)
cargo test -p ecol_lote                      # a single program
cargo test -p ecol_credito -- venda          # filter by name
cargo clippy --all-targets -- -D warnings    # strict lint (must stay clean)
cargo fmt --check
```

The LiteSVM tests load the `.so` files from `target/deploy` and `tests/fixtures/mpl_core.so`, so **run `anchor build` first**.

| Type | Where | What it covers |
|---|---|---|
| Unit | `programs/*/src/state/*.rs` | full transition matrix (every state × transition pair), weight divergence at the boundary, US$ 50 example, rounding remainder, bitmap and cap |
| Property (proptest, 5,000 cases) | same | random sequences respect the state graph; symmetric and monotonic tolerance; the revenue split always balances and respects the percentages |
| LiteSVM integration | `programs/*/tests/` | flow, authorization (every wrong role fails), Ed25519 weighing and asset, audit regressions, compute units, CPI among the 3 programs, time via `Clock` |
| TS | `clients/test/` | tree generator (vector shared with Rust) |
| E2E | `clients/e2e/` | full cycle on Surfpool, with time travel and crank |

Current total: **165 Rust tests, 6 TS and 2 E2E**.

---

## 10. TypeScript clients

`@solana/kit` 8 clients generated by [Codama](https://github.com/codama-idl/codama) from the IDLs.

```bash
cd onchain/clients
npm install
npm run gerar       # regenerates src/generated from ../target/idl (run after anchor build)
npm run typecheck
npm test            # tree generator
npm run e2e         # embedded Surfpool: lot → credit → revenue split → redemptions
```

| Module | Use |
|---|---|
| `src/generated/<programa>` | instruction builders, account decoders and types (do not edit) |
| `src/pdas.ts` | derivation of all PDAs (mirrors the Rust seeds) |
| `src/pesagem.ts` | builds the scale's Ed25519 instruction and appends the writable accounts |
| `src/arvore.ts` | totals per role (`calcularTotais`, with the collectors' proportional cap) and entitlements tree (`montarDireitos`) |
| `src/coletor.ts` | source references: `coletorRef`, `origemRef`, `normalizarReferencia` |
| `src/nome.ts` | fixed 32-byte name of the participant and the scale (`bytesDoNome`, `lerNomeFixo`) |

**v0 transactions:** `@solana/kit-plugin-rpc` 0.18, required by `@solana/surfpool` 1.6, cannot build v1 yet. Once it can, we can migrate to v1.

**After changing a program:** run `anchor build`, then `npm run gerar`, `npm run typecheck` and the tests.

---

## 11. Devnet deploy

The `onchain/scripts/deploy-devnet.sh` script performs the full deploy and aborts on the first failure (`set -euo pipefail`).

### What the script does, in order

| # | Step | Detail |
|---|---|---|
| 1 | Checks | refuses an RPC containing "mainnet"; requires Docker, `solana-verify`, a wallet and **≥ 11 SOL** |
| 2 | Confirmation | asks `Publicar na devnet? [s/N]` ("Publish to devnet?"; "s" = yes; skip with `CONFIRMAR=1`) |
| 3 | Verifiable build | `anchor build --verifiable` in the `quay.io/ottersec/anchor:v1.1.2` image → `target/verifiable/*.so` |
| 4 | Test keys | `ops/chaves-devnet.ts`: operator, payment intermediary, registrar, zupy and 5 Squads members in `onchain/.devnet/` |
| 5 | Squads | `ops/squads-devnet.ts`: v4 multisig, 5 members, threshold 3, EcolChain keys, **no config authority** (changing members also requires 3 of 5), timelock 0 (ADR 0009) |
| 6 | Deploy | `solana program deploy` of the 3 verifiable `.so` files |
| 7 | Configs | `clients/scripts/init-devnet.ts`: `admin_init_*` of the 3 programs, **before** transferring the upgrade authority (initialization requires its signature) |
| 8 | Verification | on-chain executable hash = verifiable build hash, for each program |
| 9 | Upgrade authority | `solana program set-upgrade-authority` of the 3 to vault 0 |

Steps 4, 5 and 7 are **idempotent**: if the script stops midway, you can run it again. But after step 9, a new deploy with this script fails, because the local wallet is no longer the upgrade authority. From then on, upgrades go through Squads (section 13).

### Initial config parameters (devnet)

| Config | Value |
|---|---|
| `admin` (3 programs) | Squads vault 0 |
| `arbitro` | Squads vault 1 |
| `credito_autoridade` | PDA `["autoridade_credito"]` of `ecol_credito` |
| delivery deadline | 7 days |
| weight tolerance | 200 bps (2%) |
| percentages | collector 500, cooperative 1,000, carrier 500, industry 6,500 to 7,900 |

### How to run

```bash
# 1. balance of the wallet that pays for the deploy (default: ~/.config/solana/id.json)
solana balance -u devnet                   # needs ≥ 11 SOL
# faucet: https://faucet.solana.com  (or: solana airdrop 2 -u devnet, which is rate-limited)

# 2. back up the program keypairs (they define the addresses; not in git)
mkdir -p ~/backup-ecolchain && cp onchain/target/deploy/*-keypair.json ~/backup-ecolchain/

# 3. deploy (in an interactive terminal, answering "s" = yes)
cd onchain
scripts/deploy-devnet.sh

# or without the prompt (e.g. inside Claude Code with "!", which has no keyboard)
CONFIRMAR=1 scripts/deploy-devnet.sh
```

| Variable | Default | Use |
|---|---|---|
| `RPC_DEVNET` | `https://api.devnet.solana.com` | Devnet RPC. A dedicated RPC (Helius, Triton...) avoids rate limits. |
| `CARTEIRA` | `~/.config/solana/id.json` | payer and initial upgrade authority |
| `CONFIRMAR` | — | `1` skips the confirmation prompt |

The first run downloads the Docker image (a few GB) and takes 5 to 15 minutes.

---

## 12. Current devnet deployment

Initial deploy on **2026-09-28**; upgrades through Squads on the same day (materials, guardian, deliveries, participant and scale names, ADR 0010 in proposals #15 to #17, ADR 0011 in #18 on 2026-10-07, ADR 0012 in #19 on 2026-10-08, ADR 0013 in #20 and ADR 0014 in #21, both on 2026-10-09), all with a verifiable build and checked hashes.

### Programs

| Program | Program ID | Executable hash | Size |
|---|---|---|---|
| `ecol_lote` | [`AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD`](https://explorer.solana.com/address/AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD?cluster=devnet) | `9d99a79e947be4a66f728b5771a05a06d31a79a2aec29112fbfc582b032efc01` | 846,296 bytes |
| `ecol_credito` | [`FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ`](https://explorer.solana.com/address/FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ?cluster=devnet) | `4b73d878ce5ce3bf261637a486ba18a0556b94c33bd93230b5ba1cc7cee67332` | 484,456 bytes |
| `ecol_reparticao` | [`HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K`](https://explorer.solana.com/address/HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K?cluster=devnet) | `0fe26d24fef78369defdecb436548be095ceb0ac61be291754fc79a05ae5d62f` | 390,496 bytes |

### Configs

| Config | Address |
|---|---|
| `GlobalConfig` (`ecol_lote`) | `4LWZsMqmTujQiJeFcCczwfbm1WTfPi4pVXnmRUCfvpzm` |
| `CreditoConfig` (`ecol_credito`) | `5XK3ybpnPy6dpWZoSdN6Ryv9XmvSoi15v67btdCiCoKu` |
| `ReparticaoConfig` (`ecol_reparticao`) | `Da2rhTSvZvX94pGFswBApMcqqeUnXzKUJQyxfVLwQcif` |

### Governance (Squads v4)

| Item | Address |
|---|---|
| Multisig (3 of 5, EcolChain keys, timelock 0) | `9W2HFHrGQ4sTtCaRCSeZ2ydiyuGAzg1Exb5jYFw3ZFjd` |
| **Vault 0**: upgrade authority of the 3 programs + `admin` of the 3 configs | `5QLoaHGatRQnhiJxFBXhvkJ9GdDbzhCvySXwNCKiwXHj` |
| **Vault 1**: `arbitro` | `BKgC9T4fCyMA1AMe3fNJTquQjvufgjQcdCkXjjV1yh7G` |

Members (all from EcolChain; on devnet these are **test** keys, in `onchain/.devnet/`):

| Member | Address |
|---|---|
| ecolchain-diretoria | `HU6fB42zKw1aooH7AG6g7d6RhheEwmJaPP7GeBF7gjuX` |
| ecolchain-tecnologia | `3gTZMLVADFd4zmPd1KxoUYtD7vRCYgWJ7ZaUjP73zkQU` |
| ecolchain-operacoes | `4x9mnBjUvCZUomBeBEHBnXi64DtjSwBZFVgaCmZtbnQ3` |
| ecolchain-financeiro | `6fHCPsYdKQFevbTcmW4ct8SAGFdHE7wAVDxF3yyuCi9W` |
| ecolchain-reserva | `7bePYaoQiyqmjqD7Y5gyCCU8zbFkcCt963dJcbmn2UPL` |

### Role keys (test)

| Role | Address |
|---|---|
| operator (administration) | `EhY5n8NeRo8j3F8RsV2oZeC9HyzdMib9SXS475P6ec8X` (the owner's Solflare; until 2026-09-28 it was the test key `AURz…5ZGa`) |
| payment intermediary | `39J3HAavjhvt2vFLnTGNjh6L6F5PtD3UupKuveRq82Ez` |
| registrar | `p2e29Efu4TcPGqjeA9gzhW6mTqCtF8MzmZQjeiBYmYz` |
| zupy | `GTKWcFhronTQFAyE6sBwgtdFd12VJX4gjBAYE6RCH9cb` |

### Checking the on-chain state

```bash
solana program show AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD -u devnet   # Authority must be vault 0
solana-verify get-program-hash -u devnet AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD
cat onchain/.devnet/squads.json
```

---

## 13. Governance and operation with Squads

After the deploy, **no individual key** can change the programs or the configs. Every governance change is a Squads proposal, approved by 3 of the 5 members and executed after the timelock (0 on devnet, 24 h on mainnet). **All 5 members are EcolChain's own keys** (ADR 0009): cooperatives and industries are customers and do not vote; their trust comes from on-chain transparency and the verifiable build. Day-to-day operation does not go through Squads: it is handled by the operator wallet.

Web interface: the Squads v4 app (https://app.squads.so) with a member's wallet, pointed at devnet. You can also use the `@sqds/multisig` SDK in `onchain/ops/`.

### What goes through Squads and what does not

| Type of action | Who signs | Wait |
|---|---|---|
| **Day to day**: registering participants, scales and materials, closing auctions, assembling credits, publishing the revenue split | **operator** wallet (Administration menu in the interface), a single signature | none: takes effect immediately |
| **Governance**: program upgrades, revenue split percentages, deadlines and tolerances, replacing the operator, payment intermediary, registrar or arbiter, pausing | **Squads**: 3 of the 5 EcolChain keys approve | **timelock** (0 on devnet, 24 h on mainnet) |
| **Lot disputes**: `arbitro_resolve_disputa` | Squads vault 1 (3 of 5) | timelock |

### Timelock: what it is and why it exists

The timelock is a **mandatory waiting period between approval and execution** of a proposal. Even with the 3 votes, the transaction can only be executed after this period has elapsed, counted from the last approval.

**Lifecycle of a proposal:**

```
created ──► active ──► approved (3 of 5) ──[ timelock ]──► executable ──► executed
                 │                           │
                 └──► rejected               └──► cancelled (members back out before executing)
```

**Why it exists.** The 3-of-5 approval prevents **one** person at EcolChain from acting alone, but it offers no protection if **three keys are compromised** (theft, leak) or if three people collude on something improper. The timelock covers that case: every approved change becomes **public on-chain** before it takes effect, and during that window:

- other people at EcolChain, the cooperatives, the industries and the auditors can see exactly what is going to change;
- if it is improper, the members **cancel** the proposal before execution;
- anyone who disagrees with a new rule (e.g. revenue split percentages) has time to withdraw before it takes effect.

Example: someone steals 3 keys and approves an upgrade that diverts the revenue split. Without a timelock, the upgrade takes effect the same second. With a timelock, there is a window to detect and cancel it.

**The cost.** Every governance change takes at least the timelock duration. This is intentional and only affects rare, sensitive actions; day-to-day operations (table above) do not wait.

**Values:**

| Network | Timelock | Reason |
|---|---|---|
| devnet | **0** (was 1 h until 2026-09-28) | the programs still change frequently; protection comes from the 3-of-5 approval |
| mainnet | **24 h** | every approved change is public one day before it takes effect |

**How to track it.** `npx tsx proposta.ts status` (in `onchain/ops/`) lists each proposal, its status and **from when it can be executed**. An approved proposal **does not execute by itself**: after the waiting period, someone must run `npx tsx proposta.ts executar <índice>` (or execute it from the Squads app).

**How to cancel.** Until it is executed, an approved proposal can be cancelled by the members (3 of 5) in the Squads app (`proposalCancel` in the SDK).

**How to change the timelock.** The multisig was created **without a config authority**, so not even the timelock changes without a vote: it is a Squads *config transaction* (`npx tsx proposta.ts timelock <segundos>`), approved by 3 of 5 and subject to the **current** timelock (the change only takes effect after waiting out the current period one last time). This is how devnet went from 1 h to 0 on 2026-09-28. On devnet, without a timelock, "wait and cancel" cannot be tested; that is left for mainnet.

### Guardian: emergency pause by any member

With a timelock on mainnet (24 h), even a pause would wait a day. That is why there is a second path, only for **pausing**:

| | Main Squads (admin) | Guardian Squads |
|---|---|---|
| Members | the 5 EcolChain keys | **the same 5** |
| Threshold | 3 of 5 | **1 of 5** |
| Timelock | 0 (devnet) / 24 h (mainnet) | **0** |
| Can | everything (upgrade, configs, roles, pause and **unpause**) | **only pause** (`guardiao_pausar`) |

- The programs store the guardian Squads vault in the `guardiao` field of the configs. `guardiao_pausar` only accepts making the mode **more restrictive**; anything else is rejected (`GuardiaoSoRestringe`, `NaoEhGuardiao`).
- A compromised guardian, in the worst case, pauses the platform: a nuisance, but with no loss of funds or data, and the admin (3 of 5) reverts it.
- The guardian Squads member list must track the main one. It also has no config authority: with a threshold of 1, any member can change the members, so such a change must be agreed internally.

```bash
cd onchain/ops
npx tsx pausar.ts ecolchain-tecnologia            # any member: pauses the 3 programs immediately
npx tsx pausar.ts ecolchain-tecnologia --somente-disputas   # ecol_lote in SomenteDisputas (disputes and escrow keep working)
npx tsx retomar.ts                                # admin proposal (3 of 5) to lift the pause
npx tsx proposta.ts executar <índice>
```

On devnet: guardian Squads `6JpBi3MouEaouoUKH4HwBttREpWMvCNf2srDBUkTYYsq`, vault (guardian) `7tyuT6DDGUrTARsPzjgfPUEdEkV6SQVkwCRNs52B9GWz`. Created by `ops/guardiao-devnet.ts` and set in the programs by `ops/definir-guardiao.ts`.

### Proposal scripts (devnet)

On devnet, the 5 member keys are on this machine, so the scripts in `onchain/ops/` create **and approve** (3 of 5) the proposals on their own; all that remains is to wait for the timelock and execute:

```bash
cd onchain/ops
npx tsx proposta.ts status                  # proposals, status and when they can be executed
npx tsx trocar-operador.ts <carteira>       # replaces the operator (administration) in the 3 programs
npx tsx propor-upgrade.ts <programa>=<buffer> [...]   # upgrade (extends the program account if needed)
npx tsx proposta.ts executar <índice>       # after the timelock
```

The member who creates the proposal pays its rent (~0.0035 SOL), so it needs a little balance.

**Program larger than the reserved space.** If the new binary is larger, the program account must be extended before the upgrade. This extension **cannot go inside the proposal**: the runtime rejects `ExtendProgram`/`ExtendProgramChecked` invoked by Squads (error `not supported by inner instructions`). Since plain `ExtendProgram` is permissionless (minimum of 10,240 bytes), `propor-upgrade.ts` extends the account **beforehand**, paid by the local wallet, and the proposal carries only the `Upgrade`, which Squads can execute. Buffers left over from a proposal that will not be executed are reclaimed with `npx tsx fechar-buffers.ts <buffer> [...]`.

### Upgrading a program

```bash
cd onchain
NO_DNA=1 anchor build --verifiable

# 1. upload the new binary to a buffer (paid by the local wallet)
solana program write-buffer target/verifiable/ecol_lote.so -u devnet
#    → note the BUFFER address

# 2. hand the buffer over to vault 0 (it is the one that signs the upgrade)
solana program set-buffer-authority <BUFFER> \
  --new-buffer-authority 5QLoaHGatRQnhiJxFBXhvkJ9GdDbzhCvySXwNCKiwXHj -u devnet

# 3. in Squads: new program upgrade proposal using the <BUFFER>;
#    3 members approve; after the timelock, any member executes

# 4. verify
solana-verify get-executable-hash target/verifiable/ecol_lote.so
solana-verify get-program-hash -u devnet AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD
```

### Changing a config, pausing or replacing roles

1. Build the instruction with the generated client (e.g. `getAdminSetModoInstruction` from `ecol_lote`), using **vault 0** as `admin`.
2. Create a transaction proposal in Squads (vault 0) with that instruction.
3. Three members approve and, after the timelock, the proposal is executed.

### Emergency

- **Any EcolChain member**: `npx tsx pausar.ts <membro>` pauses the 3 programs immediately, through the guardian Squads (1 of 5, no timelock).
- Through the admin (3 of 5, with timelock): `admin_set_modo(SomenteDisputas | Pausado)` in `ecol_lote` and `admin_set_pausado(true)` in `ecol_credito` and `ecol_reparticao`.
- Lifting the pause is **always** done by the admin: `npx tsx retomar.ts`.
- Even in `SomenteDisputas`, disputes, escrow release and disaggregation keep working, so that withheld money is not locked.
- When returning to `Normal`, the expired-deadline crank waits **48 h** before dropping lots.

### Resolving a dispute (arbiter = vault 1)

1. Build `arbitro_resolve_disputa` with **vault 1** as `arbitro`.
2. If the decision is `Liberar` or `Reembolsar`, the payment intermediary also signs, so the proposal must include its signature.
3. 3-of-5 approval and execution.

---

## 14. Independent verification (auditors)

- **Code = binary:** anyone can recompile with `anchor build --verifiable` (same Docker image) and compare `solana-verify get-executable-hash` with `solana-verify get-program-hash`. With the repository on GitHub, `solana-verify verify-from-repo` can automate this (requires public access to the repository).
- **Trail:** every event carries `seq`, and a gap indicates a lost event.
- **Credit × lots:** rebuild `lotes_hash` from the `LoteAgregado` events and compare it with the `Credito` account.
- **Revenue split:** rebuild the Merkle root with `montarDireitos` from the lot weights and the collectors' `origem_ref → carteira` mapping. The per-role caps are in the `Distribuicao` account and the sum matches the revenue.
- **Weighings:** each source lot and each sale lot store the scale and the `ts_pesagem` signed by it.

---

## 15. Costs

| Item | Value |
|---|---|
| Deploy of the 3 programs + Squads + configs (devnet) | ≈ 7.5 SOL, almost all of it program rent |
| Per lot (`Lote` PDA + Core asset + transactions) | ≈ 0.0084 SOL (measured; see ADR 0003) |
| Fee per signature | 5,000 lamports (multi-signature transactions cost more) |

Rent is a recoverable deposit, not an expense, if the accounts are closed (see finding 13 in section 18). The Metaplex Core fee (~0.0015 SOL per asset) still needs to be confirmed.

---

## 16. Security

- **Never commit:** `onchain/.devnet/` (test keys and `squads.json`), `onchain/target/` (includes the program keypairs) and `~/.config/solana/id.json`. The first two are in `.gitignore`.
- **Back up the program keypairs:** `onchain/target/deploy/*-keypair.json` define the program addresses. There is currently a copy in `~/backup-ecolchain/`; keep another one off the machine.
- **The Squads keys on devnet are test keys**, all on the same machine. On mainnet, the 5 keys are still EcolChain's, but with **different people and storage locations** (preferably hardware wallets, one of them offline), and a 24 h timelock.
- **Isolated Squads SDK:** `@sqds/multisig` depends on `@solana/web3.js` v1, which has 4 transitive high-severity vulnerabilities in `npm audit`. That is why it stays only in `onchain/ops/` (operational use) and never goes into the clients or the backend.
- **Practices followed** ([solana-ai-kit](https://github.com/solanabr/solana-ai-kit)): checked arithmetic, no `unwrap` in production, no `init_if_needed`, owner, discriminator and seed validation, and an audit before closing each phase. Findings and fixes in [`docs/security-audit-2026-09-28.md`](docs/security-audit-2026-09-28.md).

---

## 17. Troubleshooting

| Symptom | Cause / solution |
|---|---|
| `Docker indisponível no WSL` | Docker Desktop → Settings → Resources → WSL integration → enable the distro and restart the terminal |
| `instale o solana-verify` | `cargo install solana-verify` |
| `saldo X SOL < 11 SOL` | top up at https://faucet.solana.com |
| Script exits right after listing the programs | the `[s/N]` prompt received no answer (e.g. `!` in Claude Code). Use `CONFIRMAR=1`. |
| Verifiable build takes very long the first time | download of the `quay.io/ottersec/anchor:v1.1.2` image; subsequent runs use the cache |
| `hash on-chain ≠ build` | the published `.so` is not the one in `target/verifiable/`. Redo the verifiable build and the upgrade. |
| LiteSVM tests fail to load the program | `anchor build` is missing (the tests read `target/deploy/*.so`) |
| TS client with outdated types | `anchor build && cd clients && npm run gerar` |
| `ProgramaPausado` | the mode is not `Normal` (or the config is paused); check the on-chain config |
| `EmCarencia` in the crank | the program returned to `Normal` mode less than 48 h ago |
| Weighing error in `register_entrega` or `fechar_lote` | the Ed25519 instruction is not immediately before it, the scale is inactive or belongs to another owner, or the timestamp is outside the 15 min window |
| `PesoConsolidadoForaDaFaixa` | the sale lot weight is outside the range of sum − 10% to sum + 2% of the source lots; check the weighing or undo the combination |
| `airdrop` fails | public faucet limit; use the website or another RPC |
| Deploying again after the first one fails | the upgrade authority is now vault 0; do the upgrade through Squads (section 13) |

---

## 18. Open items before mainnet

| # | Open item |
|---|---|
| Finding 13 | `close` accounts in a terminal state (recover the rent) |
| R1 | **reduced:** collectors are split by the weight of the source lots registered on-chain; the tree is still published by the operator and verified off-chain (reproducible by any auditor). |
| R2 | the industry percentage (`bps_industria`) is provided by the operator alone |
| R3 | unredeemed entitlements do not expire |
| Costs | confirm the Metaplex Core fee (~0.0015 SOL) |
| CI | repository on GitHub since 2026-09-28; CI with build, tests, clippy and `solana-verify verify-from-repo` is still missing |
| Governance | decide which people at EcolChain hold the 5 mainnet keys (3-of-5 threshold, 24 h timelock and 1-of-5 guardian already decided, ADR 0009) |
| Regulatory | classification of coupons, carbon credits and LGPD |
| Diagram | redraw the blockchain lane of `Proposta_04.excalidraw` with the state machine |
