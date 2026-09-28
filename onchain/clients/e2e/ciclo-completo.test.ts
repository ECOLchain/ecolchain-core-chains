// E2E no Surfpool (surfnet embarcado): os três programas + Metaplex Core, dirigidos só pelos
// clientes gerados pelo Codama, como o backend da EcolChain fará. Pré-requisito: `anchor build`.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { after, before, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import {
    type Address,
    address,
    createClient,
    generateKeyPairSigner,
    getAddressEncoder,
    getProgramDerivedAddress,
    type Instruction,
    type KeyPairSigner,
} from '@solana/kit';
import { surfpool } from '@solana/surfpool/kit';
import { fetchSysvarClock } from '@solana/sysvars';
import * as credito from '../src/generated/ecol_credito';
import * as lote from '../src/generated/ecol_lote';
import * as rep from '../src/generated/ecol_reparticao';
import { calcularTotais, montarArvore, montarDireitos } from '../src/arvore';
import { eventAuthority, pdas } from './apoio';
import { comContasGravaveis, instrucaoPesagem, TipoPesagem } from '../src/pesagem';

const deploy = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'target', 'deploy');
const fixtures = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'tests', 'fixtures');
const MPL_CORE = address('CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d');
const BPF_LOADER_UPGRADEABLE = address('BPFLoaderUpgradeab1e11111111111111111111111');
const h = (s: string) => new Uint8Array(createHash('sha256').update(s).digest());

// v0: o planner do @solana/kit-plugin-rpc 0.18 (versão exigida pelo @solana/surfpool 1.6) ainda não
// monta v1 ("Version 1 transactions are not yet supported by `rpcTransactionPlanner`"). Migrar para
// `version: 1` quando o Surfpool aceitar o plugin-rpc 0.19+.
const client = await createClient().use(surfpool({ transactionConfig: { version: 0 } }));
after(() => client.surfnet.stop());

type Papeis = Record<
    | 'upgrade' | 'admin' | 'arbitro' | 'operador' | 'intermediador' | 'registrador' | 'zupy'
    | 'cooperativa' | 'industria' | 'transportador' | 'coletor' | 'comprador' | 'balancaCoop' | 'balancaInd',
    KeyPairSigner
>;
let k: Papeis;
const ev: Record<'lote' | 'credito' | 'rep', Address> = {} as never;

async function enviar(...ixs: Instruction[]) {
    return client.sendTransaction(ixs);
}

async function agora(): Promise<bigint> {
    return (await fetchSysvarClock(client.rpc)).unixTimestamp;
}

async function programData(programa: Address) {
    const [pd] = await getProgramDerivedAddress({
        programAddress: BPF_LOADER_UPGRADEABLE,
        seeds: [getAddressEncoder().encode(programa)],
    });
    return pd;
}

