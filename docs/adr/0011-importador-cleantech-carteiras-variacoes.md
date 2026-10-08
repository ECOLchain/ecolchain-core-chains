# ADR 0011 — Importador de vinhos, Clean Tech, várias carteiras por ator e variações do material

- **Status:** aceita e implementada (2026-10-07)
- **Data:** 2026-10-07

## Contexto
Quatro pedidos do usuário mudam a rede:

1. **Importador de vinhos.** É um ator novo que põe garrafas no mercado e responde pela logística reversa delas. Ele gera lotes de distribuição, coleta garrafas, entrega a uma cooperativa ou Clean Tech e informa a reciclagem com a NF. A página inicial dele é um painel que compara garrafas distribuídas e garrafas recicladas.
2. **Clean Tech.** É outro destino das garrafas, além da cooperativa.
3. **Várias carteiras por ator.** Cada carteira tem nome (balcão, motorista, portaria). Até aqui, uma carteira era um participante.
4. **Vidro com cores.** As variações são Transparente, Verde e Marrom. O lote de vidro também informa a **quantidade aproximada de garrafas**, além do peso.

Decisões do usuário (2026-10-07):
- a Clean Tech funciona como a cooperativa;
- a trilha do importador se integra ao fluxo atual;
- todas as carteiras assinam pelo ator;
- o material passa a ter variações, e o lote de venda não mistura cores.

## Decisão

### Compatibilidade
Os campos novos de contas existentes (`Material`, `Entrega`, `Lote`) ocupam o `_reserved`, então o tamanho e o layout dessas contas não mudam. Os enums `Papel` e `OrigemEntrega` ganham variantes só no fim.

A identidade do ator continua sendo a **carteira titular**: `Participante.carteira`, nas seeds de `Lote` e `Entrega` e nos campos `cooperativa`, `industria` e `transportador`. Lotes, créditos e direitos de repartição já gravados continuam válidos.

### Papéis
- `Papel::Importador` e `Papel::CleanTech`.
- **Clean Tech = cooperativa** para o programa: `Papel::eh_recebedor()` aceita os dois em lotes de origem, lotes de venda, anúncio, venda e retirada, e a Clean Tech pode ter balança.
- No `ecol_reparticao`, `balde(papel)` põe a Clean Tech na fatia da cooperativa (10%). O importador não tem fatia no crédito. No hash da folha, `papel_u8` ganha os valores 4 e 5; os valores 0 a 3 não mudam.

### Carteiras vinculadas (`Carteira`, PDA `["carteira", endereco]`)
- Campos: `endereco`, `participante` (titular), `nome`, `ativa`. Como a PDA é derivada só do endereço, **uma carteira tem no máximo um vínculo**. A do próprio titular também pode existir, só para dar nome a ela.
- `participante_add_carteira` e `participante_update_carteira` (nome, ativa) são assinadas pelo **operador ou pelo titular**. Uma carteira vinculada não gerencia outras carteiras.
- Regras de exclusividade:
  - um endereço que é titular de outro cadastro não vira carteira vinculada;
  - um endereço vinculado não vira titular (`operador_register_participante` exige `["carteira", carteira]` vazia);
  - chaves de papel do config ficam de fora.
- Nas instruções assinadas por participante, o par `X: Signer` passou a ser três contas:
  - `X` (titular, `UncheckedAccount`, mantém as seeds);
  - `X_assinante: Signer`;
  - `X_carteira: Option<Carteira>`, com seeds `["carteira", assinante]`.
  
  `exigir_assinatura` aceita o próprio titular, ou uma carteira **ativa** vinculada a ele.
- A custódia do Core asset e os campos do lote ficam com o titular, nunca com a carteira que assinou.
- O gerador dos clientes (Codama) não deriva automaticamente contas opcionais. Sem essa regra, o cliente mandaria a PDA `["carteira", titular]` inexistente quando o titular assina.

### Variações do material e garrafas
- `Material.qtd_variacoes` e `Material.conta_garrafas` foram adicionados.
- Conta `MaterialVariacao` (PDA `["variacao", codigo, indice]`, índices 1..16, nome e `ativa`), com as instruções:
  - `operador_create_variacao`;
  - `operador_update_variacao`;
  - `operador_set_material_garrafas`.
- `Entrega` e `Lote` ganham `variacao` (0 = sem variação) e `qtd_garrafas`.
- Material com variações exige variação ativa no lote de origem e no lote de venda.
- `cooperativa_add_entregas` exige a mesma variação e soma as garrafas. Desfazer subtrai as garrafas.
- Garrafas só são aceitas em material que conta garrafas.
- O crédito de carbono continua agregando por material, sem olhar a cor.

### Logística reversa do importador
```
Distribuicao (garrafas postas no mercado)

Coleta: Coletada → EmEntrega ──(destino pesa: Entrega origem Importador)──► Entregue ──(lote Reciclado + NF)──► Reciclada
                   EmEntrega → Coletada (cancelamento)
```
- `Distribuicao` (PDA `["distrib", importador, id]`) guarda material, variação, garrafas, peso estimado e o hash da NF/DI de importação.
- `Coleta` (PDA `["coleta", importador, id]`) guarda:
  - material, variação, garrafas e peso estimado;
  - o hash do ponto de coleta (LGPD);
  - o destino, a entrega e o hash da NF da reciclagem.
- Instruções do importador:
  - `importador_create_distribuicao`;
  - `importador_register_coleta`;
  - `importador_entregar_coleta`, cujo destino é uma cooperativa ou Clean Tech ativa;
  - `importador_cancelar_entrega`;
  - `importador_informar_reciclagem`.
- O importador **não pesa**. O peso que vale é o da balança do destino: em `cooperativa_register_entrega` com `origem = Importador`, a coleta entra na transação e precisa cumprir quatro condições:
  - estar `EmEntrega`;
  - ter este destino;
  - ter o mesmo material e a mesma cor;
  - ter `origem_ref` igual à chave da coleta.
  
  A coleta então vira `Entregue`. As garrafas da entrega são as contadas pelo destino.
- A NF da reciclagem só é aceita quando o lote de venda que recebeu a origem está `Reciclado` ou `Agregado`.

## Consequências
- As instruções assinadas por participante mudaram de contas. Clientes, interface, testes e scripts foram atualizados. É preciso fazer upgrade dos três programas na devnet (Squads), com ampliação da conta do `ecol_lote`.
- O painel do importador lê as contas `Distribuicao` e `Coleta` (filtro pelo importador no offset 8) e os lotes ligados a elas.
- **Riscos em aberto:**
  - **Distribuição autodeclarada.** As garrafas distribuídas são informadas pelo importador e não há conferência on-chain com a NF/DI. A taxa de recuperação mostrada depende dessa declaração.
  - **Contagem de garrafas aproximada.** O peso atestado continua sendo a medida canônica.
  - **Carteiras sem limite de alçada.** Uma carteira vinculada assina tudo pelo ator. Limites por carteira, como um valor máximo, ficam para quando houver pedido.
  - **Vínculo sem prova de posse.** O titular vincula um endereço sem que essa carteira assine. Assim, um participante poderia vincular o endereço de outra pessoa e impedi-la de se cadastrar como titular (a PDA continua existindo mesmo desativada). Não há ganho para quem faz isso, porque a carteira alheia nunca assinaria por ele, e o operador vê todos os vínculos. Antes da mainnet, decidir entre exigir a assinatura da carteira nova ou criar um `operador_close_carteira`.
