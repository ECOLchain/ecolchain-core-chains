use anchor_lang::prelude::*;

#[constant]
pub const SEED_GLOBAL: &[u8] = b"global";

#[constant]
pub const SEED_PARTICIPANTE: &[u8] = b"part";

#[constant]
pub const SEED_LOTE: &[u8] = b"lote";

/// Entrega de um coletor na cooperativa: PDA `["entrega", cooperativa, entrega_id (u64 LE)]`.
#[constant]
pub const SEED_ENTREGA: &[u8] = b"entrega";

/// Entregas vinculadas a um lote por transação (cada uma é uma conta gravável).
pub const MAX_ENTREGAS_POR_TX: usize = 10;

/// Material cadastrado: PDA `["material", codigo (u16 LE)]`.
#[constant]
pub const SEED_MATERIAL: &[u8] = b"material";

/// Variação de um material (cor do vidro, por exemplo): PDA `["variacao", codigo (u16 LE), indice (u8)]`.
#[constant]
pub const SEED_VARIACAO: &[u8] = b"variacao";

/// Máximo de variações por material (índices 1..=MAX; 0 = sem variação).
pub const MAX_VARIACOES: u8 = 16;

/// Carteira vinculada a um participante: PDA `["carteira", endereco]` (ADR 0011).
#[constant]
pub const SEED_CARTEIRA: &[u8] = b"carteira";

/// Lote de distribuição do importador (garrafas postas no mercado): `["distrib", importador, id (u64 LE)]`.
#[constant]
pub const SEED_DISTRIBUICAO: &[u8] = b"distrib";

/// Coleta do importador (logística reversa): `["coleta", importador, id (u64 LE)]`.
#[constant]
pub const SEED_COLETA: &[u8] = b"coleta";

/// Lance de uma indústria num leilão: `["lance", lote, industria]` (ADR 0013).
#[constant]
pub const SEED_LANCE: &[u8] = b"lance";

/// Tamanho máximo do nome de um material, em bytes UTF-8.
pub const MATERIAL_NOME_MAX: usize = 32;
/// Nome de exibição de participantes e balanças, em bytes UTF-8 (ocupa o antigo espaço reservado das contas).
pub const NOME_FIXO_MAX: usize = 32;

/// Base dos percentuais (basis points): 10_000 = 100%.
pub const BPS_BASE: u64 = 10_000;
/// Pesagem do lote de venda consolidado: perda máxima aceita sobre a soma dos lotes de origem
/// (umidade, rejeito) e excesso máximo (erro de balança). Valem enquanto o config estiver zerado.
pub const PERDA_MONTAGEM_PADRAO_BPS: u16 = 1_000;
pub const EXCESSO_MONTAGEM_PADRAO_BPS: u16 = 200;
/// Teto para os dois limites acima (acima disso não é mais o mesmo material).
pub const TOLERANCIA_MONTAGEM_MAX_BPS: u16 = 3_000;

/// Limite superior de `tolerancia_peso_bps` (20%), para impedir que a verificação de peso seja desligada.
pub const TOLERANCIA_PESO_MAX_BPS: u16 = 2_000;

#[constant]
pub const SEED_BALANCA: &[u8] = b"balanca";

/// PDA que é update authority e autoridade dos plugins permanentes dos Core assets.
#[constant]
pub const SEED_AUTORIDADE_ASSET: &[u8] = b"autoridade_asset";

/// Core asset do lote: PDA `["asset", lote]`, então cada lote tem no máximo um asset.
#[constant]
pub const SEED_ASSET: &[u8] = b"asset";

/// Prefixo de domínio da mensagem assinada pela balança (evita reaproveitar assinaturas de outro contexto).
pub const DOMINIO_PESAGEM: &[u8; 17] = b"ECOLCHAIN:PESO:v1";

/// Idade máxima (e adiantamento máximo) aceita para o timestamp assinado pela balança.
pub const JANELA_PESAGEM_S: i64 = 15 * 60;

pub const URI_MAX_LEN: usize = 200;

/// Limite do prazo de entrega configurável (180 dias).
pub const PRAZO_ENTREGA_MAX_S: i64 = 180 * 24 * 3600;

/// Duração máxima de um anúncio (90 dias).
pub const DURACAO_LEILAO_MAX_S: i64 = 90 * 24 * 3600;

/// Depois de voltar ao modo Normal, o crank de prazo espera esta carência para as partes retomarem.
pub const CARENCIA_RETOMADA_S: i64 = 48 * 3600;
