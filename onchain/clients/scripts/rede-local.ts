// Rede local de demonstração (Surfpool embarcado): os três programas + Metaplex Core, configs,
// uma cooperativa, dois coletores, um transportador, uma indústria, materiais e a balança de teste da cooperativa já
// cadastrados (com a balança da indústria), e quatro lotes de venda: #1 vendido (esperando a retirada), #2 em leilão,
// #3 em transporte (esperando o recebimento) e #4 recebido (esperando a liberação do escrow).
// ADR 0011: vidro com três cores e garrafas, um importador com duas carteiras (seis meses de distribuições e
// coletas em todas as etapas) e uma Clean Tech com balança e um lote de vidro verde já reciclado.
// O JSON de saída traz as sementes de todos os papéis usados na interface.
// Fica rodando até Ctrl+C. Útil para testar a interface web sem depender da devnet.
//
// Uso: npx tsx scripts/rede-local.ts [arquivo-de-saída.json]
//   Grava no arquivo: rpcUrl, wsUrl, e as chaves de teste (cooperativa, balança, coletores e transportador; semente do coletor 1).
//   Na interface: VITE_RPC_LOCALNET=<rpcUrl> VITE_WS_LOCALNET=<wsUrl> npm run dev, rede "Localnet".
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
    type Address,
    address,
    createClient,
    createKeyPairSignerFromPrivateKeyBytes,
    generateKeyPairSigner,
    getAddressEncoder,
    getProgramDerivedAddress,
    type Instruction,
} from '@solana/kit';
import { surfpool } from '@solana/surfpool/kit';
import * as credito from '../src/generated/ecol_credito';
import * as lote from '../src/generated/ecol_lote';
import * as rep from '../src/generated/ecol_reparticao';
import { credito as pCredito, eventAuthority, lote as pLote } from '../src/pdas';
import { comContasGravaveis, instrucaoPesagem, TipoPesagem } from '../src/pesagem';
import { coletorRef } from '../src/coletor';
import { fetchSysvarClock } from '@solana/sysvars';

const raiz = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const MPL_CORE = address('CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d');
const LOADER = address('BPFLoaderUpgradeab1e11111111111111111111111');
const h = (s: string) => new Uint8Array(createHash('sha256').update(s).digest());
const saida = process.argv[2] ?? join(raiz, '.surfpool-demo.json');

const client = await createClient().use(surfpool({ transactionConfig: { version: 0 } }));
const enviar = (...ixs: Instruction[]) => client.sendTransaction(ixs);
const semente = () => crypto.getRandomValues(new Uint8Array(32));

const [upgrade, admin, arbitro, registrador, zupy] = await Promise.all(Array.from({ length: 5 }, () => generateKeyPairSigner()));
const sementes = {
    cooperativa: semente(),
    balanca: semente(),
    coletor1: semente(),
    coletor2: semente(),
    transportador: semente(),
    industria: semente(),
    operador: semente(),
    balancaIndustria: semente(),
    intermediador: semente(),
    importador: semente(),
    importadorLoja: semente(),
    cleantech: semente(),
    balancaCleantech: semente(),
};
const operador = await createKeyPairSignerFromPrivateKeyBytes(sementes.operador);
const intermediador = await createKeyPairSignerFromPrivateKeyBytes(sementes.intermediador);
const cooperativa = await createKeyPairSignerFromPrivateKeyBytes(sementes.cooperativa);
const balanca = await createKeyPairSignerFromPrivateKeyBytes(sementes.balanca);
const coletores = await Promise.all([sementes.coletor1, sementes.coletor2].map((s) => createKeyPairSignerFromPrivateKeyBytes(s)));
const transportador = await createKeyPairSignerFromPrivateKeyBytes(sementes.transportador);
const industria = await createKeyPairSignerFromPrivateKeyBytes(sementes.industria);
const importador = await createKeyPairSignerFromPrivateKeyBytes(sementes.importador);
const importadorLoja = await createKeyPairSignerFromPrivateKeyBytes(sementes.importadorLoja);
const cleantech = await createKeyPairSignerFromPrivateKeyBytes(sementes.cleantech);
const balancaCleantech = await createKeyPairSignerFromPrivateKeyBytes(sementes.balancaCleantech);

