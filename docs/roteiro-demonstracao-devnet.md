# Roteiro de demonstração na devnet

Demonstração da EcolChain com carteiras reais (Solflare) na **devnet**: do material que chega à cooperativa até a retirada pelo transportador, passando pelo lote de venda, pelo leilão e pela venda, com a trilha pública no fim. Duração: **30 a 35 minutos**, mais perguntas.

Estado da devnet levantado em 2026-09-28; telas de venda e retirada a partir de 2026-09-29. Antes da apresentação, confira de novo (seção 1.5).

---

## 1. Preparação (na véspera)

### 1.1 Carteiras

Todas na **Solflare**, com a rede da carteira em **Devnet**. As contas abaixo já estão cadastradas on-chain.

| Papel | Nome no cadastro | Carteira | Saldo | Paga taxas na demo? |
|---|---|---|---|---|
| Administração (operador) | — | `EhY5…ec8X` | 0,18 SOL | sim, se cadastrar algo ao vivo |
| Cooperativa | Cooperativa  01 | `kv3D…Jg3` | 0,39 SOL | **sim**: lotes de origem, lote de venda, anúncio, retirada |
| Coletor | Coletor  C01 | `AmpF…8Tv` | 0,01 SOL | não (só mostra o QR) |
| Transportador | Transportador T01 | `G1Un…kAHn` | 0,02 SOL | não (a cooperativa paga a retirada) |
| Indústria | Industria 01 | `6Kmc…Nw1fN` | 0,04 SOL | sim, só a taxa do recebimento (centésimos de centavo) |
| Intermediador de pagamento | — | `39J3…82Ez` | **0 SOL** | sim, só a taxa da liberação: envie **0,01 SOL** antes (chave de teste: importe na Solflare, 1.2) |

Custos de referência: cada lote de origem ≈ 0,002 SOL; cada lote de venda fechado, com o recibo digital, ≈ 0,0084 SOL. O saldo da cooperativa dá para dezenas de lotes. A **primeira venda** cria a conta de assinatura das vendas da administração (≈ 0,0015 SOL, uma vez só). Faucet: https://faucet.solana.com.

### 1.2 Um perfil de navegador por papel

Troque de conta criando **um perfil do Chrome para cada papel** (Administração, Cooperativa, Coletor, Transportador, Indústria e Intermediador), cada um com a Solflare importada na conta daquele papel.

**Intermediador:** na devnet ele é uma chave de teste, guardada em `onchain/.devnet/intermediador.json` (fora do git). Para importá-la na Solflare (Importar carteira → chave privada), gere a chave em base58 com:

```bash
cd onchain/clients && node -e "const {getBase58Decoder}=require('@solana/kit');console.log(getBase58Decoder().decode(Uint8Array.from(require('../.devnet/intermediador.json'))))"
```

Trate essa saída como senha: só serve na devnet, mas quem a tiver assina como intermediador. É mais confiável que trocar de conta dentro da mesma Solflare no meio da apresentação, e permite deixar duas janelas lado a lado, por exemplo cooperativa e transportador.

O **selo "Operando como"** no header, com a cor de cada papel, mostra ao público quem está na tela: verde para a cooperativa, âmbar para o coletor, azul para o transportador, roxo para a indústria e cinza-azulado para a administração.

### 1.3 A balança de teste fica no navegador da cooperativa

A pesagem é assinada pela chave da balança. Na devnet, essa chave é a "Balança de teste" gerada pelo navegador e guardada no `localStorage` do perfil que a gerou. A balança cadastrada é **"Balanca teste 01"** (`Ftcu…YxwBx`), da Cooperativa 01.

- **Use o mesmo navegador e o mesmo perfil** em que ela foi gerada. Em outro perfil ou computador, a tela Lotes de origem mostra a balança como não cadastrada.
- Se precisar trocar de computador: no perfil novo, em **Lotes de origem → Balança**, gere a balança de teste e peça à Administração que a cadastre em **Balanças**, com a Cooperativa 01 como proprietária. Leva 2 minutos, mas faça antes da apresentação.

