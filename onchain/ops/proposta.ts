// Propostas no Squads da devnet: cria a transação do vault 0, aprova com 3 membros de teste e,
// depois do timelock, executa. A carteira local paga as taxas; o membro criador paga o rent da proposta
// (~0,0035 SOL, exigência do Squads), por isso precisa de um pouco de saldo.
//
// Uso direto:  npx tsx proposta.ts status            (lista as propostas registradas)
//              npx tsx proposta.ts aprovar <índice>  (cria a proposta, se faltar, e aprova com o quórum)
//              npx tsx proposta.ts timelock <segundos> (propõe mudar o timelock do multisig)
//              npx tsx proposta.ts executar <índice> (executa uma proposta aprovada, após o timelock)
import * as multisig from '@sqds/multisig';
import { Connection, PublicKey, TransactionInstruction, TransactionMessage } from '@solana/web3.js';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { carregarOuGerar, carteiraLocal, DIR, lerJson, MEMBROS, RPC_DEVNET, salvarJson } from './comum';

type Squads = { multisig: string; vaultAdmin: string; quorum: number };
type Registro = { indice: string; descricao: string; criadaEm: string; executada?: string };

export const connection = new Connection(RPC_DEVNET, 'confirmed');
const squads = lerJson<Squads>('squads.json');
const multisigPda = new PublicKey(squads.multisig);
export const vaultAdmin = new PublicKey(squads.vaultAdmin);

/** Instrução no formato do @solana/kit (clientes Codama), convertida para o SDK do Squads. */
export type InstrucaoKit = {
    programAddress: string;
    accounts?: readonly { address: string; role: number }[];
    data?: Uint8Array | ArrayLike<number>;
};

export function paraWeb3(ix: InstrucaoKit): TransactionInstruction {
    // AccountRole do kit: 0 leitura, 1 gravável, 2 assinante, 3 assinante gravável.
    return new TransactionInstruction({
        programId: new PublicKey(ix.programAddress),
        keys: (ix.accounts ?? []).map((a) => ({
            pubkey: new PublicKey(a.address),
            isSigner: a.role >= 2,
            isWritable: (a.role & 1) === 1,
        })),
        data: Buffer.from(ix.data ?? []),
    });
}

function registros(): Registro[] {
    return existsSync(join(DIR, 'propostas.json')) ? lerJson<Registro[]>('propostas.json') : [];
}

/** Cria a proposta com as instruções (assinadas pelo vault 0) e aprova com o quórum. Devolve o índice. */
export async function propor(descricao: string, instrucoes: TransactionInstruction[]): Promise<bigint> {
    const pagador = carteiraLocal();
    const membros = MEMBROS.slice(0, squads.quorum).map(carregarOuGerar);
    const conta = await multisig.accounts.Multisig.fromAccountAddress(connection, multisigPda);
    const indice = BigInt(conta.transactionIndex.toString()) + 1n;
    const { blockhash } = await connection.getLatestBlockhash();

    const confirmar = async (sig: string) => {
        await connection.confirmTransaction(sig, 'confirmed');
        return sig;
    };
    await confirmar(
        await multisig.rpc.vaultTransactionCreate({
            connection,
            feePayer: pagador,
            multisigPda,
            transactionIndex: indice,
            creator: membros[0].publicKey,
            rentPayer: pagador.publicKey,
            vaultIndex: 0,
            ephemeralSigners: 0,
            transactionMessage: new TransactionMessage({ payerKey: vaultAdmin, recentBlockhash: blockhash, instructions: instrucoes }),
            memo: descricao,
            signers: [membros[0]],
        }),
    );
    await aprovar(indice);
    salvarJson('propostas.json', [...registros(), { indice: indice.toString(), descricao, criadaEm: new Date().toISOString() }]);
    return indice;
}

/** Cria a proposta (se ainda não existir) e aprova com o quórum. O criador paga o rent da proposta. */
export async function aprovar(indice: bigint) {
    const pagador = carteiraLocal();
    const membros = MEMBROS.slice(0, squads.quorum).map(carregarOuGerar);
    const [proposta] = multisig.getProposalPda({ multisigPda, transactionIndex: indice });
    if (!(await connection.getAccountInfo(proposta))) {
        const sig = await multisig.rpc.proposalCreate({ connection, feePayer: pagador, creator: membros[0], multisigPda, transactionIndex: indice });
        await connection.confirmTransaction(sig, 'confirmed');
    }
    const atual = await multisig.accounts.Proposal.fromAccountAddress(connection, proposta);
    const jaAprovaram = new Set(atual.approved.map((k) => k.toBase58()));
    for (const membro of membros) {
        if (jaAprovaram.has(membro.publicKey.toBase58())) continue;
        const sig = await multisig.rpc.proposalApprove({ connection, feePayer: pagador, member: membro, multisigPda, transactionIndex: indice });
        await connection.confirmTransaction(sig, 'confirmed');
    }
}

