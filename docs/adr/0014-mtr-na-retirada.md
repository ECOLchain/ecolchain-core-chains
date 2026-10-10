# ADR 0014 — Número do MTR na retirada do lote

- **Status:** aceita e implementada na rede local (2026-10-09); upgrade da devnet pendente
- **Data:** 2026-10-09

## Contexto
O usuário pediu que a indústria veja, na retirada de um lote comprado, o número do **MTR** (Manifesto de Transporte de Resíduos), com até 6 dígitos.

Decisões do usuário (2026-10-09):
- quem informa o MTR é **quem retira**: o transportador, ou a indústria quando a retirada é própria (ADR 0012);
- o MTR fica **on-chain e é obrigatório** na retirada.

Na retirada, a cooperativa assina primeiro e passa a transação pelo QR. A assinatura cobre a instrução inteira, então o MTR não pode ser acrescentado depois. Das duas saídas apresentadas, o usuário escolheu **"MTR antes, no código de quem retira"**. A outra era uma instrução separada, `informar_mtr`, numa segunda transação.

## Decisão

### Programa (`ecol_lote`)
- `Lote.mtr: u32` ocupa os últimos 4 bytes do `_reserved`, que acaba. O tamanho da conta não muda. Nas contas antigas o valor é zero, que significa sem MTR.
- `transportador_pickup_lote(mtr: u32)` exige `1 ≤ mtr ≤ 999.999` (`MTR_MAXIMO`; erro `MtrInvalido`) e grava o número no lote.

### Fluxo na interface
1. Quem retira abre **Retiradas › Código da retirada**, digita o MTR e mostra o código `ecolchain:retirador:v1:<carteira>:<mtr>`.
2. A cooperativa lê esse código em **Registrar retirada**. A escolha do transportador pela lista deixa de existir. A interface confere o papel (ou a indústria compradora, na retirada própria) e se o cadastro está ativo, monta a retirada com o MTR, assina e mostra o código `ecolchain:retirada:v3:…:<mtr>:<assinatura>`.
3. Quem retira lê esse código, confere o lote e o MTR, assina e envia.

O MTR aparece nas grades de Retiradas (cooperativa, transportador e indústria), em Compras e na Trilha pública.

## Consequências
- O `Lote` fica sem bytes reservados: o próximo campo exige `realloc` das contas existentes.
- O programa não confere o MTR no SINIR. O número é declarado por quem retira e fica registrado com a assinatura dele e a da cooperativa.
- Os lotes retirados antes deste upgrade ficam com MTR zero, mostrado como "—".
- O código antigo de retirada (`v2`) deixa de ser aceito.