**Balança da indústria:** o recebimento também é pesado. No perfil da Indústria, em **Recebimentos → Balança**, gere a balança de teste e peça à Administração que a cadastre em **Balanças**, com a Industria 01 como proprietária. Sem ela, o recebimento funciona, mas o peso fica "informado", sem atestação.

### 1.4 Ajustes de cadastro recomendados

Pela Administração, em **Participantes** e **Balanças**:

| O que | Por quê | Como |
|---|---|---|
| "Coletor  C01" e "Cooperativa  01" têm **dois espaços** | aparecem assim no selo e nas grades | clique no nome e corrija |
| "Cooperativa 02" (`657Y…vUFx`) está cadastrada com papel **Coletor** | o público vê "Cooperativa" no nome e "Coletor" no selo. O papel não muda depois do cadastro | renomeie para um nome de coletor (ex.: "Coletor C03") ou desative |
| Balança **sem nome** cujo dispositivo é `5P4Y…g5Uc`, a carteira do Coletor C02 | aparece na lista de balanças da cooperativa e confunde | desative ou dê um nome |

### 1.5 Ambiente

1. **App:** em `web/`, `npm run dev` e abra http://localhost:5173. Em `localhost`, a webcam do computador lê QR Codes sem https.
2. **Rede no app:** seletor de rede em **Devnet**, igual à rede da Solflare.
3. **RPC dedicado (recomendado):** o RPC público da devnet devolve erro 429 quando há muitas consultas seguidas, e a trilha pública consulta bastante. Crie uma chave gratuita num provedor (Helius, Triton, QuickNode) e rode `VITE_RPC_DEVNET=<url> npm run dev`.
4. **QR do coletor:** abra **Minha carteira** no perfil do Coletor numa segunda tela, num tablet ou num celular, ou **imprima o QR** num cartão. Catadores nem sempre têm smartphone, e o cartão impresso funciona igual, porque o QR é só o endereço da carteira.
5. **Conferir o estado:** Trilha pública → busque "Coletor C01"; devem aparecer os lotes de origem #1 e #3, dentro do lote de venda #1.

### 1.6 Ensaio

Faça o roteiro inteiro uma vez na véspera, com as mesmas carteiras e o mesmo computador. Os lotes criados no ensaio continuam existindo, o que é bom: a trilha e o "Meus lotes" ficam com mais conteúdo.

---

## 2. Roteiro ao vivo

Cada ato traz o perfil (carteira), o que fazer e o que falar.

### 2.1 Abertura: quem é quem (2 min)
**Perfil:** qualquer um, depois troque de janela.

- Mostre o selo **"Operando como"** e a faixa colorida no topo. Troque de janela entre Cooperativa, Coletor e Transportador.
- **Fala:** "Cada participante entra com a própria carteira. O menu e as permissões vêm da blockchain: o programa só aceita a operação se quem assina tem o papel certo."

### 2.2 Administração: cadastros (2 min)
**Perfil:** Administração.

- **Participantes:** grade com nome, papel, carteira e situação; busca e filtros.
- **Balanças:** a balança da cooperativa, com proprietário.
- **Materiais:** os 6 tipos cadastrados.
- **Fala:** "Nenhum dado pessoal vai para a blockchain: só o nome público, a carteira e um código derivado do cadastro (LGPD). O papel de uma carteira não muda depois de cadastrado."

### 2.3 Coletor: Minha carteira (1 min)
**Perfil:** Coletor, ou o cartão impresso.

- Mostre o QR Code, o nome e o endereço em blocos de 4.
- **Fala:** "O coletor recebe em R$ na hora (Pix, fora da blockchain) e, depois, uma parte da venda dos créditos de carbono. Para isso, a entrega precisa ficar no nome dele: é o que este QR garante."

### 2.4 Cooperativa: lotes de origem (4 min)
**Perfil:** Cooperativa.

1. **Lotes de origem → Novo lote de origem.** Origem **Coletor**, toque em **Ler QR** e aponte a webcam para o QR do coletor. O coletor aparece selecionado: "Coletor identificado".
2. Material **Plástico**, peso **12,5** kg → **Pesar e registrar** → assine na Solflare.
3. Registre um segundo lote de origem: origem **Doação**, referência **"Recibo 2026-001"**, Plástico, **8** kg.
4. **Fala:** "Cada pesagem é assinada pela balança da cooperativa, e o programa confere essa assinatura. Da doação, só um código derivado do recibo vai para a blockchain; com o recibo na mão, o doador encontra o material depois."

