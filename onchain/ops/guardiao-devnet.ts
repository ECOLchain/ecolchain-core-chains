// Cria o Squads do guardião na devnet: os mesmos 5 membros da EcolChain, quórum 1, timelock 0.
// O vault 0 dele é o `guardiao` dos três programas: qualquer membro pausa sozinho e na hora (ADR 0009).
// Idempotente: se .devnet/guardiao.json existir, só mostra os endereços.
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import * as multisig from '@sqds/multisig';
import { Connection } from '@solana/web3.js';
import { carregarOuGerar, carteiraLocal, DIR, lerJson, MEMBROS, RPC_DEVNET, salvarJson } from './comum';

export type Guardiao = { multisig: string; vault: string; quorum: number; membros: string[] };

if (existsSync(join(DIR, 'guardiao.json'))) {
    console.log(lerJson<Guardiao>('guardiao.json'));
    process.exit(0);
}

const connection = new Connection(RPC_DEVNET, 'confirmed');
const criador = carteiraLocal();
const createKey = carregarOuGerar('guardiao-create-key');
const membros = MEMBROS.map(carregarOuGerar);
const [multisigPda] = multisig.getMultisigPda({ createKey: createKey.publicKey });
const [vault] = multisig.getVaultPda({ multisigPda, index: 0 });
const [programConfigPda] = multisig.getProgramConfigPda({});
const { treasury } = await multisig.accounts.ProgramConfig.fromAccountAddress(connection, programConfigPda);

const assinatura = await multisig.rpc.multisigCreateV2({
    connection,
    treasury,
    createKey,
    creator: criador,
    multisigPda,
    // Sem config authority: mudar membros ou quórum exige proposta; com quórum 1, qualquer membro.
    // Por isso a lista de membros deste Squads deve acompanhar a do Squads principal.
    configAuthority: null,
    threshold: 1,
    members: membros.map((m) => ({ key: m.publicKey, permissions: multisig.types.Permissions.all() })),
    timeLock: 0,
    rentCollector: null,
    sendOptions: { skipPreflight: false },
});
await connection.confirmTransaction(assinatura, 'confirmed');

const dados: Guardiao = {
    multisig: multisigPda.toBase58(),
    vault: vault.toBase58(),
    quorum: 1,
    membros: membros.map((m) => m.publicKey.toBase58()),
};
salvarJson('guardiao.json', dados);
console.log('Squads do guardião criado:', assinatura, dados);
