#!/usr/bin/env bun
// SupaWallet CLI: pay any GitHub user on Solana, and spend with the SSH key on your GitHub profile.
//
//   supawallet address <github-id>                      the user's vault (receives SOL before they ever sign up)
//   supawallet fund <github-id> <sol>                   send SOL to a GitHub user's vault
//   supawallet sync <github-login> [--id N] [--pub F]   write their current GitHub ed25519 keys on-chain (authority)
//   supawallet send <sol> <to> --id <github-id> --key <ssh key> [--fee <sol>]
//                                                       sign the intent with stock ssh-keygen and submit it
//
// Environment: RPC (default devnet), PAYER (keypair JSON that pays transaction fees), AUTHORITY (key-sync keypair).
import {
  Connection, Ed25519Program, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction,
  TransactionInstruction, SYSVAR_INSTRUCTIONS_PUBKEY, sendAndConfirmTransaction,
} from "@solana/web3.js";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID ?? "EHLyW17oB3NjpPEiAGJthSDG4vsX9dV5QgWnoT6JTZ4N");
export const NAMESPACE = "supawallet@v1";
const conn = new Connection(process.env.RPC ?? "https://api.devnet.solana.com", "confirmed");

const u64 = (n: bigint) => { const b = Buffer.alloc(8); b.writeBigUInt64LE(n); return b; };
const pda = (seed: string, id: bigint) => PublicKey.findProgramAddressSync([Buffer.from(seed), u64(id)], PROGRAM_ID)[0];
export const vaultOf = (id: bigint) => pda("vault", id);
const sha256 = (b: Buffer) => createHash("sha256").update(b).digest();
const sshString = (b: Buffer) => { const l = Buffer.alloc(4); l.writeUInt32BE(b.length); return Buffer.concat([l, b]); };
const loadKeypair = (path: string) => Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, "utf8"))));
const sol = (s: string) => BigInt(Math.round(Number(s) * LAMPORTS_PER_SOL));

/** The exact text a spend signs; the program rebuilds it byte for byte. */
export function intent(vault: PublicKey, id: bigint, nonce: bigint, amount: bigint, to: PublicKey, fee: bigint) {
  return `supawallet v1\ncluster: devnet\nvault: ${vault.toBase58()}\ngithub-id: ${id}\nnonce: ${nonce}\n` +
    `send: ${amount} lamports\nto: ${to.toBase58()}\nrelayer-fee: ${fee}\n`;
}

/** OpenSSH PROTOCOL.sshsig signed data for a sha256 signature in our namespace. */
export function signedData(message: Buffer) {
  return Buffer.concat([Buffer.from("SSHSIG"), sshString(Buffer.from(NAMESPACE)), sshString(Buffer.alloc(0)),
    sshString(Buffer.from("sha256")), sshString(sha256(message))]);
}

/** Parse an armored SSHSIG made by an ed25519 key: returns the 32-byte public key and 64-byte signature. */
export function parseSshSig(armored: string) {
  const b = Buffer.from(armored.replace(/-----[^-]+-----/g, "").replace(/\s+/g, ""), "base64");
  let o = 0;
  const str = () => { const l = b.readUInt32BE(o); o += 4; const s = b.subarray(o, o + l); o += l; return s; };
  if (b.subarray(0, 6).toString() !== "SSHSIG") throw new Error("not an SSHSIG signature");
  o = 10; // magic + version
  const pubBlob = str(); const ns = str().toString(); str(); const hashAlg = str().toString(); const sigBlob = str();
  if (ns !== NAMESPACE || hashAlg !== "sha256") throw new Error(`wrong namespace/hash: ${ns}/${hashAlg}`);
  o = 0; const pb = pubBlob; let p = 0;
  const pstr = (x: Buffer) => { const l = x.readUInt32BE(p); p += 4; const s = x.subarray(p, p + l); p += l; return s; };
  if (pstr(pb).toString() !== "ssh-ed25519") throw new Error("only ssh-ed25519 keys are supported on Solana");
  const publicKey = pstr(pb); p = 0; pstr(sigBlob); const signature = pstr(sigBlob);
  return { publicKey, signature };
}

