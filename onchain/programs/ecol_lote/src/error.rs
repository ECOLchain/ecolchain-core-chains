use anchor_lang::prelude::*;

#[error_code]
pub enum EcolLoteError {
    #[msg("Programa pausado para esta operação")]
    ProgramaPausado,
    #[msg("Parâmetro de configuração inválido")]
    ParametroInvalido,
    #[msg("Somente a upgrade authority do programa pode inicializar a configuração")]
    NaoEhUpgradeAuthority,
    #[msg("Assinante não é o admin")]
    NaoEhAdmin,
    #[msg("Assinante não é o operador")]
    NaoEhOperador,
    #[msg("Assinante não é o intermediador de pagamento")]
    NaoEhIntermediador,
    #[msg("Assinante não é o árbitro")]
    NaoEhArbitro,
    #[msg("Assinante não é a autoridade do programa de créditos")]
    NaoEhAutoridadeCredito,
    #[msg("Participante inativo")]
    ParticipanteInativo,
    #[msg("Participante não tem o papel exigido")]
    PapelIncorreto,
    #[msg("Assinante não é a cooperativa dona do lote")]
    NaoEhCooperativaDoLote,
    #[msg("Assinante não é a indústria compradora do lote")]
    NaoEhIndustriaDoLote,
    #[msg("Assinante não é parte do lote")]
    NaoEhParteDoLote,
    #[msg("Transição de estado inválida para o estado atual do lote")]
    TransicaoInvalida,
    #[msg("Prazo inválido")]
    PrazoInvalido,
    #[msg("Leilão ainda em andamento")]
    LeilaoEmAndamento,
    #[msg("Prazo de entrega ainda não expirou")]
    PrazoNaoExpirado,
    #[msg("Peso deve ser maior que zero")]
    PesoInvalido,
    #[msg("Valor abaixo do preço mínimo do anúncio")]
    ValorAbaixoDoMinimo,
    #[msg("Referência (hash) obrigatória não informada")]
    ReferenciaVazia,
    #[msg("Estouro aritmético")]
    Overflow,
    #[msg("Falta a instrução Ed25519 da balança imediatamente antes desta")]
    AtestacaoAusente,
    #[msg("Instrução Ed25519 malformada ou com dados fora dela")]
    AtestacaoInvalida,
    #[msg("Assinatura não é da balança informada")]
    BalancaIncorreta,
    #[msg("Mensagem assinada pela balança não corresponde ao lote, peso ou horário")]
    PesagemDivergente,
    #[msg("Pesagem fora da janela de tempo aceita")]
    PesagemExpirada,
    #[msg("Balança inativa")]
    BalancaInativa,
    #[msg("Balança não pertence ao participante")]
    BalancaDeOutroDono,
    #[msg("URI de metadados vazia ou longa demais")]
    UriInvalida,
    #[msg("Asset informado não é o do lote")]
    AssetIncorreto,
    #[msg("Esta etapa já teve disputa julgada; só o árbitro pode reabri-la")]
    DisputaJaJulgada,
    #[msg("Crank de prazo em carência após retomada do programa")]
    EmCarencia,
    #[msg("Chaves de papéis do config devem ser distintas e não nulas")]
    ChavesDeConfigInvalidas,
    #[msg("Carteira é chave de papel do config e não pode ser participante")]
    CarteiraReservada,
    #[msg("Nenhuma troca de admin pendente para este assinante")]
    SemAdminPendente,
    #[msg("Valor a liberar maior que o valor da venda")]
    ValorLiberadoInvalido,
}
