# EcolChain

Reciclagem rastreável na Solana, com repartição da receita dos créditos de carbono.

**Fluxo físico:** Coletor → Cooperativa (pesa e classifica) → Transportador → Indústria.
**Trilha on-chain:** lote pesado por balança atestada → anúncio e venda com escrow (R$ off-chain) → retirada → recebimento → reciclado → agregado a um crédito → emissão no registro (Verra / Gold Standard) → venda → repartição da receita → resgate em cupons Zupy.

| Participante | Parte da receita do crédito | Forma |
|---|---|---|
| Coletor | 5% | cupons Zupy (além de R$ via Pix na entrega) |
| Cooperativa | 10% | cupons Zupy |
| Transportador | 5% | cupons Zupy |
| Indústria | 65% a 79% (negociado) | cupons Zupy |
| EcolChain | restante (1% a 15%) | R$ / stablecoin |

As decisões de arquitetura estão em [`docs/adr/`](docs/adr/README.md) e o estado do trabalho em [`CONTINUATION.md`](CONTINUATION.md).

---

## Estrutura

```
docs/
  Proposta_04.excalidraw        diagrama de referência (Proposta_03 = original)
  adr/                          8 decisões de arquitetura
  security-audit-2026-09-28.md  auditoria da F2 e achados corrigidos
onchain/                        workspace Anchor
  programs/
    ecol_lote/                  lote: máquina de estados, balança Ed25519, Core asset, escrow, disputas
    ecol_credito/               crédito: agregação de lotes, emissão, venda, aposentadoria
    ecol_reparticao/            repartição: tetos por papel + árvore Merkle de direitos, resgates Zupy
  tests/fixtures/mpl_core.so    programa Metaplex Core usado nos testes LiteSVM
  clients/                      clientes TS (Codama + @solana/kit 8), gerador da árvore, E2E Surfpool
  ops/                          operação da devnet: chaves de teste e Squads (SDK web3.js v1 isolado)
  scripts/deploy-devnet.sh      deploy verificável na devnet
```

### Programas

| Programa | Contas principais | Instruções |
|---|---|---|
| `ecol_lote` | `GlobalConfig`, `Participante`, `Balanca`, `Lote` (+ Core asset `["asset", lote]`) | 21 |
| `ecol_credito` | `CreditoConfig`, `Credito` `["credito", id]`, serial `["serial", padrão, sha256(serial)]` | 13 |
| `ecol_reparticao` | `Config`, `Distribuicao` `["dist", credito]` | 8 |

Endereços (localnet e devnet, definidos pelas keypairs em `onchain/target/deploy/`):

| Programa | Program ID |
|---|---|
| `ecol_lote` | `AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD` |
| `ecol_credito` | `FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ` |
| `ecol_reparticao` | `HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K` |

As instruções seguem o padrão `<papel>_<ação>` (`cooperativa_create_lote`, `industria_confirm_recebimento`, `zupy_record_resgate`...). O prefixo diz quem assina. `public_*` são cranks que qualquer um pode chamar.

**Estados do lote:** `Criado → Anunciado → Vendido → EmTransporte → Recebido → Reciclado → Agregado`, com `EmDisputa` e `Reembolsado`.
**Estados do crédito:** `EmFormacao → EmValidacao → Emitido → Vendido → Aposentado`, ou `Rejeitado` / `Abandonado` (180 dias). Nesses dois casos, `public_release_lotes` devolve os lotes a `Reciclado`.
**Estados da distribuição:** `Pendente → Financiada`.

---

## Pré-requisitos

| Ferramenta | Versão usada |
|---|---|
| Rust | 1.89.0 (fixado em `onchain/rust-toolchain.toml`) |
| Solana CLI (Agave) | 3.1.10 |
| Anchor CLI | 1.1.2 |
| Node.js | ≥ 20.18 (testado com 24) |
| Surfpool | 1.6.0 (só para o E2E) |
| Docker | necessário para `anchor build --verifiable` |
| `solana-verify` | `cargo install solana-verify` |

No WSL, o Docker Desktop precisa da integração ativada para a distro: **Settings → Resources → WSL integration**.

---

## Build e testes

```bash
cd onchain
NO_DNA=1 anchor build        # compila os 3 programas e gera as IDLs em target/idl
NO_DNA=1 anchor test         # roda `cargo test`: unitários, LiteSVM e proptest
cargo clippy --all-targets -- -D warnings
```

Os testes LiteSVM carregam os `.so` de `target/deploy`, então rode `anchor build` antes. Eles cobrem fluxo, autorização, pesagem, regressões da auditoria, custo em compute units, CPI entre os 3 programas e passagem de tempo (via `Clock`).

### Clientes TypeScript

```bash
cd onchain/clients
npm install
npm run gerar       # regenera src/generated a partir de ../target/idl (após anchor build)
npm run typecheck
npm test            # gerador da árvore de direitos, com vetor compartilhado com o Rust
npm run e2e         # Surfpool embarcado: lote → crédito → repartição → resgates, time travel + crank
```