/** keyHash = sha256(raw SSH key blob), from an authorized_keys line. */
export function keyHash(line: string) {
  return sha256(Buffer.from(line.trim().split(/\s+/)[1], "base64"));
}

const flag = (args: string[], name: string) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };

async function main() {
  const [cmd, ...args] = process.argv.slice(2);
  const payer = () => loadKeypair(process.env.PAYER ?? `${process.env.HOME}/.config/solana/id.json`);

  if (cmd === "address") {
    console.log(vaultOf(BigInt(args[0])).toBase58());
  } else if (cmd === "fund") {
    const [id, amount] = [BigInt(args[0]), sol(args[1])];
    const tx = new Transaction().add(SystemProgram.transfer({ fromPubkey: payer().publicKey, toPubkey: vaultOf(id), lamports: amount }));
    console.log("funded", vaultOf(id).toBase58(), await sendAndConfirmTransaction(conn, tx, [payer()]));
  } else if (cmd === "sync") {
    const login = args[0];
    const id = BigInt(flag(args, "--id") ?? (await (await fetch(`https://api.github.com/users/${login}`, { headers: { "user-agent": "supawallet" } })).json()).id);
    const lines = (flag(args, "--pub") ? readFileSync(flag(args, "--pub")!, "utf8")
      : await (await fetch(`https://github.com/${login}.keys`)).text()).split("\n").filter((l) => l.startsWith("ssh-ed25519 "));
    const hashes = lines.map(keyHash);
    const authority = loadKeypair(process.env.AUTHORITY ?? ".keys/authority.json");
    const data = Buffer.concat([Buffer.from([0]), u64(id), Buffer.from([hashes.length]), ...hashes]);
    const ix = new TransactionInstruction({ programId: PROGRAM_ID, data, keys: [
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
      { pubkey: pda("keys", id), isSigner: false, isWritable: true },
      { pubkey: pda("state", id), isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ] });
    console.log(`synced ${hashes.length} ed25519 key(s) for ${login} (id ${id}):`,
      await sendAndConfirmTransaction(conn, new Transaction().add(ix), [authority]));
  } else if (cmd === "send") {
    const [amount, to] = [sol(args[0]), new PublicKey(args[1])];
    const id = BigInt(flag(args, "--id")!); const key = flag(args, "--key")!; const fee = sol(flag(args, "--fee") ?? "0");
    const state = await conn.getAccountInfo(pda("state", id));
    if (!state) throw new Error("no keys synced for this GitHub id yet: run `supawallet sync`");
    const nonce = state.data.readBigUInt64LE(0);
    const text = intent(vaultOf(id), id, nonce, amount, to, fee);
    process.stderr.write(`\nYou are signing exactly this:\n\n${text}\n`);
    const dir = mkdtempSync(join(tmpdir(), "supawallet-"));
    writeFileSync(join(dir, "intent.txt"), text);
    execFileSync("ssh-keygen", ["-Y", "sign", "-n", NAMESPACE, "-O", "hashalg=sha256", "-f", key, join(dir, "intent.txt")], { stdio: "inherit" });
    const { publicKey, signature } = parseSshSig(readFileSync(join(dir, "intent.txt.sig"), "utf8"));
    const verify = Ed25519Program.createInstructionWithPublicKey({ publicKey, signature, message: signedData(Buffer.from(text)) });
    const relayer = payer();
    const exec = new TransactionInstruction({ programId: PROGRAM_ID, data: Buffer.concat([Buffer.from([1]), u64(id), u64(amount), u64(fee)]), keys: [
      { pubkey: relayer.publicKey, isSigner: true, isWritable: true },
      { pubkey: vaultOf(id), isSigner: false, isWritable: true },
      { pubkey: pda("state", id), isSigner: false, isWritable: true },
      { pubkey: pda("keys", id), isSigner: false, isWritable: false },
      { pubkey: to, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ] });
    console.log("paid:", await sendAndConfirmTransaction(conn, new Transaction().add(verify, exec), [relayer]));
  } else {
    console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n").slice(1, 9).join("\n"));
  }
}

if (import.meta.main) main().catch((e) => { console.error(e.message ?? e); process.exit(1); });
