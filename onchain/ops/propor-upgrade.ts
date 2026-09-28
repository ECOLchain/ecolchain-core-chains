// Proposta no Squads: upgrade de programas a partir de buffers já gravados (autoridade do buffer = vault 0).
// Se o binário novo for maior, amplia a conta do programa antes, fora da proposta (ExtendProgram é
// permissionless e não pode ser chamado via CPI), pagando com a carteira local.
//
// Preparar cada buffer:
//   solana program write-buffer target/verifiable/<programa>.so -u devnet
//   solana program set-buffer-authority <BUFFER> --new-buffer-authority <vault 0> -u devnet
// Uso: npx tsx propor-upgrade.ts <program-id>=<buffer> [<program-id>=<buffer> ...]
import {
    PublicKey,
    sendAndConfirmTransaction,
    SystemProgram,
    SYSVAR_CLOCK_PUBKEY,
    SYSVAR_RENT_PUBKEY,
    Transaction,
    TransactionInstruction,
} from '@solana/web3.js';
import { carteiraLocal } from './comum';
import { connection, propor, vaultAdmin } from './proposta';

const LOADER = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
const CABECALHO_BUFFER = 37; // tag (4) + Option<authority> (1 + 32)
const CABECALHO_PROGRAMDATA = 45; // tag (4) + slot (8) + Option<authority> (1 + 32)
const MINIMO_EXTENSAO = 10_240; // exigência do loader para ExtendProgram

const pares = process.argv.slice(2).map((a) => a.split('=').map((x) => new PublicKey(x)) as [PublicKey, PublicKey]);
if (pares.length === 0) {
    console.error('uso: npx tsx propor-upgrade.ts <program-id>=<buffer> [...]');
    process.exit(1);
}

const u32 = (v: number) => {
    const b = Buffer.alloc(4);
    b.writeUInt32LE(v, 0);
    return b;
};

const pagador = carteiraLocal();
const spill = pagador.publicKey;
const instrucoes: TransactionInstruction[] = [];

for (const [programa, buffer] of pares) {
    const [programData] = PublicKey.findProgramAddressSync([programa.toBuffer()], LOADER);
    const [contaBuffer, contaPd] = await Promise.all([connection.getAccountInfo(buffer), connection.getAccountInfo(programData)]);
    if (!contaBuffer || !contaPd) throw new Error(`buffer ou programdata inexistente para ${programa.toBase58()}`);
    if (!new PublicKey(contaBuffer.data.subarray(5, 37)).equals(vaultAdmin)) {
        throw new Error(`a autoridade do buffer ${buffer.toBase58()} não é o vault 0`);
    }
    const falta = contaBuffer.data.length - CABECALHO_BUFFER - (contaPd.data.length - CABECALHO_PROGRAMDATA);
    if (falta > 0) {
        // A ampliação não pode ir dentro da proposta: o runtime recusa ExtendProgram via CPI
        // ("not supported by inner instructions"). O ExtendProgram simples é permissionless:
        // a carteira local amplia e paga o rent aqui, antes da proposta (mínimo de 10.240 bytes).
        const bytes = Math.max(falta, MINIMO_EXTENSAO);
        const data = Buffer.alloc(8);
        data.writeUInt32LE(6, 0);
        data.writeUInt32LE(bytes, 4);
        const tx = new Transaction().add(
            new TransactionInstruction({
                programId: LOADER,
                keys: [
                    { pubkey: programData, isSigner: false, isWritable: true },
                    { pubkey: programa, isSigner: false, isWritable: true },
                    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
                    { pubkey: pagador.publicKey, isSigner: true, isWritable: true },
                ],
                data,
            }),
        );
        await sendAndConfirmTransaction(connection, tx, [pagador], { commitment: 'confirmed' });
        console.log(`${programa.toBase58()}: ampliado em ${bytes} bytes`);
    }
    // Upgrade (3): programdata, programa, buffer, spill, rent, clock, autoridade.
    instrucoes.push(
        new TransactionInstruction({
            programId: LOADER,
            keys: [
                { pubkey: programData, isSigner: false, isWritable: true },
                { pubkey: programa, isSigner: false, isWritable: true },
                { pubkey: buffer, isSigner: false, isWritable: true },
                { pubkey: spill, isSigner: false, isWritable: true },
                { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
                { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
                { pubkey: vaultAdmin, isSigner: true, isWritable: false },
            ],
            data: u32(3),
        }),
    );
    console.log(`${programa.toBase58()}: upgrade com o buffer ${buffer.toBase58()}`);
}

const indice = await propor(`upgrade: ${pares.map(([p]) => p.toBase58()).join(', ')}`, instrucoes);
console.log(`\nProposta #${indice} criada e aprovada por 3 de 5.`);
console.log(`Depois do timelock: npx tsx proposta.ts executar ${indice}`);