Detalhes em [`onchain/clients/README.md`](onchain/clients/README.md). As transações são v0 porque o `@solana/kit-plugin-rpc` 0.18, exigido pelo Surfpool 1.6, ainda não monta v1.

---

## Deploy na devnet

O script `onchain/scripts/deploy-devnet.sh` faz tudo em ordem e aborta na primeira falha:

1. **Checagens:** recusa RPC de mainnet, exige Docker, `solana-verify`, a carteira e **pelo menos 11 SOL** de saldo.
2. Pede confirmação (ou use `CONFIRMAR=1`).
3. `anchor build --verifiable` (gera `target/verifiable/*.so`).
4. `ops/chaves-devnet.ts`: gera ou reaproveita as chaves de teste (operador, intermediador, registrador, zupy e 5 membros do Squads) em `onchain/.devnet/`.
5. `ops/squads-devnet.ts`: cria o Squads v4 com 3 de 5, sem config authority e timelock de 1 h. **Vault 0** = upgrade authority + `admin`; **vault 1** = árbitro.
6. `solana program deploy` dos 3 programas.
7. `clients/scripts/init-devnet.ts`: inicializa as configs. Isso precisa ser feito antes de transferir a upgrade authority, porque a inicialização exige a assinatura dela.
8. Confere se o hash on-chain é igual ao do build verificável, para cada programa.
9. Transfere a upgrade authority dos 3 programas para o vault 0.

```bash
# 1. saldo (a carteira padrão é ~/.config/solana/id.json)
solana balance -u devnet            # precisa de ≥ 11 SOL; complete em https://faucet.solana.com

# 2. backup das keypairs dos programas (definem os endereços; ficam fora do git)
cp -r onchain/target/deploy ~/backup-ecolchain-deploy-keypairs

# 3. deploy
cd onchain
scripts/deploy-devnet.sh
```

| Variável | Padrão | Uso |
|---|---|---|
| `RPC_DEVNET` | `https://api.devnet.solana.com` | RPC da devnet (um RPC dedicado evita limites) |
| `CARTEIRA` | `~/.config/solana/id.json` | pagador e upgrade authority inicial |
| `CONFIRMAR` | — | `1` pula a pergunta de confirmação |

Os scripts de Squads e de init são idempotentes: se o deploy parar no meio, basta rodar de novo. Endereços do multisig e dos vaults ficam em `onchain/.devnet/squads.json`.

**Segurança:**
- `onchain/.devnet/` e `onchain/target/` contêm chaves privadas e estão no `.gitignore`. Nunca versione esses diretórios.
- As chaves do Squads na devnet são **de teste**. Na mainnet, cada membro (EcolChain, cooperativas, indústria, auditor...) usa a própria carteira, e o timelock deve ser maior (ex.: 48 h).
- O SDK do Squads usa `@solana/web3.js` v1, que tem vulnerabilidades transitivas no `npm audit`. Por isso ele fica isolado em `onchain/ops/`, usado só para operação, e nunca entra nos clientes.

---

## Governança

- Upgrade dos programas, mudanças de config e troca de admin passam pelo **Squads multisig com terceiros** + timelock (ADR 0008).
- Troca de admin em dois passos: `admin_propose_admin` → `admin_accept_admin`.
- Modos de operação do `ecol_lote`: `Normal`, `SomenteDisputas` (disputas, crank e árbitro continuam) e `Pausado`. `ecol_credito` e `ecol_reparticao` têm `admin_set_pausado`.

---

## Pendências antes da mainnet

- **Achado 13:** `close` das contas em estado terminal (recuperar rent).
- **R1:** a divisão dentro de cada papel depende do operador (a árvore é reproduzível pelo gerador, mas não é verificada on-chain).
- **R2:** o percentual da indústria é informado só pelo operador.
- **R3:** direitos não resgatados não expiram.
- Confirmar a taxa do Metaplex Core (~0,0015 SOL). O custo medido é ≈ 0,0084 SOL por lote (ADR 0003).
- CI (o repositório ainda não tem remoto).
- Enquadramento regulatório: cupons, créditos de carbono e LGPD (on-chain só entram hashes com salt).

---

## Documentação

- [`CONTINUATION.md`](CONTINUATION.md): contexto, decisões, estado das fases e histórico.
- [`docs/adr/`](docs/adr/README.md): decisões de arquitetura (escrow em R$, sem token próprio, Core asset, carteira embutida, crédito como registro, coletor, Zupy, governança).
- [`docs/security-audit-2026-09-28.md`](docs/security-audit-2026-09-28.md): auditoria com os checklists do [solana-ai-kit](https://github.com/solanabr/solana-ai-kit).
- `docs/Proposta_04.excalidraw`: diagrama do fluxo (abra em https://excalidraw.com).