### 2.5 Cooperativa: lote de venda (4 min)
**Perfil:** Cooperativa.

1. **Lotes de venda → Montar lote.** Material **Plástico**; marque os dois lotes de origem do ato anterior (20,5 kg).
2. Peso do consolidado: **20** kg. O campo mostra a faixa aceita, de −10% a +2% da soma.
3. Em **Nota fiscal ou recibo**, digite **"NF 1001"** → **Montar, pesar e fechar**. A Solflare pede algumas assinaturas; a tela mostra o progresso.
4. Selecione o lote, clique em **Anunciar**, informe o preço mínimo e o prazo do leilão em dias, e assine.
5. **Fala:** "O lote de venda junta vários lotes de origem do mesmo material e é pesado de novo. Nesse momento nasce o **recibo digital (NFT)** do lote, que acompanha a custódia até a indústria. Antes da venda, a união pode ser desfeita: o recibo é queimado e as origens voltam a ficar disponíveis."

### 2.6 Coletor: Meus lotes (1 min)
**Perfil:** Coletor.

- **Meus lotes:** o lote de origem de 12,5 kg aparece **no lote de venda**, com a situação "Anunciado". (Volte a esta tela depois da retirada: a situação acompanha o lote.)
- **Fala:** "O coletor acompanha o próprio material até a indústria, sem depender de ninguém."

### 2.7 Venda (4 min)
**Perfis:** Administração → Intermediador → Indústria. Num computador só, o código passa por **Copiar código** e **Ou cole o código copiado**; em aparelhos diferentes, pela câmera.

1. **Administração → Leilões:** selecione o lote anunciado no ato 2.5 → **Registrar venda**. Indústria vencedora **Industria 01**, valor **1.250,00** (acima do preço mínimo), depósito **"PIX E2E 123"**, ata **"Ata 2026-10"** → **Assinar e gerar código**. Na primeira venda, a Solflare pede uma assinatura a mais: a criação da conta de assinatura das vendas.
2. **Intermediador → Escrow dos lotes → Assinar venda:** leia ou cole o código. A tela mostra lote, indústria, valor, referências e quem já assinou → **Assinar e confirmar depósito** → copie o novo código.
3. **Indústria → Compras → Aceitar compra:** leia ou cole o código → **Assinar e confirmar compra**. A venda vai para a blockchain; a tela da administração mostra "Venda do lote #… registrada" sozinha.
4. **Fala:** "Três partes assinam a mesma operação: a administração atesta o resultado do leilão, o intermediador atesta que o dinheiro está retido e a indústria aceita a compra. O dinheiro fica fora da blockchain, em R$; na blockchain fica a prova de quem assinou o quê. O código não expira com o tempo: cada um assina quando puder."

### 2.8 Retirada (3 min)
**Perfis:** Cooperativa e Transportador, lado a lado.

1. **Cooperativa → Retiradas:** o lote vendido aparece como **Aguardando retirada** → **Registrar retirada**. Identifique o transportador pelo QR da tela **Minha carteira** dele (webcam) ou pela lista → **Assinar e gerar código**.
2. **Transportador → Retiradas → Assinar retirada:** leia ou cole o código em até **1 minuto** → confira o lote → **Assinar e confirmar retirada**.
3. As duas telas mostram o lote **Em transporte**. Na trilha pública, o lote aparece com o transportador.
4. **Fala:** "A passagem de custódia é assinada pelas duas partes, e o recibo digital do lote passa para a carteira do transportador. Daqui em diante, a indústria confirma o recebimento com a balança dela."

### 2.8b Recebimento na indústria (2 min)
**Perfil:** Indústria.

