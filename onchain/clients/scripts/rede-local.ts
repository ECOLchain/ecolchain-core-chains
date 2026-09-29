// Rede local de demonstração (Surfpool embarcado): os três programas + Metaplex Core, configs,
// uma cooperativa, dois coletores, um transportador, uma indústria, materiais e a balança de teste da cooperativa já
// cadastrados (com a balança da indústria), e três lotes de venda: #1 vendido (esperando a retirada), #2 em leilão e
// #3 em transporte (esperando o recebimento). O JSON de saída traz as sementes de todos os papéis usados na interface.
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
};
const operador = await createKeyPairSignerFromPrivateKeyBytes(sementes.operador);
const intermediador = await createKeyPairSignerFromPrivateKeyBytes(sementes.intermediador);
const cooperativa = await createKeyPairSignerFromPrivateKeyBytes(sementes.cooperativa);
const balanca = await createKeyPairSignerFromPrivateKeyBytes(sementes.balanca);
const coletores = await Promise.all([sementes.coletor1, sementes.coletor2].map((s) => createKeyPairSignerFromPrivateKeyBytes(s)));
const transportador = await createKeyPairSignerFromPrivateKeyBytes(sementes.transportador);
const industria = await createKeyPairSignerFromPrivateKeyBytes(sementes.industria);

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
] as const) {
    await enviar(
        await lote.getOperadorRegisterParticipanteInstructionAsync({
            payer: client.payer,
            operador,
            participante: await pLote.participante(carteira),
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
// #1 vendido (Retiradas), #2 em leilão (Leilões) e #3 em transporte (Recebimentos da indústria).
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

type Etapa = 'anunciado' | 'vendido' | 'emTransporte';
async function loteDemo(id: bigint, material: number, pesoG: bigint, coletor: Address, ate: Etapa) {
    const entrega = await pLote.entrega(cooperativa.address, id);
    const lotePda = await pLote.lote(cooperativa.address, id);
    let ts = await agora();
    await enviar(
        await instrucaoPesagem(balanca, TipoPesagem.Entrega, entrega, pesoG, ts),
        await lote.getCooperativaRegisterEntregaInstructionAsync({
            payer: client.payer,
            cooperativa,
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
        }),
    );
    await enviar(
        await lote.getCooperativaCreateLoteInstructionAsync({
            payer: client.payer,
            cooperativa,
            materialCadastro: await pLote.material(material),
            lote: lotePda,
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
            loteId: id,
            material,
            evidenciasHash: h(`lote-demo:${id}`),
        }),
    );
    await enviar(
        comContasGravaveis(
            await lote.getCooperativaAddEntregasInstructionAsync({ cooperativa, lote: lotePda, eventAuthority: ev, program: lote.ECOL_LOTE_PROGRAM_ADDRESS }),
            [entrega],
        ),
    );
    ts = await agora();
    await enviar(
        await instrucaoPesagem(balanca, TipoPesagem.Origem, lotePda, pesoG, ts),
        await lote.getCooperativaFecharLoteInstructionAsync({
            payer: client.payer,
            cooperativa,
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
            cooperativa,
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
            industria,
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
            transportador,
            cooperativa,
            lote: lotePda,
            payer: client.payer,
            asset: await pLote.asset(lotePda),
            eventAuthority: ev,
            program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        }),
    );
}
await loteDemo(1n, 1, 250_000n, coletores[0].address, 'vendido');
await loteDemo(2n, 2, 180_000n, coletores[1].address, 'anunciado');
await loteDemo(3n, 1, 120_000n, coletores[0].address, 'emTransporte');

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