before(async () => {
    const nomes = [
        'upgrade', 'admin', 'arbitro', 'operador', 'intermediador', 'registrador', 'zupy',
        'cooperativa', 'industria', 'transportador', 'coletor', 'comprador', 'balancaCoop', 'balancaInd',
    ] as const;
    k = Object.fromEntries(await Promise.all(nomes.map(async (n) => [n, await generateKeyPairSigner()]))) as Papeis;

    client.surfnet.deploy({ programId: MPL_CORE, soPath: join(fixtures, 'mpl_core.so') });
    for (const [programa, arquivo] of [
        [lote.ECOL_LOTE_PROGRAM_ADDRESS, 'ecol_lote.so'],
        [credito.ECOL_CREDITO_PROGRAM_ADDRESS, 'ecol_credito.so'],
        [rep.ECOL_REPARTICAO_PROGRAM_ADDRESS, 'ecol_reparticao.so'],
    ] as const) {
        client.surfnet.deploy({ programId: programa, soPath: join(deploy, arquivo) });
        await client.cheatcodes.setProgramAuthority(programa, k.upgrade.address).send();
    }
    ev.lote = await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS);
    ev.credito = await eventAuthority(credito.ECOL_CREDITO_PROGRAM_ADDRESS);
    ev.rep = await eventAuthority(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS);

    // Configurações dos três programas (a upgrade authority assina a inicialização).
    await enviar(
        await lote.getAdminInitGlobalInstructionAsync({
            payer: client.payer,
            upgradeAuthority: k.upgrade,
            programData: await programData(lote.ECOL_LOTE_PROGRAM_ADDRESS),
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            admin: k.admin.address,
            params: {
                arbitro: k.arbitro.address,
                operador: k.operador.address,
                intermediador: k.intermediador.address,
                creditoAutoridade: await pdas.credito.autoridade(),
                prazoEntregaS: 7n * 24n * 3600n,
                toleranciaPesoBps: 200,
            },
        }),
        await credito.getAdminInitConfigInstructionAsync({
            payer: client.payer,
            upgradeAuthority: k.upgrade,
            programData: await programData(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
            eventAuthority: ev.credito,
            program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
            admin: k.admin.address,
            operador: k.operador.address,
            registrador: k.registrador.address,
            intermediador: k.intermediador.address,
        }),
        await rep.getAdminInitConfigInstructionAsync({
            payer: client.payer,
            upgradeAuthority: k.upgrade,
            programData: await programData(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
            eventAuthority: ev.rep,
            program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
            admin: k.admin.address,
            params: {
                operador: k.operador.address,
                zupy: k.zupy.address,
                intermediador: k.intermediador.address,
                bps: { coletor: 500, cooperativa: 1_000, transportador: 500, industriaMin: 6_500, industriaMax: 7_900 },
            },
        }),
    );

    // Participantes e balanças.
    const cadastros: [KeyPairSigner, lote.Papel][] = [
        [k.cooperativa, lote.Papel.Cooperativa],
        [k.industria, lote.Papel.Industria],
        [k.transportador, lote.Papel.Transportador],
        [k.coletor, lote.Papel.Coletor],
    ];
    for (const [carteira, papel] of cadastros) {
        await enviar(
            await lote.getOperadorRegisterParticipanteInstructionAsync({
                payer: client.payer,
                operador: k.operador,
                participante: await pdas.lote.participante(carteira.address),
                eventAuthority: ev.lote,
                program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
                carteira: carteira.address,
                papel,
                kycHash: h(`kyc:${carteira.address}`),
                nome: lote.Papel[papel],
            }),
        );
    }
    for (const [dono, balanca] of [[k.cooperativa, k.balancaCoop], [k.industria, k.balancaInd]] as const) {
        await enviar(
            await lote.getOperadorRegisterBalancaInstructionAsync({
                payer: client.payer,
                operador: k.operador,
                donoPart: await pdas.lote.participante(dono.address),
                balanca: await pdas.lote.balanca(balanca.address),
                eventAuthority: ev.lote,
                program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
                dispositivo: balanca.address,
            }),
        );
    }
    await enviar(
        await lote.getOperadorCreateMaterialInstructionAsync({
            payer: client.payer,
            operador: k.operador,
            material: await pdas.lote.material(PLASTICO),
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            codigo: PLASTICO,
            nome: 'Plástico',
        }),
    );
});

/** Código do material cadastrado no início do teste. */
const PLASTICO = 1;

/** Registra a entrega `entregaId` do coletor (pesagem assinada pela balança) e a vincula ao lote. */
async function registrarEVincular(lotePda: Address, entregaId: bigint, pesoG: bigint) {
    const entregaPda = await pdas.lote.entrega(k.cooperativa.address, entregaId);
    const ts = await agora();
    await enviar(
        await instrucaoPesagem(k.balancaCoop, TipoPesagem.Entrega, entregaPda, pesoG, ts),
        await lote.getCooperativaRegisterEntregaInstructionAsync({
            payer: client.payer,
            cooperativa: k.cooperativa,
            balanca: await pdas.lote.balanca(k.balancaCoop.address),
            materialCadastro: await pdas.lote.material(PLASTICO),
            entrega: entregaPda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            entregaId,
            coletorRef: h(`coletor:${k.coletor.address}:salt`),
            material: PLASTICO,
            pesoG,
            tsPesagem: ts,
        }),
    );
    await enviar(
        comContasGravaveis(
            await lote.getCooperativaAddEntregasInstructionAsync({
                cooperativa: k.cooperativa,
                lote: lotePda,
                eventAuthority: ev.lote,
                program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            }),
            [entregaPda],
        ),
    );
}

/** Leva o lote `loteId` de `Criado` até `Reciclado`, com pesagens atestadas na origem e no recebimento. */
async function loteAteReciclado(loteId: bigint, pesoG: bigint): Promise<Address> {
    const lotePda = await pdas.lote.lote(k.cooperativa.address, loteId);
    const asset = await pdas.lote.asset(lotePda);
    let ts = await agora();
    await enviar(
        await instrucaoPesagem(k.balancaCoop, TipoPesagem.Origem, lotePda, pesoG, ts),
        await lote.getCooperativaCreateLoteInstructionAsync({
            payer: client.payer,
            cooperativa: k.cooperativa,
            balanca: await pdas.lote.balanca(k.balancaCoop.address),
            materialCadastro: await pdas.lote.material(PLASTICO),
            lote: lotePda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            loteId,
            material: PLASTICO,
            pesoG,
            tsPesagem: ts,
            evidenciasHash: h(`fotos:${loteId}`),
            uri: `https://arweave.net/ecolchain/lote-${loteId}.json`,
        }),
    );
    // Montagem: a entrega do coletor, pesada e assinada pela balança, vinculada ao lote.
    await registrarEVincular(lotePda, loteId, pesoG);
    await enviar(
        await lote.getCooperativaListLoteInstructionAsync({
            cooperativa: k.cooperativa,
            lote: lotePda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            prazoLeilao: (await agora()) + 3600n,
            precoMinimoCentavos: 2_500n,
        }),
    );
    await enviar(
        await lote.getIndustriaAcceptVendaInstructionAsync({
            industria: k.industria,
            cooperativaPart: await pdas.lote.participante(k.cooperativa.address),
            operador: k.operador,
            intermediador: k.intermediador,
            lote: lotePda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            valorCentavos: 5_000n,
            escrowRefHash: h(`escrow:${loteId}`),
            lancesRoot: h(`lances:${loteId}`),
        }),
    );
    await enviar(
        await lote.getTransportadorPickupLoteInstructionAsync({
            transportador: k.transportador,
            cooperativa: k.cooperativa,
            lote: lotePda,
            payer: client.payer,
            asset,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        }),
    );
    ts = await agora();
    await enviar(
        await instrucaoPesagem(k.balancaInd, TipoPesagem.Recebimento, lotePda, pesoG, ts),
        await lote.getIndustriaConfirmRecebimentoInstructionAsync({
            industria: k.industria,
            balanca: await pdas.lote.balanca(k.balancaInd.address),
            lote: lotePda,
            payer: client.payer,
            asset,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            pesoRecebidoG: pesoG,
            tsPesagem: ts,
        }),
    );
    await enviar(
        await lote.getIntermediadorConfirmLiberacaoInstructionAsync({
            intermediador: k.intermediador,
            lote: lotePda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            liberacaoRefHash: h(`pix-liberacao:${loteId}`),
        }),
    );
    return lotePda;
}

test('ciclo completo: lote → crédito vendido → direitos → resgates', async () => {
    const lotes = [await loteAteReciclado(1n, 1_000_000n), await loteAteReciclado(2n, 500_000n)];
    for (const l of lotes) {
        const conta = await lote.fetchLote(client.rpc, l);
        assert.equal(conta.data.estado.__kind, 'Reciclado');
        assert.equal(conta.data.recebimentoAtestado, true);
    }

    // Crédito: formação, agregação (CPI no ecol_lote), validação, emissão, venda.
    const creditoPda = await pdas.credito.credito(1n);
    await enviar(
        await credito.getOperadorCreateCreditoInstructionAsync({
            payer: client.payer,
            operador: k.operador,
            credito: creditoPda,
            eventAuthority: ev.credito,
            program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
            creditoId: 1n,
            padrao: credito.PadraoRegistro.Verra,
            material: PLASTICO,
            vintage: 2026,
            metodologiaHash: h('metodologia-plastico-v1'),
        }),
    );
    await enviar(
        comContasGravaveis(
            await credito.getOperadorAddLotesInstructionAsync({
                operador: k.operador,
                credito: creditoPda,
                loteConfig: await pdas.lote.config(),
                loteEventAuthority: ev.lote,
                eventAuthority: ev.credito,
                program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
            }),
            lotes,
        ),
    );
    await enviar(
        await credito.getOperadorSubmitCreditoInstructionAsync({
            operador: k.operador,
            credito: creditoPda,
            eventAuthority: ev.credito,
            program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
        }),
    );
    const serial = 'VCS-4242-2026-BR-000001';
    const serialHash = h(serial);
    await enviar(
        await credito.getRegistradorRecordEmissaoInstructionAsync({
            payer: client.payer,
            registrador: k.registrador,
            credito: creditoPda,
            serialRegistro: await pdas.credito.serial(credito.PadraoRegistro.Verra, serialHash),
            eventAuthority: ev.credito,
            program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
            serial,
            serialHash,
            tco2eMilli: 1_500n,
        }),
    );
    await enviar(
        await credito.getOperadorRecordVendaInstructionAsync({
            operador: k.operador,
            comprador: k.comprador,
            intermediador: k.intermediador,
            credito: creditoPda,
            eventAuthority: ev.credito,
            program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
            valorCentavos: 7_500n,
            pagamentoRefHash: h('pix-venda-credito-1'),
        }),
    );
    const cr = await credito.fetchCredito(client.rpc, creditoPda);
    assert.equal(cr.data.estado, credito.EstadoCredito.Vendido);
    assert.equal(cr.data.qtdLotes, 2);
    assert.equal(cr.data.pesoTotalG, 1_500_000n);

    // Repartição: árvore gerada pelo `arvore.ts` a partir dos lotes do crédito.
    const bpsIndustria = 7_000;
    const { totais, ecolchain } = calcularTotais(7_500n, { coletor: 500, cooperativa: 1_000, transportador: 500 }, bpsIndustria);
    const direitos = montarDireitos(
        [
            { cooperativa: k.cooperativa.address, transportador: k.transportador.address, industria: k.industria.address, pesoG: 1_000_000n, entregas: [{ coletor: k.coletor.address, pesoG: 1_000_000n }] },
            { cooperativa: k.cooperativa.address, transportador: k.transportador.address, industria: k.industria.address, pesoG: 500_000n, entregas: [{ coletor: k.coletor.address, pesoG: 500_000n }] },
        ],
        totais,
    );
    const arvore = montarArvore(direitos);
    const distPda = await pdas.reparticao.distribuicao(creditoPda);
    await enviar(
        await rep.getOperadorCreateDistribuicaoInstructionAsync({
            payer: client.payer,
            operador: k.operador,
            credito: creditoPda,
            distribuicao: distPda,
            eventAuthority: ev.rep,
            program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
            bpsIndustria,
            merkleRoot: arvore.raiz,
            numFolhas: direitos.length,
        }),
    );
    let dist = await rep.fetchDistribuicao(client.rpc, distPda);
    assert.deepEqual(dist.data.totalPorPapel, totais);
    assert.equal(dist.data.ecolchainCentavos, ecolchain);

    await enviar(
        await rep.getIntermediadorRecordRepasseInstructionAsync({
            intermediador: k.intermediador,
            distribuicao: distPda,
            eventAuthority: ev.rep,
            program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
            zupyRefHash: h('pix-zupy-1'),
            ecolchainRefHash: h('pix-ecolchain-1'),
        }),
    );
    const assinante: Record<string, KeyPairSigner> = {
        [k.coletor.address]: k.coletor,
        [k.cooperativa.address]: k.cooperativa,
        [k.transportador.address]: k.transportador,
        [k.industria.address]: k.industria,
    };
    for (const d of direitos) {
        await enviar(
            await rep.getZupyRecordResgateInstructionAsync({
                zupy: k.zupy,
                beneficiario: assinante[d.beneficiario],
                distribuicao: distPda,
                eventAuthority: ev.rep,
                program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
                indice: d.indice,
                papel: d.papel as unknown as rep.Papel,
                valorCentavos: d.valorCentavos,
                prova: arvore.prova(d.indice),
                reciboHash: h(`cupom:${d.indice}`),
            }),
        );
    }
    dist = await rep.fetchDistribuicao(client.rpc, distPda);
    assert.deepEqual(dist.data.resgatadoPorPapel, totais);
    assert.equal(dist.data.qtdResgates, direitos.length);
});

test('prazo de entrega vencido: time travel no surfnet + crank público', async () => {
    const lotePda = await pdas.lote.lote(k.cooperativa.address, 99n);
    const ts = await agora();
    await enviar(
        await instrucaoPesagem(k.balancaCoop, TipoPesagem.Origem, lotePda, 1_000n, ts),
        await lote.getCooperativaCreateLoteInstructionAsync({
            payer: client.payer,
            cooperativa: k.cooperativa,
            balanca: await pdas.lote.balanca(k.balancaCoop.address),
            materialCadastro: await pdas.lote.material(PLASTICO),
            lote: lotePda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            loteId: 99n,
            material: PLASTICO,
            pesoG: 1_000n,
            tsPesagem: ts,
            evidenciasHash: h('e'),
            uri: 'https://arweave.net/x.json',
        }),
    );
    await registrarEVincular(lotePda, 99n, 1_000n);
    await enviar(
        await lote.getCooperativaListLoteInstructionAsync({
            cooperativa: k.cooperativa,
            lote: lotePda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            prazoLeilao: ts + 3600n,
            precoMinimoCentavos: 1n,
        }),
    );
    await enviar(
        await lote.getIndustriaAcceptVendaInstructionAsync({
            industria: k.industria,
            cooperativaPart: await pdas.lote.participante(k.cooperativa.address),
            operador: k.operador,
            intermediador: k.intermediador,
            lote: lotePda,
            eventAuthority: ev.lote,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            valorCentavos: 1n,
            escrowRefHash: h('esc'),
            lancesRoot: h('lan'),
        }),
    );
    const prazo = (await lote.fetchLote(client.rpc, lotePda)).data.prazoEntrega;
    const cranker = await generateKeyPairSigner();
    const crank = async () =>
        enviar(
            await lote.getPublicCrankPrazoExpiradoInstructionAsync({
                cranker,
                lote: lotePda,
                eventAuthority: ev.lote,
                program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            }),
        );
    await assert.rejects(crank(), 'antes do prazo o crank falha');

    // `absoluteTimestamp` é em milissegundos.
    await client.cheatcodes.timeTravel({ absoluteTimestamp: (prazo + 60n) * 1000n }).send();
    assert.ok((await agora()) > prazo, 'relógio do surfnet avançou além do prazo');
    await crank();
    const l = await lote.fetchLote(client.rpc, lotePda);
    assert.equal(l.data.estado.__kind, 'EmDisputa');
});
