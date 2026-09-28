#!/usr/bin/env bash
# Deploy da EcolChain na DEVNET com build verificável e Squads (3 de 5, chaves de teste).
#
# Ordem: checagens → build verificável → chaves + Squads → deploy → init das configs (a carteira
# local ainda é upgrade authority) → confere hash on-chain × build → upgrade authority → vault do Squads.
#
# Uso:  scripts/deploy-devnet.sh            (pede confirmação)
#       CONFIRMAR=1 scripts/deploy-devnet.sh
set -euo pipefail

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
RPC="${RPC_DEVNET:-https://api.devnet.solana.com}"
CARTEIRA="${CARTEIRA:-$HOME/.config/solana/id.json}"
PROGRAMAS=(ecol_lote ecol_credito ecol_reparticao)
SALDO_MIN_SOL=11   # ~7,5 SOL de rent + buffer temporário do maior programa (~3,4 SOL)

falha() { echo "ERRO: $*" >&2; exit 1; }

# --- checagens -------------------------------------------------------------------------------
[[ "$RPC" == *mainnet* ]] && falha "este script é só para devnet (RPC=$RPC)"
docker info >/dev/null 2>&1 || falha "Docker indisponível no WSL: ative Settings → Resources → WSL integration no Docker Desktop"
command -v solana-verify >/dev/null || falha "instale o solana-verify: cargo install solana-verify"
[[ -f "$CARTEIRA" ]] || falha "carteira não encontrada: $CARTEIRA"
PAGADOR="$(solana address -k "$CARTEIRA")"
SALDO="$(solana balance -u "$RPC" -k "$CARTEIRA" | awk '{print $1}')"
awk -v s="$SALDO" -v m="$SALDO_MIN_SOL" 'BEGIN { exit !(s >= m) }' \
    || falha "saldo $SALDO SOL < $SALDO_MIN_SOL SOL em $PAGADOR (use https://faucet.solana.com)"

echo "Deploy na devnet"
echo "  RPC:       $RPC"
echo "  pagador:   $PAGADOR ($SALDO SOL)"
for p in "${PROGRAMAS[@]}"; do echo "  programa:  $p $(solana address -k "$RAIZ/target/deploy/$p-keypair.json")"; done
if [[ "${CONFIRMAR:-}" != "1" ]]; then
    read -r -p "Publicar na devnet? [s/N] " r
    [[ "$r" == "s" || "$r" == "S" ]] || falha "cancelado"
fi

# --- build verificável -----------------------------------------------------------------------
cd "$RAIZ"
NO_DNA=1 anchor build --verifiable
for p in "${PROGRAMAS[@]}"; do [[ -f "target/verifiable/$p.so" ]] || falha "build verificável sem target/verifiable/$p.so"; done

# --- chaves de teste e Squads ----------------------------------------------------------------
(cd ops && npm ci --silent && npx tsx chaves-devnet.ts && RPC_DEVNET="$RPC" CARTEIRA="$CARTEIRA" npx tsx squads-devnet.ts)
VAULT_ADMIN="$(node -p "require('$RAIZ/.devnet/squads.json').vaultAdmin")"

# --- deploy ----------------------------------------------------------------------------------
for p in "${PROGRAMAS[@]}"; do
    solana program deploy "target/verifiable/$p.so" \
        --program-id "target/deploy/$p-keypair.json" \
        --keypair "$CARTEIRA" -u "$RPC"
done

# --- configs (antes de entregar a upgrade authority) -----------------------------------------
(cd clients && npm ci --silent && RPC_DEVNET="$RPC" CARTEIRA="$CARTEIRA" npx tsx scripts/init-devnet.ts)

# --- verificação: hash on-chain = hash do build verificável ----------------------------------
for p in "${PROGRAMAS[@]}"; do
    ID="$(solana address -k "target/deploy/$p-keypair.json")"
    LOCAL="$(solana-verify get-executable-hash "target/verifiable/$p.so")"
    ONCHAIN="$(solana-verify get-program-hash -u "$RPC" "$ID")"
    [[ "$LOCAL" == "$ONCHAIN" ]] || falha "$p: hash on-chain $ONCHAIN ≠ build $LOCAL"
    echo "$p: hash confere ($LOCAL)"
done

# --- upgrade authority → vault 0 do Squads ---------------------------------------------------
for p in "${PROGRAMAS[@]}"; do
    ID="$(solana address -k "target/deploy/$p-keypair.json")"
    solana program set-upgrade-authority "$ID" \
        --new-upgrade-authority "$VAULT_ADMIN" --skip-new-upgrade-authority-signer-check \
        --keypair "$CARTEIRA" -u "$RPC"
done

echo
echo "Concluído. Upgrade authority e admin: $VAULT_ADMIN (Squads 3 de 5)."
echo "Endereços e chaves de teste: $RAIZ/.devnet (fora do git)."
