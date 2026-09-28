// Cadastra uma carteira como participante do `ecol_lote` na devnet (instrução `operador_register_participante`).
// Assina como operador (chave de teste em ../.devnet/operador.json); a carteira local paga taxa e rent.
//
// Uso: npx tsx scripts/cadastrar-participante.ts <carteira> <coletor|cooperativa|transportador|industria> <nome> [referencia-kyc]
//
// `nome` é o nome de exibição (até 32 bytes, público on-chain).
// `referencia-kyc` identifica o cadastro off-chain; on-chain vai só o sha256 dela (LGPD). Sem ela,
// usa uma referência de teste derivada da carteira.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Address, createClient, createKeyPairSignerFromBytes, isAddress } from '@solana/kit';
import { solanaRpc } from '@solana/kit-plugin-rpc';
import { signerFromFile } from '@solana/kit-plugin-signer';
import * as lote from '../src/generated/ecol_lote';
import { bytesDoNome, PARTICIPANTE_NOME_MAX } from '../src/participante';
import { eventAuthority, lote as pLote } from '../src/pdas';

const DIR = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '.devnet');
const RPC = process.env.RPC_DEVNET ?? 'https://api.devnet.solana.com';
const WS = RPC.replace(/^http/, 'ws');
const CARTEIRA = process.env.CARTEIRA ?? join(homedir(), '.config', 'solana', 'id.json');

const PAPEIS: Record<string, lote.Papel> = {
    coletor: lote.Papel.Coletor,
    cooperativa: lote.Papel.Cooperativa,
    transportador: lote.Papel.Transportador,
    industria: lote.Papel.Industria,
};

function falha(msg: string): never {
    console.error(`ERRO: ${msg}`);
    process.exit(1);
}

const [carteiraArg, papelArg, nome, kycArg] = process.argv.slice(2);
if (!carteiraArg || !papelArg || !nome?.trim()) {
    falha('uso: npx tsx scripts/cadastrar-participante.ts <carteira> <coletor|cooperativa|transportador|industria> <nome> [referencia-kyc]');
}
if (bytesDoNome(nome) > PARTICIPANTE_NOME_MAX) falha(`nome com mais de ${PARTICIPANTE_NOME_MAX} bytes: ${nome}`);
if (RPC.includes('mainnet')) falha(`este script é só para devnet/localnet (RPC=${RPC})`);
if (!isAddress(carteiraArg)) falha(`endereço inválido: ${carteiraArg}`);
const papel = PAPEIS[papelArg.toLowerCase()];
if (papel === undefined) falha(`papel inválido: ${papelArg} (use ${Object.keys(PAPEIS).join(', ')})`);
const carteira: Address = carteiraArg;

const operador = await createKeyPairSignerFromBytes(
    Uint8Array.from(JSON.parse(readFileSync(join(DIR, 'operador.json'), 'utf8'))),
);
const client = await createClient()
    .use(signerFromFile(CARTEIRA))
    .use(solanaRpc({ rpcUrl: RPC, rpcSubscriptionsUrl: WS, transactionConfig: { version: 0 } }));

// Checagens antes de gastar: o programa recusaria de qualquer forma, mas com erro menos claro.
const configEndereco = await pLote.config();
const config = await lote.fetchMaybeGlobalConfig(client.rpc, configEndereco);
if (!config.exists) falha(`GlobalConfig do ecol_lote não encontrado em ${RPC}; os programas foram inicializados?`);
if (config.data.operador !== operador.address) {
    falha(`a chave em .devnet/operador.json (${operador.address}) não é o operador configurado (${config.data.operador})`);
}
if (config.data.modo !== lote.ModoOperacao.Normal) {
    falha(`o ecol_lote está em modo ${lote.ModoOperacao[config.data.modo]}; cadastros só em modo Normal`);
}
const chavesDePapel = [config.data.admin, config.data.arbitro, config.data.operador, config.data.intermediador, config.data.creditoAutoridade];
if (chavesDePapel.includes(carteira)) falha(`${carteira} é uma chave de papel da config e não pode ser participante`);

const participanteEndereco = await pLote.participante(carteira);
const existente = await lote.fetchMaybeParticipante(client.rpc, participanteEndereco);
if (existente.exists) {
    const atual = lote.Papel[existente.data.papel];
    console.log(`${carteira} já está cadastrada como ${atual} (${existente.data.ativo ? 'ativa' : 'inativa'}).`);
    console.log('O papel não pode ser trocado: para outro papel, use outra carteira.');
    process.exit(0);
}

const referencia = kycArg ?? `teste-devnet:${carteira}`;
const kycHash = createHash('sha256').update(referencia).digest();

console.log(`Cadastrando ${carteira} como ${lote.Papel[papel]}`);
console.log(`  rede:      ${RPC}`);
console.log(`  nome:      ${nome}`);
console.log(`  operador:  ${operador.address}`);
console.log(`  pagador:   ${client.payer.address}`);
console.log(`  kyc_hash:  sha256("${referencia}")`);

const ix = await lote.getOperadorRegisterParticipanteInstructionAsync({
    payer: client.payer,
    operador,
    eventAuthority: await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS),
    program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
    carteira,
    papel,
    kycHash,
    nome,
});
const resultado = await client.sendTransaction([ix]);

console.log(`\nCadastrada. Participante: ${participanteEndereco}`);
const cluster = RPC.includes('devnet') ? 'devnet' : `custom&customUrl=${encodeURIComponent(RPC)}`;
console.log(`Transação: https://explorer.solana.com/tx/${resultado.context.signature}?cluster=${cluster}`);
console.log('Reconecte a carteira na interface web para ver o menu do papel.');
