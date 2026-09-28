# ADR 0004 — Carteira embutida/MPC por participante; EcolChain como fee payer

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
Se a EcolChain assinasse por todos, a trilha perderia valor probatório.

## Decisão
Cada participante tem carteira própria embutida no app (MPC). Cada instrução exige a assinatura de quem executa a etapa; o operador da EcolChain paga taxas e rent (conta `payer` separada da autoridade).

## Consequências
Participantes não precisam de SOL. O provedor de carteira embutida entra como dependência.

**Adendo (auditoria 2026-09-28, achado 2):** o fee payer (patrocinador/Kora) é uma chave **separada** do `operador` e não tem nenhum papel on-chain. Se a mesma chave pagasse taxas e fosse autoridade, co-assinar como pagador uma transação montada pelo cliente autorizaria instruções de operador embutidas nela. O `operador` nunca co-assina transações de terceiros; o relayer só aceita transações com instruções e contas em whitelist.
