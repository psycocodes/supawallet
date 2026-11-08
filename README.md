# SupaWallet

**Every GitHub user already has a Solana wallet. They spend it with the SSH key on their GitHub profile.**

Pay anyone by GitHub ID: the SOL lands in a vault that exists before they've ever heard of SupaWallet. To spend, they sign a plain-text intent with the stock `ssh-keygen` they already have (no seed phrase, no wallet app, no signup), and a Solana program checks that signature against the keys on their GitHub profile.

Built for the MLH Solana Hackathon (6–8 Nov 2026).

## How it works

| Piece | What it is |
|---|---|
| **Vault** | PDA `["vault", github_id]`: a plain system account, so anyone can send SOL to it at any time |
| **Key set** | PDA `["keys", github_id]`: sha256 hashes of the user's GitHub `ssh-ed25519` keys plus when each was first seen. Written by a key-sync authority that reads `github.com/<user>.keys`; a key deleted on GitHub disappears at the next sync, which revokes it |
| **Execute** | The user signs the intent below with `ssh-keygen -Y sign -n supawallet@v1 -O hashalg=sha256`. The transaction carries Solana's **native Ed25519 program** over the SSHSIG signed data; our program reads it through the instructions sysvar, rebuilds the intent and the SSHSIG signed data on-chain, checks they are exactly what was verified and that the key is on the profile, then bumps the nonce and pays. Anyone can submit and earns the signed relayer fee |
| **Safety** | Spends over 0.5 SOL from a key first seen less than 72 hours ago are refused (account-takeover guard). Users sign readable text, never blind transaction bytes |

The intent (every line ends in `\n`):

```
supawallet v1
cluster: devnet
vault: <vault, base58>
github-id: <numeric GitHub id>
nonce: <n>
send: <lamports> lamports
to: <recipient, base58>
relayer-fee: <lamports>
```

Solana makes this cheap: ed25519, the default `ssh-keygen` key type, is verified by a native program instead of in contract code.

## Live on devnet

- **Web app:** https://psycocodes.github.io/supawallet/ (look up any GitHub user, pay their vault with Phantom on devnet, spend by pasting an `ssh-keygen` signature). Source: `web/index.html`, a single static page with no backend
- Program: [`EHLyW17oB3NjpPEiAGJthSDG4vsX9dV5QgWnoT6JTZ4N`](https://explorer.solana.com/address/EHLyW17oB3NjpPEiAGJthSDG4vsX9dV5QgWnoT6JTZ4N?cluster=devnet)
- A payment signed with stock `ssh-keygen` (test GitHub ID 4242, test key): [`3p5N3rtL…ieDQQicsD`](https://explorer.solana.com/tx/3p5N3rtLRJTFVkXYoKDTj1pTwkta5N12Zr1vv8rVdGyvEvQrTqdspCkjUmiDQWWzuJdUmkUg2LzSnq9ieDQQicsD?cluster=devnet)
- Demo video: https://youtu.be/KTiar2QqC-M

## Run it

```sh
# program (native Rust, solana-program 5)
cd program && cargo test && cargo build-sbf
solana program deploy target/deploy/supawallet.so --program-id <program keypair>

# client (Bun + @solana/web3.js)
cd client && bun install
bun src/cli.ts address <github-id>                     # where to pay someone
bun src/cli.ts fund <github-id> 1                      # pay them
AUTHORITY=<keypair> bun src/cli.ts sync <login>        # put their GitHub ed25519 keys on-chain
bun src/cli.ts send 0.25 <to> --id <github-id> --key ~/.ssh/id_ed25519 --fee 0.001
```

`RPC` picks the cluster (default devnet), `PAYER` the keypair that pays transaction fees.

**Tested** end to end on `solana-test-validator`: sync a key, fund, spend 0.25 SOL with an `ssh-keygen` signature, a 1 SOL spend from the new key is refused (error 8, 72 h delay), and after revoking the key the next spend is refused (error 7).

## Limits and next steps

- ed25519 keys only (P-256 via Solana's secp256r1 program is next; RSA has no native verifier).
- The key-sync authority is a single key today; the plan is a decentralised oracle that reaches consensus on GitHub's key list.
- Next: SPL tokens (USDC), a pay page, and git commits signed with your SSH key as payments.

## AI usage

Written with Claude Code (Claude Opus 5.5) pairing with the team; commits carry a `Co-Authored-By` line.

## License

MIT

## Hackathon submission

- Event page: https://www.mlh.com/events/solana-hackathon
- Submission form (sign in with your MLH account): https://www.mlh.com/events/solana-hackathon/submissions/new
- Deadline: **Nov 8, 1:00 PM EST**

### Submitting with an AI agent (DevRelay)

Paste this prompt into your coding agent (e.g. Claude Code) from the repo root:

```text
Submit the project in this repo to Solana Hackathon on MLH (event slug: solana-hackathon) using the DevRelay MCP server.

If you don't have DevRelay's tools yet, install it first by running:
  macOS or Linux: curl -fsSL https://devrelay.com/install.sh | sh
  Windows (PowerShell): iwr -useb https://devrelay.com/install.ps1 | iex
Then stop and tell me to restart you and paste this prompt again, so you pick up the new tools.
If DevRelay says my MLH account isn't connected, call connect_mlh_account and wait while I sign in.

1. Fill in the project from this repo: a name, a one-line pitch (short_description), a description based on the README, the technologies the code uses (including any AI APIs or models it calls, like Gemini or OpenAI), the GitHub link, and any live demo or demo video links you find. Put any other links, like a second repo or slides, in extra_links with a short label. Ask me for anything you can't find.
2. Fill in ai_tools: your own harness and the model you're running on (for example claude-code and coding:anthropic/claude-opus-5.5), nothing else. AI APIs the project calls go with the technologies.
3. Enter it in the prize categories it fits: Top 3 Overall; Best Beginner Hack; Gaming Track Top Prize; DeFi Track Top Prize; AI Track Top Prize. For each one, write a sentence or two on how the project uses it and send it as that category's sponsor_usage answer. Skip any it doesn't fit.
4. Submit it before Nov 8, 1:00 PM EST. If DevRelay's tools don't accept short_description, ai_tools, sponsor_usage or extra_links yet, submit what they do accept, then give me the pitch and category answers so I can paste them into the form. Tell me exactly what you submitted and give me the project's link.
```