/** Proposta de configuração do próprio multisig: muda o timelock (em segundos). */
export async function proporTimelock(segundos: number): Promise<bigint> {
    const pagador = carteiraLocal();
    const criador = carregarOuGerar(MEMBROS[0]);
    const conta = await multisig.accounts.Multisig.fromAccountAddress(connection, multisigPda);
    const indice = BigInt(conta.transactionIndex.toString()) + 1n;
    const descricao = `timelock de ${conta.timeLock} s para ${segundos} s`;
    const sig = await multisig.rpc.configTransactionCreate({
        connection,
        feePayer: pagador,
        multisigPda,
        transactionIndex: indice,
        creator: criador.publicKey,
        rentPayer: pagador.publicKey,
        actions: [{ __kind: 'SetTimeLock', newTimeLock: segundos }],
        memo: descricao,
        signers: [criador],
    });
    await connection.confirmTransaction(sig, 'confirmed');
    await aprovar(indice);
    salvarJson('propostas.json', [...registros(), { indice: indice.toString(), descricao, criadaEm: new Date().toISOString() }]);
    return indice;
}

export async function executar(indice: bigint) {
    const pagador = carteiraLocal();
    const membro = carregarOuGerar(MEMBROS[0]);
    // A mesma sequência de índices guarda transações do vault e de configuração do multisig.
    const [transacao] = multisig.getTransactionPda({ multisigPda, index: indice });
    const ehConfig = await multisig.accounts.ConfigTransaction.fromAccountAddress(connection, transacao)
        .then(() => true)
        .catch(() => false);
    const sig = ehConfig
        ? await multisig.rpc.configTransactionExecute({
              connection,
              feePayer: pagador,
              multisigPda,
              transactionIndex: indice,
              member: membro,
              rentPayer: pagador,
              signers: [membro],
          })
        : await multisig.rpc.vaultTransactionExecute({
              connection,
              feePayer: pagador,
              multisigPda,
              transactionIndex: indice,
              member: membro.publicKey,
              signers: [membro],
          });
    await connection.confirmTransaction(sig, 'confirmed');
    salvarJson(
        'propostas.json',
        registros().map((r) => (r.indice === indice.toString() ? { ...r, executada: new Date().toISOString() } : r)),
    );
    return sig;
}

export async function status() {
    const conta = await multisig.accounts.Multisig.fromAccountAddress(connection, multisigPda);
    console.log(`timelock: ${conta.timeLock} s`);
    for (const r of registros()) {
        const [pda] = multisig.getProposalPda({ multisigPda, transactionIndex: BigInt(r.indice) });
        const p = await multisig.accounts.Proposal.fromAccountAddress(connection, pda).catch(() => null);
        const st = p?.status as { __kind: string; timestamp?: { toString(): string } } | undefined;
        const desde = st?.timestamp ? new Date(Number(st.timestamp.toString()) * 1000) : undefined;
        const liberaEm = desde && st?.__kind === 'Approved' ? new Date(desde.getTime() + conta.timeLock * 1000) : undefined;
        console.log(
            `#${r.indice} ${st?.__kind ?? '?'}${liberaEm ? ` (executável a partir de ${liberaEm.toLocaleString('pt-BR')})` : ''} ${r.descricao}`,
        );
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    const [cmd, arg] = process.argv.slice(2);
    if (cmd === 'status') await status();
    else if (cmd === 'executar' && arg) console.log('executada:', await executar(BigInt(arg)));
    else if (cmd === 'timelock' && arg) {
        const indice = await proporTimelock(Number(arg));
        console.log(`proposta #${indice} (timelock) criada e aprovada; executar depois do timelock atual`);
    } else if (cmd === 'aprovar' && arg) {
        await aprovar(BigInt(arg));
        console.log(`proposta #${arg} aprovada`);
    }
    else console.log('uso: npx tsx proposta.ts status | executar <índice>');
}