client.surfnet.deploy({ programId: MPL_CORE, soPath: join(raiz, 'tests', 'fixtures', 'mpl_core.so') });
for (const [programa, arquivo] of [
    [lote.ECOL_LOTE_PROGRAM_ADDRESS, 'ecol_lote.so'],
    [credito.ECOL_CREDITO_PROGRAM_ADDRESS, 'ecol_credito.so'],
    [rep.ECOL_REPARTICAO_PROGRAM_ADDRESS, 'ecol_reparticao.so'],
] as const) {
    client.surfnet.deploy({ programId: programa, soPath: join(raiz, 'target', 'deploy', arquivo) });
    await client.cheatcodes.setProgramAuthority(programa, upgrade.address).send();
}
client.surfnet.fundSol(cooperativa.address, 10_000_000_000);
client.surfnet.fundSol(operador.address, 10_000_000_000);
// A indústria paga a taxa do recebimento.
client.surfnet.fundSol(industria.address, 1_000_000_000);
// O intermediador paga a taxa da liberação do escrow.
client.surfnet.fundSol(intermediador.address, 1_000_000_000);
// Importador (titular e a carteira da loja) e Clean Tech pagam as próprias transações na interface.
for (const c of [importador, importadorLoja, cleantech]) client.surfnet.fundSol(c.address, 2_000_000_000);

const programData = async (programa: string) =>
    (await getProgramDerivedAddress({ programAddress: LOADER, seeds: [getAddressEncoder().encode(address(programa))] }))[0];
const ev = await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS);