1. **Recebimentos:** o lote retirado aparece **Em transporte**, com o prazo de entrega → **Registrar recebimento**.
2. A tela mostra o peso de saída e a faixa aceita sem disputa (±2%, fixada na venda). Digite primeiro um peso bem menor (ex.: 10% abaixo) para mostrar o aviso de disputa. Depois digite um peso dentro da faixa, com **Pesagem atestada pela balança da indústria** marcada, e assine.
3. O lote vai para **Recebido**, com ✓ de peso atestado, e o recibo digital passa para a carteira da indústria.
4. **Fala:** "A indústria pesa na chegada com a balança dela, que também assina. Se o peso não bater, o lote vai sozinho para disputa, e o árbitro decide. Ninguém pode 'ajustar' o peso depois."

### 2.8c Liberação do pagamento (2 min)
**Perfil:** Intermediador.

1. **Escrow dos lotes:** o filtro abre em **Aguardando liberação**, com o lote que a indústria acabou de receber.
2. **Liberar pagamento:** a tela mostra quem recebe (a cooperativa), quem pagou, o valor e os pesos de saída e de chegada. Informe a referência do repasse, por exemplo **"PIX E2E 456"**, e assine.
3. O lote passa a **Reciclado**; no filtro **Liberados**, aparece "Liberado à cooperativa".
4. **Fala:** "O dinheiro da venda ficou retido com o intermediador desde a compra e só vai para a cooperativa depois que a indústria confirma o recebimento. Com isso, o lote está pronto para entrar num crédito de carbono."

### 2.9 Trilha pública (3 min)
**Janela anônima, sem carteira.**

1. **Trilha pública**, busque **"Recibo 2026-001"**: aparece a doação → lote de venda → venda à indústria → transportador.
2. Busque **"NF 1001"**: o lote de venda com as duas origens, já recebido pela indústria.
3. Busque **"Coletor C01"**: os lotes de origem dele, inclusive os antigos.
4. Clique em **Ver na blockchain** num dos cartões: o registro no Solana Explorer.
5. **Fala:** "Qualquer pessoa confere a origem do material, sem login. O texto do recibo não está na blockchain: a página recalcula o código a partir do que foi digitado."

### 2.10 Fechamento: por que blockchain (2 min)

- Em **Lotes de venda**, clique em **Ver transação** de qualquer ação: o registro público, com quem assinou e quando.
- **Fala:**
  - "Os programas são públicos e verificáveis: qualquer auditor recompila o código e confere que é o mesmo que está rodando."
  - "Mudanças nos programas só com 3 de 5 aprovações da EcolChain (multisig Squads), e na rede principal com 24 h de espera pública."
  - "A receita do crédito de carbono é repartida por regra no próprio programa: 5% coletores, 10% cooperativa, 5% transportador, 65 a 79% indústria."

---

## 3. Se algo der errado

| Sintoma | O que fazer |
|---|---|
| O selo não mudou depois de trocar de conta na Solflare | desconecte e conecte de novo no botão da carteira, ou use o perfil do navegador daquele papel (1.2) |
| "A balança precisa estar cadastrada e ativa" | perfil errado do navegador (1.3) |
| "Não foi possível consultar a blockchain agora" na trilha | limite do RPC público: espere alguns segundos e busque de novo, ou use RPC dedicado (1.5) |
| "Fora da faixa aceita" ao montar o lote | use um peso entre −10% e +2% da soma; a própria mensagem mostra os limites em kg |
| A câmera não abre | a página precisa estar em `localhost` ou https; permita a câmera no navegador. Plano B: escolha na lista ou use Copiar/colar código |
| "O código venceu" (venda) | outra venda foi registrada depois dele; registre a venda de novo em Leilões |
| "O código expirou" (retirada) | a retirada vale ~1 minuto: gere um novo código e cole logo em seguida |
| "A carteira conectada não é a deste papel" | a Solflare está na conta errada para este passo; confira o selo |
| "Assinatura cancelada na carteira" | a assinatura foi recusada na Solflare, ou ela está em outra conta; confira o selo e repita |
| Transação demora | a devnet oscila; espere até 30 s antes de repetir, para não registrar duas vezes |

---

## 4. Depois da demonstração

Nada precisa ser desfeito: os lotes ficam na devnet e enriquecem as próximas demonstrações. Para liberar os lotes de origem de um lote de venda que não foi vendido, use **Desfazer** em Lotes de venda.
