# Clientes TypeScript — EcolChain

Clientes `@solana/kit` 8 gerados pelo Codama a partir das IDLs, mais os módulos de apoio que o backend usa.

| Caminho | Conteúdo |
|---|---|
| `src/generated/<programa>` | Clientes gerados (não editar; `npm run gerar` após `anchor build`) |
| `src/pdas.ts` | Endereços derivados dos três programas |
| `src/pesagem.ts` | Instrução Ed25519 da balança (mesmo formato de `pesagem.rs`) e anexo de contas graváveis |
| `src/coletor.ts` | `coletorRef(carteira)`: referência on-chain do coletor nas entregas (`sha256("ECOLCHAIN:COLETOR:v1" ‖ carteira)`) |
| `scripts/rede-local.ts` | rede local de demonstração (Surfpool) já com configs, cooperativa, coletores, materiais e balança |
| `src/arvore.ts` | Totais por papel e árvore de direitos determinística (mesmo hash de `ecol_reparticao::merkle`) |
| `e2e/` | Ciclo completo no Surfpool embarcado |

```bash
npm install
npm run gerar      # regenera os clientes a partir de ../target/idl
npm run typecheck
npm test           # gerador da árvore (inclui vetor compartilhado com o Rust)
npm run e2e        # Surfpool: lote → crédito → repartição → resgates; time travel + crank

# cadastrar uma carteira como participante na devnet, assinando com a chave local .devnet/operador.json
# (só funciona enquanto ela for o operador; com o operador na sua carteira, use a tela Participantes da interface)
npx tsx scripts/cadastrar-participante.ts <carteira> <coletor|cooperativa|transportador|industria> [referencia-kyc]
```

**Divisão dentro de cada papel (`montarDireitos`)**: proporcional ao peso dos lotes do crédito; arredonda para baixo e distribui os centavos restantes pelo maior resto (empate: endereço crescente). Folhas ordenadas por papel e endereço. Qualquer auditor reconstrói a raiz a partir dos lotes e do mapeamento `coletor_ref → carteira`.

**Transações**: v0 por enquanto — o planner do `@solana/kit-plugin-rpc` 0.18 (exigido pelo `@solana/surfpool` 1.6) ainda não monta v1.