await enviar(
    await lote.getAdminInitGlobalInstructionAsync({
        payer: client.payer,
        upgradeAuthority: upgrade,
        programData: await programData(lote.ECOL_LOTE_PROGRAM_ADDRESS),
        eventAuthority: ev,
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        admin: admin.address,
        params: {
            arbitro: arbitro.address,
            operador: operador.address,
            intermediador: intermediador.address,
            creditoAutoridade: await pCredito.autoridade(),
            prazoEntregaS: 7n * 24n * 3600n,
            toleranciaPesoBps: 200,
        },
    }),
    await credito.getAdminInitConfigInstructionAsync({
        payer: client.payer,
        upgradeAuthority: upgrade,
        programData: await programData(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        eventAuthority: await eventAuthority(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
        admin: admin.address,
        operador: operador.address,
        registrador: registrador.address,
        intermediador: intermediador.address,
    }),
    await rep.getAdminInitConfigInstructionAsync({
        payer: client.payer,
        upgradeAuthority: upgrade,
        programData: await programData(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        eventAuthority: await eventAuthority(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
        admin: admin.address,
        params: {
            operador: operador.address,
            zupy: zupy.address,
            intermediador: intermediador.address,
            bps: { coletor: 500, cooperativa: 1_000, transportador: 500, industriaMin: 6_500, industriaMax: 7_900 },
        },
    }),
);

for (const [carteira, papel, nome] of [
    [cooperativa.address, lote.Papel.Cooperativa, 'Cooperativa Demo'],
    ...coletores.map((c, i) => [c.address, lote.Papel.Coletor, `Coletor ${i + 1}`] as const),
    [transportador.address, lote.Papel.Transportador, 'Transportador Demo'],
    [industria.address, lote.Papel.Industria, 'Indústria Demo'],
    [importador.address, lote.Papel.Importador, 'Importadora de Vinhos Demo'],
    [cleantech.address, lote.Papel.CleanTech, 'Clean Tech Demo'],
] as const) {
    await enviar(
        await lote.getOperadorRegisterParticipanteInstructionAsync({
            payer: client.payer,
            operador,
            participante: await pLote.participante(carteira),
            vinculo: await pLote.carteira(carteira),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            carteira,
            papel,
            kycHash: h(`kyc:${carteira}`),
            nome,
        }),
    );
}
await enviar(
    await lote.getOperadorRegisterBalancaInstructionAsync({
        payer: client.payer,
        operador,
        donoPart: await pLote.participante(cooperativa.address),
        balanca: await pLote.balanca(balanca.address),
        eventAuthority: ev,
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        dispositivo: balanca.address,
        nome: 'Balança de teste',
    }),
);
for (const [codigo, nome] of [
    [1, 'Plástico'],
    [2, 'Papel'],
] as const) {
    await enviar(
        await lote.getOperadorCreateMaterialInstructionAsync({
            payer: client.payer,
            operador,
            material: await pLote.material(codigo),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            codigo,
            nome,
        }),
    );
}

// Lotes de demonstração, cada um parado numa etapa, para as telas terem o que mostrar:
// #1 vendido (Retiradas), #2 em leilão (Leilões), #3 em transporte (Recebimentos da indústria) e #4 recebido
// (liberação do escrow pelo intermediador).
const agora = async () => (await fetchSysvarClock(client.rpc)).unixTimestamp;
const balancaPda = await pLote.balanca(balanca.address);

// Balança de teste da indústria, já cadastrada; a semente vai no JSON para o navegador da indústria.
const balancaIndustria = await createKeyPairSignerFromPrivateKeyBytes(sementes.balancaIndustria);
await enviar(
    await lote.getOperadorRegisterBalancaInstructionAsync({
        payer: client.payer,
        operador,
        donoPart: await pLote.participante(industria.address),
        balanca: await pLote.balanca(balancaIndustria.address),
        eventAuthority: ev,
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        dispositivo: balancaIndustria.address,
        nome: 'Balança da indústria',
    }),
);

type Etapa = 'anunciado' | 'vendido' | 'emTransporte' | 'recebido';
async function loteDemo(id: bigint, material: number, pesoG: bigint, coletor: Address, ate: Etapa) {
    const entrega = await pLote.entrega(cooperativa.address, id);
    const lotePda = await pLote.lote(cooperativa.address, id);
    let ts = await agora();
    await enviar(
        await instrucaoPesagem(balanca, TipoPesagem.Entrega, entrega, pesoG, ts),
        await lote.getCooperativaRegisterEntregaInstructionAsync({
            payer: client.payer,
            cooperativa: cooperativa.address,
            cooperativaAssinante: cooperativa,
            balanca: balancaPda,
            materialCadastro: await pLote.material(material),
            entrega,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            entregaId: id,
            origem: lote.OrigemEntrega.Coletor,
            origemRef: await coletorRef(coletor),
            material,
            pesoG,
            tsPesagem: ts,
            variacao: 0,
            qtdGarrafas: 0,
        }),
    );
    await enviar(
        await lote.getCooperativaCreateLoteInstructionAsync({
            payer: client.payer,
            cooperativa: cooperativa.address,
            cooperativaAssinante: cooperativa,
            materialCadastro: await pLote.material(material),
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            loteId: id,
            material,
            evidenciasHash: h(`lote-demo:${id}`),
            variacao: 0,
        }),
    );
    await enviar(
        comContasGravaveis(
            await lote.getCooperativaAddEntregasInstructionAsync({ cooperativa: cooperativa.address, cooperativaAssinante: cooperativa, lote: lotePda, eventAuthority: ev, program: lote.ECOL_LOTE_PROGRAM_ADDRESS }),
            [entrega],
        ),
    );
    ts = await agora();
    await enviar(
        await instrucaoPesagem(balanca, TipoPesagem.Origem, lotePda, pesoG, ts),
        await lote.getCooperativaFecharLoteInstructionAsync({
            payer: client.payer,
            cooperativa: cooperativa.address,
            cooperativaAssinante: cooperativa,
            balanca: balancaPda,
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            pesoG,
            tsPesagem: ts,
            uri: `https://arweave.net/ecolchain/lote-demo-${id}.json`,
        }),
    );
    await enviar(
        await lote.getCooperativaListLoteInstructionAsync({
            cooperativa: cooperativa.address,
            cooperativaAssinante: cooperativa,
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            prazoLeilao: (await agora()) + 7n * 24n * 3600n,
            precoMinimoCentavos: 90_000n,
        }),
    );
    if (ate === 'anunciado') return;
    await enviar(
        await lote.getIndustriaAcceptVendaInstructionAsync({
            industria: industria.address,
            industriaAssinante: industria,
            cooperativaPart: await pLote.participante(cooperativa.address),
            operador,
            intermediador,
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            valorCentavos: 125_000n,
            escrowRefHash: h(`escrow-demo:${id}`),
            lancesRoot: h(`lances-demo:${id}`),
        }),
    );
    if (ate === 'vendido') return;
    await enviar(
        await lote.getTransportadorPickupLoteInstructionAsync({
            transportador: transportador.address,
            transportadorAssinante: transportador,
            cooperativa: cooperativa.address,
            cooperativaAssinante: cooperativa,
            lote: lotePda,
            payer: client.payer,
            asset: await pLote.asset(lotePda),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        }),
    );
    if (ate === 'emTransporte') return;
    ts = await agora();
    await enviar(
        await instrucaoPesagem(balancaIndustria, TipoPesagem.Recebimento, lotePda, pesoG, ts),
        await lote.getIndustriaConfirmRecebimentoInstructionAsync({
            industria: industria.address,
            industriaAssinante: industria,
            balanca: await pLote.balanca(balancaIndustria.address),
            lote: lotePda,
            payer: client.payer,
            asset: await pLote.asset(lotePda),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            pesoRecebidoG: pesoG,
            tsPesagem: ts,
        }),
    );
}
await loteDemo(1n, 1, 250_000n, coletores[0].address, 'vendido');
await loteDemo(2n, 2, 180_000n, coletores[1].address, 'anunciado');
await loteDemo(3n, 1, 120_000n, coletores[0].address, 'emTransporte');
await loteDemo(4n, 2, 300_000n, coletores[1].address, 'recebido');

// ---- ADR 0011: vidro com cores e garrafas, importador com duas carteiras e Clean Tech ----
const VIDRO = 3;
const CORES = ['Transparente', 'Verde', 'Marrom'] as const;
const [TRANSPARENTE, VERDE, MARROM] = [1, 2, 3];
await enviar(
    await lote.getOperadorCreateMaterialInstructionAsync({
        payer: client.payer,
        operador,
        material: await pLote.material(VIDRO),
        eventAuthority: ev,
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        codigo: VIDRO,
        nome: 'Vidro',
    }),
);
for (const [i, nome] of CORES.entries()) {
    await enviar(
        await lote.getOperadorCreateVariacaoInstructionAsync({
            payer: client.payer,
            operador,
            material: await pLote.material(VIDRO),
            variacao: await pLote.variacao(VIDRO, i + 1),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            nome,
        }),
    );
}
await enviar(
    await lote.getOperadorSetMaterialGarrafasInstructionAsync({
        operador,
        material: await pLote.material(VIDRO),
        eventAuthority: ev,
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        contaGarrafas: true,
    }),
);
// Carteiras do importador: o titular ganha nome e a loja é vinculada a ele.
for (const [endereco, nome] of [
    [importador.address, 'Diretoria'],
    [importadorLoja.address, 'Loja Centro'],
] as const) {
    await enviar(
        await lote.getParticipanteAddCarteiraInstructionAsync({
            payer: client.payer,
            autoridade: operador,
            participante: await pLote.participante(importador.address),
            enderecoPart: await pLote.participante(endereco),
            carteira: await pLote.carteira(endereco),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            endereco,
            nome,
        }),
    );
}
await enviar(
    await lote.getOperadorRegisterBalancaInstructionAsync({
        payer: client.payer,
        operador,
        donoPart: await pLote.participante(cleantech.address),
        balanca: await pLote.balanca(balancaCleantech.address),
        eventAuthority: ev,
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        dispositivo: balancaCleantech.address,
        nome: 'Balança da Clean Tech',
    }),
);

// Seis meses de garrafas distribuídas (datas das NF/DI no passado) e coletas em todas as etapas.
const DIA = 86_400n;
const hoje = await agora();
const imp = importador.address;
let distribId = 0n;
for (let mes = 5; mes >= 0; mes--) {
    for (const [cor, base] of [
        [TRANSPARENTE, 1_800],
        [VERDE, 4_200],
        [MARROM, 2_400],
    ] as const) {
        distribId += 1n;
        const garrafas = base + ((mes * 317 + cor * 131) % 900);
        await enviar(
            await lote.getImportadorCreateDistribuicaoInstructionAsync({
                payer: client.payer,
                importador: imp,
                importadorAssinante: importador,
                importadorPart: await pLote.participante(imp),
                materialCadastro: await pLote.material(VIDRO),
                variacaoCadastro: await pLote.variacao(VIDRO, cor),
                distribuicao: await pLote.distribuicao(imp, distribId),
                eventAuthority: ev,
                program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
                distribId,
                material: VIDRO,
                variacao: cor,
                qtdGarrafas: garrafas,
                pesoEstimadoG: BigInt(garrafas) * 500n,
                docHash: h(`DI 26/${String(distribId).padStart(6, '0')}`),
                emitidoEm: hoje - BigInt(mes) * 30n * DIA - 3n * DIA,
            }),
        );
    }
}

type EtapaColeta = 'coletada' | 'emEntregaCleanTech' | 'emEntregaCooperativa' | 'entregue' | 'reciclada';
/** Coleta `id` registrada pela loja (carteira vinculada) e levada até a etapa pedida. */
async function coletaDemo(id: bigint, cor: number, garrafas: number, diasAtras: number, ate: EtapaColeta) {
    const coleta = await pLote.coleta(imp, id);
    await enviar(
        await lote.getImportadorRegisterColetaInstructionAsync({
            payer: client.payer,
            importador: imp,
            importadorAssinante: importadorLoja,
            importadorCarteira: await pLote.carteira(importadorLoja.address),
            importadorPart: await pLote.participante(imp),
            materialCadastro: await pLote.material(VIDRO),
            variacaoCadastro: await pLote.variacao(VIDRO, cor),
            coleta,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            coletaId: id,
            material: VIDRO,
            variacao: cor,
            qtdGarrafas: garrafas,
            pesoEstimadoG: BigInt(garrafas) * 500n,
            localRef: h(`ponto-de-coleta:${id}:salt`),
            coletadoEm: hoje - BigInt(diasAtras) * DIA,
        }),
    );
    if (ate === 'coletada') return;
    const destino = ate === 'emEntregaCooperativa' ? cooperativa.address : cleantech.address;
    await enviar(
        await lote.getImportadorEntregarColetaInstructionAsync({
            importador: imp,
            importadorAssinante: importador,
            importadorPart: await pLote.participante(imp),
            coleta,
            destinoPart: await pLote.participante(destino),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        }),
    );
    if (ate === 'emEntregaCleanTech' || ate === 'emEntregaCooperativa') return;
    // A Clean Tech pesa e registra o lote de origem (garrafas recontadas: 2% de quebra).
    const pesoG = BigInt(garrafas) * 480n;
    const entrega = await pLote.entrega(cleantech.address, id);
    const ts = await agora();
    await enviar(
        await instrucaoPesagem(balancaCleantech, TipoPesagem.Entrega, entrega, pesoG, ts),
        await lote.getCooperativaRegisterEntregaInstructionAsync({
            payer: client.payer,
            cooperativa: cleantech.address,
            cooperativaAssinante: cleantech,
            balanca: await pLote.balanca(balancaCleantech.address),
            materialCadastro: await pLote.material(VIDRO),
            variacaoCadastro: await pLote.variacao(VIDRO, cor),
            coleta,
            entrega,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            entregaId: id,
            origem: lote.OrigemEntrega.Importador,
            origemRef: getAddressEncoder().encode(coleta) as Uint8Array,
            material: VIDRO,
            pesoG,
            tsPesagem: ts,
            variacao: cor,
            qtdGarrafas: Math.round(garrafas * 0.98),
        }),
    );
}
await coletaDemo(1n, VERDE, 3_100, 120, 'reciclada');
await coletaDemo(2n, VERDE, 2_650, 75, 'reciclada');
await coletaDemo(3n, MARROM, 1_900, 40, 'entregue');
await coletaDemo(4n, TRANSPARENTE, 1_200, 12, 'emEntregaCleanTech');
await coletaDemo(5n, MARROM, 900, 6, 'emEntregaCooperativa');
await coletaDemo(6n, VERDE, 1_500, 2, 'coletada');

// Lote de venda da Clean Tech com as coletas 1 e 2 (verde), levado até Reciclado; o importador anexa as NF.
{
    const ct = cleantech.address;
    const lotePda = await pLote.lote(ct, 1n);
    const entregas = [await pLote.entrega(ct, 1n), await pLote.entrega(ct, 2n)];
    const pesoG = (3_100n + 2_650n) * 480n;
    const balancaCt = await pLote.balanca(balancaCleantech.address);
    const asset = await pLote.asset(lotePda);
    await enviar(
        await lote.getCooperativaCreateLoteInstructionAsync({
            payer: client.payer,
            cooperativa: ct,
            cooperativaAssinante: cleantech,
            materialCadastro: await pLote.material(VIDRO),
            variacaoCadastro: await pLote.variacao(VIDRO, VERDE),
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            loteId: 1n,
            material: VIDRO,
            evidenciasHash: h('lote-demo-cleantech:1'),
            variacao: VERDE,
        }),
    );
    await enviar(
        comContasGravaveis(
            await lote.getCooperativaAddEntregasInstructionAsync({
                cooperativa: ct,
                cooperativaAssinante: cleantech,
                lote: lotePda,
                eventAuthority: ev,
                program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            }),
            entregas,
        ),
    );
    let ts = await agora();
    await enviar(
        await instrucaoPesagem(balancaCleantech, TipoPesagem.Origem, lotePda, pesoG, ts),
        await lote.getCooperativaFecharLoteInstructionAsync({
            payer: client.payer,
            cooperativa: ct,
            cooperativaAssinante: cleantech,
            balanca: balancaCt,
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            pesoG,
            tsPesagem: ts,
            uri: 'https://arweave.net/ecolchain/lote-demo-cleantech-1.json',
        }),
    );
    await enviar(
        await lote.getCooperativaListLoteInstructionAsync({
            cooperativa: ct,
            cooperativaAssinante: cleantech,
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            prazoLeilao: (await agora()) + 7n * 24n * 3600n,
            precoMinimoCentavos: 50_000n,
        }),
    );
    await enviar(
        await lote.getIndustriaAcceptVendaInstructionAsync({
            industria: industria.address,
            industriaAssinante: industria,
            cooperativaPart: await pLote.participante(ct),
            operador,
            intermediador,
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            valorCentavos: 86_000n,
            escrowRefHash: h('escrow-demo:cleantech-1'),
            lancesRoot: h('lances-demo:cleantech-1'),
        }),
    );
    await enviar(
        await lote.getTransportadorPickupLoteInstructionAsync({
            transportador: transportador.address,
            transportadorAssinante: transportador,
            cooperativa: ct,
            cooperativaAssinante: cleantech,
            lote: lotePda,
            payer: client.payer,
            asset,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        }),
    );
    ts = await agora();
    await enviar(
        await instrucaoPesagem(balancaIndustria, TipoPesagem.Recebimento, lotePda, pesoG, ts),
        await lote.getIndustriaConfirmRecebimentoInstructionAsync({
            industria: industria.address,
            industriaAssinante: industria,
            balanca: await pLote.balanca(balancaIndustria.address),
            lote: lotePda,
            payer: client.payer,
            asset,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            pesoRecebidoG: pesoG,
            tsPesagem: ts,
        }),
    );
    await enviar(
        await lote.getIntermediadorConfirmLiberacaoInstructionAsync({
            intermediador,
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            liberacaoRefHash: h('pix-liberacao:cleantech-1'),
        }),
    );
    for (const [id, entrega] of [
        [1n, entregas[0]],
        [2n, entregas[1]],
    ] as const) {
        await enviar(
            await lote.getImportadorInformarReciclagemInstructionAsync({
                importador: imp,
                importadorAssinante: importador,
                coleta: await pLote.coleta(imp, id),
                entrega,
                lote: lotePda,
                eventAuthority: ev,
                program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
                nfReciclagemHash: h(`NF reciclagem ${id}`),
            }),
        );
    }
}

const hex = (b: Uint8Array) => Buffer.from(b).toString('hex');
writeFileSync(
    saida,
    JSON.stringify(
        {
            rpcUrl: client.rpcUrl,
            wsUrl: client.wsUrl,
            cooperativa: { endereco: cooperativa.address, semente: hex(sementes.cooperativa) },
            balanca: { endereco: balanca.address, semente: hex(sementes.balanca) },
            coletores: coletores.map((c) => c.address),
            // Para entrar na interface como coletor (tela Meus lotes).
            coletor: { endereco: coletores[0].address, semente: hex(sementes.coletor1) },
            transportador: { endereco: transportador.address, semente: hex(sementes.transportador) },
            industria: { endereco: industria.address, semente: hex(sementes.industria) },
            balancaIndustria: { endereco: balancaIndustria.address, semente: hex(sementes.balancaIndustria) },
            operador: { endereco: operador.address, semente: hex(sementes.operador) },
            intermediador: { endereco: intermediador.address, semente: hex(sementes.intermediador) },
            importador: { endereco: importador.address, semente: hex(sementes.importador) },
            // Carteira vinculada ao importador ("Loja Centro"): assina pelo importador.
            importadorLoja: { endereco: importadorLoja.address, semente: hex(sementes.importadorLoja) },
            cleantech: { endereco: cleantech.address, semente: hex(sementes.cleantech) },
            balancaCleantech: { endereco: balancaCleantech.address, semente: hex(sementes.balancaCleantech) },
        },
        null,
        2,
    ),
);
console.log(`Rede local pronta: ${client.rpcUrl} (ws ${client.wsUrl})`);
console.log(`Chaves de teste em ${saida}. Ctrl+C encerra.`);
process.on('SIGINT', () => {
    client.surfnet.stop();
    process.exit(0);
});
process.on('SIGTERM', () => {
    client.surfnet.stop();
    process.exit(0);
});
// Mantém o processo (e o surfnet embarcado) vivo até o sinal de encerramento.
setInterval(() => {}, 60_000);
