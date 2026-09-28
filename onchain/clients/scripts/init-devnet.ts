// Inicializa as configs dos três programas na devnet, logo após o deploy e ANTES de transferir a
// upgrade authority ao Squads (a inicialização exige a assinatura da upgrade authority).
// admin = vault 0 do Squads; árbitro = vault 1. Idempotente: pula configs que já existem.
// Uso: RPC_DEVNET=... CARTEIRA=~/.config/solana/id.json npx tsx scripts/init-devnet.ts
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
    type Address,
    address,
    createClient,
    createKeyPairSignerFromBytes,
    fetchEncodedAccount,
    getAddressEncoder,
    getProgramDerivedAddress,
    type Instruction,
} from '@solana/kit';
import { solanaRpc } from '@solana/kit-plugin-rpc';
import { signerFromFile } from '@solana/kit-plugin-signer';
import * as credito from '../src/generated/ecol_credito';
import * as lote from '../src/generated/ecol_lote';
import * as rep from '../src/generated/ecol_reparticao';
import { eventAuthority, credito as pCredito, lote as pLote, reparticao as pRep } from '../src/pdas';

const DIR = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '.devnet');
const RPC = process.env.RPC_DEVNET ?? 'https://api.devnet.solana.com';
const WS = RPC.replace(/^http/, 'ws');
const CARTEIRA = process.env.CARTEIRA ?? join(homedir(), '.config', 'solana', 'id.json');
const BPF_LOADER_UPGRADEABLE = address('BPFLoaderUpgradeab1e11111111111111111111111');

const chave = (nome: string) =>
    createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(readFileSync(join(DIR, `${nome}.json`), 'utf8'))));
const squads = JSON.parse(readFileSync(join(DIR, 'squads.json'), 'utf8')) as { vaultAdmin: Address; vaultArbitro: Address };

// A carteira local é quem fez o deploy: paga e assina como upgrade authority.
const client = await createClient()
    .use(signerFromFile(CARTEIRA))
    .use(solanaRpc({ rpcUrl: RPC, rpcSubscriptionsUrl: WS, transactionConfig: { version: 0 } }));
const [operador, intermediador, registrador, zupy] = await Promise.all(
    ['operador', 'intermediador', 'registrador', 'zupy'].map(chave),
);

async function programData(programa: Address) {
    const [pd] = await getProgramDerivedAddress({ programAddress: BPF_LOADER_UPGRADEABLE, seeds: [getAddressEncoder().encode(programa)] });
    return pd;
}

async function seNaoExiste(nome: string, conta: Address, ix: () => Promise<Instruction>) {
    if ((await fetchEncodedAccount(client.rpc, conta)).exists) {
        console.log(`${nome}: já inicializado (${conta})`);
        return;
    }
    await client.sendTransaction([await ix()]);
    console.log(`${nome}: inicializado (${conta})`);
}

await seNaoExiste('ecol_lote', await pLote.config(), async () =>
    lote.getAdminInitGlobalInstructionAsync({
        payer: client.payer,
        upgradeAuthority: client.payer,
        programData: await programData(lote.ECOL_LOTE_PROGRAM_ADDRESS),
        eventAuthority: await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS),
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        admin: squads.vaultAdmin,
        params: {
            arbitro: squads.vaultArbitro,
            operador: operador.address,
            intermediador: intermediador.address,
            creditoAutoridade: await pCredito.autoridade(),
            prazoEntregaS: 7n * 24n * 3600n,
            toleranciaPesoBps: 200,
        },
    }),
);
await seNaoExiste('ecol_credito', await pCredito.config(), async () =>
    credito.getAdminInitConfigInstructionAsync({
        payer: client.payer,
        upgradeAuthority: client.payer,
        programData: await programData(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        eventAuthority: await eventAuthority(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
        admin: squads.vaultAdmin,
        operador: operador.address,
        registrador: registrador.address,
        intermediador: intermediador.address,
    }),
);
await seNaoExiste('ecol_reparticao', await pRep.config(), async () =>
    rep.getAdminInitConfigInstructionAsync({
        payer: client.payer,
        upgradeAuthority: client.payer,
        programData: await programData(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        eventAuthority: await eventAuthority(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
        admin: squads.vaultAdmin,
        params: {
            operador: operador.address,
            zupy: zupy.address,
            intermediador: intermediador.address,
            bps: { coletor: 500, cooperativa: 1_000, transportador: 500, industriaMin: 6_500, industriaMax: 7_900 },
        },
    }),
);
