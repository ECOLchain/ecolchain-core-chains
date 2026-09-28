# ADR 0010 — Lote de origem (com ou sem coletor) e lote de venda

- **Status:** aceita e implementada (2026-09-28)
- **Data:** 2026-09-28

## Contexto
Hoje o lote nasce de **entregas de coletores**: cada entrega (coletor, material, peso assinado pela balança) entra em um `Lote`, e o lote só vai a leilão com entregas. Na operação da cooperativa:

- parte do material **não vem de um coletor cadastrado**: triagem própria (coleta seletiva, equipe da cooperativa), doação de um gerador (empresa, condomínio, escola, PEV), compra de terceiro (sucateiro, outra cooperativa) ou catador avulso sem carteira;
- o que vai a leilão costuma ser a **união de vários lotes menores** do mesmo material, pesada de novo depois de consolidada;
- a rastreabilidade precisa atravessar essa união: da indústria que consumiu o lote até cada origem.

## Decisão
**Duas camadas fixas**, sempre dentro da **mesma cooperativa** e do **mesmo material**:

```
Lote de origem ──(união)──► Lote de venda ──► leilão ► venda ► retirada ► recebimento ► reciclado ► crédito
```

Os nomes técnicos no programa continuam `Entrega` (lote de origem) e `Lote` (lote de venda); a interface usa os nomes novos. Renomear as contas quebraria clientes, testes e as contas já existentes.

### Lote de origem (`Entrega`)
- Cada pesagem registrada na cooperativa é um lote de origem; não há camada intermediária (fardo).
- Novo campo `origem`: `Coletor | TriagemPropria | Doacao | Compra | Avulso`, no espaço reservado da conta. Contas antigas (zero) são lidas como `Coletor`.
- O campo de 32 bytes `coletor_ref` passa a ser a **referência da origem**:
  - `Coletor`: `sha256("ECOLCHAIN:COLETOR:v1" ‖ carteira)`, como hoje;
  - `Doacao` / `Compra`: hash da nota, recibo ou CNPJ;
  - `TriagemPropria` / `Avulso`: hash de uma referência opcional (ticket interno).

  Nenhum dado pessoal vai on-chain (LGPD).
- A pesagem assinada pela balança (Ed25519, tipo 2) continua obrigatória em todas as origens.

### Lote de venda (`Lote`)
Montagem em etapas, com a pesagem **depois** da união (hoje ela acontece na criação):

| Passo | Instrução | Estado |
|---|---|---|
| Criar vazio (material, evidências) | `cooperativa_create_lote`, sem pesagem | `EmMontagem` |
| Vincular lotes de origem (até 10 por transação) | `cooperativa_add_entregas` | `EmMontagem` |
| Pesar o consolidado e fechar | **nova** `cooperativa_fechar_lote` (pesagem Ed25519) | `Criado` |
| Anunciar / reanunciar | `cooperativa_list_lote` | `Anunciado` |

- **Sempre** via lote de venda, mesmo com um único lote de origem: um só fluxo de venda, logística e crédito.
- No fechamento, o programa confere o peso consolidado contra a soma dos lotes de origem: aceito entre **soma − 10%** (umidade, rejeito) e **soma + 2%** (erro de balança). Os dois limites ficam no config (`perda_max_bps = 1000`, `excesso_max_bps = 200`).
- O **Core asset** (NFT) existe só no lote de venda e nasce no fechamento. Os lotes de origem são apenas PDAs, mais baratos para centenas de registros.

### Desfazer a união
- **Nova** `cooperativa_desmontar_lote`, permitida em `EmMontagem`, `Criado` e `SemLance` (e depois de cancelar o anúncio). Nunca depois da venda.
- Devolve até 10 lotes de origem por transação, que voltam a ficar disponíveis. Enquanto restar algum, o lote fica em `EmDesmontagem` e não pode ser anunciado. Com o último, vai a `Desfeito`, o Core asset é queimado e o rent volta.

### Consumo e rastreabilidade
- A indústria recebe o lote de venda inteiro (pesagem de recebimento) e registra `Reciclado` uma vez (**consumo total**), como hoje.
- A trilha é navegável nos dois sentidos:
  - **para baixo:** crédito → lotes de venda → lotes de origem (`entrega.lote` e o `entregas_hash` encadeado) → coletor ou origem;
  - **para cima:** de um lote de origem até o crédito, pelo `lote.credito`.

### Repartição do crédito
- O lote de venda soma também o **peso com coletor** (`peso_coletores_g`) ao vincular origens; o crédito soma esse valor dos seus lotes.
- O teto dos coletores passa a ser proporcional: `floor(receita × 5% × peso_com_coletor / peso_total)`. A parte do peso sem coletor vai para a **EcolChain**, que já recebe o resíduo; não há regra nova de pagamento.
- A árvore Merkle continua dividindo a fatia dos coletores pelo peso das entregas de cada um.

## Consequências
- Material sem coletor entra na trilha com a mesma prova física (balança) e uma referência verificável da origem, sem cadastrar geradores ou vendedores.
- O peso vendido é o do fardo consolidado, e a diferença para a soma das origens fica registrada e limitada.
- Uma venda frustrada não prende os lotes de origem: a cooperativa desfaz e recombina.
- Afeta os três programas:
  - `ecol_lote`: origem, novos estados, fechar e desmontar;
  - `ecol_credito`: soma do peso com coletor;
  - `ecol_reparticao`: teto proporcional dos coletores.

  Upgrades na devnet por proposta Squads.
- Contas antigas continuam válidas: entregas viram origem `Coletor`, e lotes já pesados seguem o fluxo atual a partir de `Criado`.
- Interface:
  - "Entregas de coletores" vira **Lotes de origem** (popup com a escolha da origem; coluna e filtro "Origem");
  - "Meus lotes" vira **Lotes de venda** (montar → pesar e fechar → anunciar; desfazer);
  - nova página de **trilha pública** por lote de venda.

## Decisões complementares (2026-09-28)
1. **Tolerância da nova pesagem:** entre soma − 10% e soma + 2%, configurável (ver Lote de venda).
2. **Dados existentes na devnet:** todas as entregas já registradas são tratadas como origem `Coletor` (o campo novo é zero nas contas antigas).
3. **Comprovante ao doador:** a trilha pública permite buscar pela referência da doação: quem tem o texto da referência (nota, recibo) calcula o hash e encontra o lote de origem, o lote de venda, a indústria e a data do consumo. O texto nunca vai on-chain.
