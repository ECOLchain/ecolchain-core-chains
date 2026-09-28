# ADR 0003 — Lote = Metaplex Core asset + PDA de estado

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
O NFT acumulava o papel de registro e de motor do processo; cNFT exige indexador DAS a cada transição.

## Decisão
O estado vive na PDA `Lote` (máquina de estados explícita). O lote é representado por um Core asset não transferível (freeze + transfer delegate permanentes do programa), usado só como recibo/identidade.

## Consequências
Transições não dependem de indexador. Custo ~0,003 SOL por lote, em grande parte rent recuperável (validar em devnet).
