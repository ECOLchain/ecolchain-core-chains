// Proposta no Squads: fecha buffers de upgrade que ficaram sem uso (autoridade = vault 0) e devolve
// o rent à carteira local. Close via CPI é permitido pelo loader (diferente do ExtendProgram).
// Uso: npx tsx fechar-buffers.ts <buffer> [<buffer> ...]
import { PublicKey, TransactionInstruction } from '@solana/web3.js';
import { carteiraLocal } from './comum';
import { propor, vaultAdmin } from './proposta';

const LOADER = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
const buffers = process.argv.slice(2).map((b) => new PublicKey(b));
if (buffers.length === 0) {
    console.error('uso: npx tsx fechar-buffers.ts <buffer> [...]');
    process.exit(1);
}
const destino = carteiraLocal().publicKey;
const data = Buffer.alloc(4);
data.writeUInt32LE(5, 0); // Close
const instrucoes = buffers.map(
    (buffer) =>
        new TransactionInstruction({
            programId: LOADER,
            keys: [
                { pubkey: buffer, isSigner: false, isWritable: true },
                { pubkey: destino, isSigner: false, isWritable: true },
                { pubkey: vaultAdmin, isSigner: true, isWritable: false },
            ],
            data,
        }),
);
const indice = await propor(`fechar buffers: ${buffers.map((b) => b.toBase58()).join(', ')}`, instrucoes);
console.log(`Proposta #${indice} criada e aprovada por 3 de 5.`);
console.log(`Executar: npx tsx proposta.ts executar ${indice}`);
