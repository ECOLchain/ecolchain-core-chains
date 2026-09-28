// Rede local de demonstração (Surfpool embarcado): os três programas + Metaplex Core, configs,
// uma cooperativa, dois coletores, materiais e a balança de teste da cooperativa já cadastrados.
// Fica rodando até Ctrl+C. Útil para testar a interface web sem depender da devnet.
//
// Uso: npx tsx scripts/rede-local.ts [arquivo-de-saída.json]
//   Grava no arquivo: rpcUrl, wsUrl, e as chaves de teste (cooperativa, balança e coletores).
//   Na interface: VITE_RPC_LOCALNET=<rpcUrl> VITE_WS_LOCALNET=<wsUrl> npm run dev, rede "Localnet".
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
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

const raiz = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const MPL_CORE = address('CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d');
const LOADER = address('BPFLoaderUpgradeab1e11111111111111111111111');
const h = (s: string) => new Uint8Array(createHash('sha256').update(s).digest());
const saida = process.argv[2] ?? join(raiz, '.surfpool-demo.json');

const client = await createClient().use(surfpool({ transactionConfig: { version: 0 } }));
const enviar = (...ixs: Instruction[]) => client.sendTransaction(ixs);
const semente = () => crypto.getRandomValues(new Uint8Array(32));

const [upgrade, admin, arbitro, operador, intermediador, registrador, zupy] = await Promise.all(
    Array.from({ length: 7 }, () => generateKeyPairSigner()),
);
const sementes = { cooperativa: semente(), balanca: semente(), coletor1: semente(), coletor2: semente() };
const cooperativa = await createKeyPairSignerFromPrivateKeyBytes(sementes.cooperativa);
const balanca = await createKeyPairSignerFromPrivateKeyBytes(sementes.balanca);
const coletores = await Promise.all([sementes.coletor1, sementes.coletor2].map((s) => createKeyPairSignerFromPrivateKeyBytes(s)));

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

for (const [carteira, papel] of [
    [cooperativa.address, lote.Papel.Cooperativa],
    ...coletores.map((c) => [c.address, lote.Papel.Coletor] as const),
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
