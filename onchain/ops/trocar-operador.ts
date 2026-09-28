// Proposta no Squads: troca o operador dos três programas para outra carteira, mantendo o resto das configs.
// Uso: npx tsx trocar-operador.ts <nova-carteira>
// Depois do timelock: npx tsx proposta.ts executar <índice>
import { address, createNoopSigner, createSolanaRpc, isAddress } from '@solana/kit';
import * as credito from '../clients/src/generated/ecol_credito';
import * as lote from '../clients/src/generated/ecol_lote';
import * as rep from '../clients/src/generated/ecol_reparticao';
import { credito as pCredito, eventAuthority, lote as pLote, reparticao as pRep } from '../clients/src/pdas';
import { RPC_DEVNET } from './comum';
import { paraWeb3, propor, vaultAdmin } from './proposta';

const novo = process.argv[2];
if (!novo || !isAddress(novo)) {
    console.error('uso: npx tsx trocar-operador.ts <nova-carteira>');
    process.exit(1);
}
const operador = address(novo);
const rpc = createSolanaRpc(RPC_DEVNET);
const admin = createNoopSigner(address(vaultAdmin.toBase58()));

const g = await lote.fetchGlobalConfig(rpc, await pLote.config());
const c = await credito.fetchCreditoConfig(rpc, await pCredito.config());
const r = await rep.fetchReparticaoConfig(rpc, await pRep.config());
if (g.data.admin !== admin.address) throw new Error(`o admin do ecol_lote não é o vault 0 (${g.data.admin})`);

const participante = await lote.fetchMaybeParticipante(rpc, await pLote.participante(operador));
if (participante.exists) throw new Error(`${operador} já é participante; uma carteira não pode ser participante e operador`);

const instrucoes = [
    await lote.getAdminUpdateGlobalInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS),
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        params: {
            arbitro: g.data.arbitro,
            operador,
            intermediador: g.data.intermediador,
            creditoAutoridade: g.data.creditoAutoridade,
            prazoEntregaS: g.data.prazoEntregaS,
            toleranciaPesoBps: g.data.toleranciaPesoBps,
        },
    }),
    await credito.getAdminUpdateConfigInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
        operador,
        registrador: c.data.registrador,
        intermediador: c.data.intermediador,
    }),
    await rep.getAdminUpdateConfigInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
        params: { operador, zupy: r.data.zupy, intermediador: r.data.intermediador, bps: r.data.bps },
    }),
].map(paraWeb3);

console.log(`Operador atual: ${g.data.operador}`);
console.log(`Novo operador:  ${operador}`);
const indice = await propor(`trocar operador para ${operador}`, instrucoes);
console.log(`\nProposta #${indice} criada e aprovada por 3 de 5.`);
console.log(`Depois do timelock: npx tsx proposta.ts executar ${indice}`);
