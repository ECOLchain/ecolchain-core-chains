# ADR 0003 — Lote = Metaplex Core asset + PDA de estado

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
O NFT acumulava o papel de registro e de motor do processo; cNFT exige indexador DAS a cada transição.

## Decisão
O estado vive na PDA `Lote` (máquina de estados explícita). O lote é representado por um Core asset não transferível (freeze + transfer delegate permanentes do programa), usado só como recibo/identidade.

## Consequências
Transições não dependem de indexador.

**Custo medido (LiteSVM, 2026-09-28):** Core asset = 0,00402 SOL (234 bytes; ~0,0025 SOL de rent + ~0,0015 SOL que aparenta ser taxa de protocolo do Core, não recuperável — confirmar) e PDA `Lote` = 0,00434 SOL (496 bytes, recuperável só com uma instrução `close`, ainda não implementada). Total ≈ 0,0084 SOL/lote ≈ US$ 1,25 com SOL a US$ 150 — acima da estimativa de ~US$ 0,50 do diagrama v04.

**Comportamento confirmado em teste:** o `PermanentTransferDelegate` transfere o asset mesmo congelado pelo `PermanentFreezeDelegate`; o dono não consegue transferir nem queimar.
