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

/// Tamanho máximo do nome de um material, em bytes UTF-8.
pub const MATERIAL_NOME_MAX: usize = 32;
/// Nome de exibição do participante, em bytes UTF-8 (ocupa o antigo espaço reservado da conta).
pub const PARTICIPANTE_NOME_MAX: usize = 32;

/// Base dos percentuais (basis points): 10_000 = 100%.
pub const BPS_BASE: u64 = 10_000;

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
