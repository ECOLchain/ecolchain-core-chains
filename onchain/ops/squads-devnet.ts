// Cria o Squads multisig v4 da devnet: 5 chaves de teste da EcolChain, quórum 3 (ADR 0009).
// Vault 0 = upgrade authority dos três programas e `admin` das configs; vault 1 = árbitro de disputas.
// Idempotente: se .devnet/squads.json existir, só mostra os endereços.
// SDK legado (web3.js v1) isolado neste pacote de operação.
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import * as multisig from '@sqds/multisig';
import { Connection, type PublicKey } from '@solana/web3.js';
import { carregarOuGerar, carteiraLocal, DIR, lerJson, MEMBROS, RPC_DEVNET, salvarJson } from './comum';

type Squads = { multisig: string; vaultAdmin: string; vaultArbitro: string; quorum: number; membros: string[] };

if (existsSync(join(DIR, 'squads.json'))) {
    console.log(lerJson<Squads>('squads.json'));
    process.exit(0);
}

const connection = new Connection(RPC_DEVNET, 'confirmed');
const criador = carteiraLocal();
const createKey = carregarOuGerar('squads-create-key');
const membros = MEMBROS.map(carregarOuGerar);
const [multisigPda] = multisig.getMultisigPda({ createKey: createKey.publicKey });
const vault = (index: number): PublicKey => multisig.getVaultPda({ multisigPda, index })[0];

const [programConfigPda] = multisig.getProgramConfigPda({});
const { treasury } = await multisig.accounts.ProgramConfig.fromAccountAddress(connection, programConfigPda);

const assinatura = await multisig.rpc.multisigCreateV2({
    connection,
    treasury,
    createKey,
    creator: criador,
    multisigPda,
    // Sem config authority: mudar membros/quórum também exige proposta aprovada por 3 de 5.
    configAuthority: null,
    threshold: 3,
    members: membros.map((m) => ({ key: m.publicKey, permissions: multisig.types.Permissions.all() })),
    // Timelock 0 na devnet; 24 h na mainnet (ADR 0009).
    timeLock: 0,
    rentCollector: null,
    sendOptions: { skipPreflight: false },
});
await connection.confirmTransaction(assinatura, 'confirmed');

const dados: Squads = {
    multisig: multisigPda.toBase58(),
    vaultAdmin: vault(0).toBase58(),
    vaultArbitro: vault(1).toBase58(),
    quorum: 3,
    membros: membros.map((m) => m.publicKey.toBase58()),
};
salvarJson('squads.json', dados);
console.log('Squads criado:', assinatura, dados);
