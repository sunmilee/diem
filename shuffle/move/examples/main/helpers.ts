// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

import * as DiemTypes from "./generated/diemTypes/mod.ts";
import * as ed from "https://deno.land/x/ed25519@1.0.1/mod.ts";
import { BcsSerializer } from "./generated/bcs/mod.ts";
import { ListTuple, uint8 } from "./generated/serde/types.ts";
import { createHash } from "https://deno.land/std@0.77.0/hash/mod.ts";

export function newRawTransaction(
  addressStr: string,
  payload: DiemTypes.TransactionPayloadVariantScript,
  sequenceNumber: number,
): DiemTypes.RawTransaction {
  return new DiemTypes.RawTransaction(
    hexToAccountAddress(addressStr),
    BigInt(sequenceNumber),
    payload, // txn payload
    BigInt(1000000), // max gas amount
    BigInt(0), // gas_unit_price
    "XUS", // currency
    BigInt(99999999999), // expiration_timestamp_secs
    new DiemTypes.ChainId(4), // chain id, hardcoded to test
  );
}

export function hashPrefix(name: string): Uint8Array {
  const hash = createHash("sha3-256");
  hash.update("DIEM::");
  hash.update(name);
  return new Uint8Array(hash.digest());
}

export function generateSigningMessage(
  rawTxn: DiemTypes.RawTransaction,
): Uint8Array {
  const bcsSerializer = new BcsSerializer();
  rawTxn.serialize(bcsSerializer);
  const rawTxnBytes = bcsSerializer.getBytes();

  const signingMsg = appendBuffer(hashPrefix("RawTransaction"), rawTxnBytes);
  return signingMsg;
}

export async function newSignedTransaction(
  privateKeyBytes: Uint8Array,
  rawTxn: DiemTypes.RawTransaction,
  signingMsg: Uint8Array,
): Promise<string> {
  const publicKey = await ed.getPublicKey(privateKeyBytes);

  const signatureTmp = await ed.sign(signingMsg, privateKeyBytes);
  const signature = new DiemTypes.Ed25519Signature(signatureTmp);

  const txnAuthenticator = new DiemTypes.TransactionAuthenticatorVariantEd25519(
    new DiemTypes.Ed25519PublicKey(publicKey),
    signature,
  );

  const signedTxn = new DiemTypes.SignedTransaction(rawTxn, txnAuthenticator);
  const signedTxnSerializer = new BcsSerializer();
  signedTxn.serialize(signedTxnSerializer);
  return bufferToHex(signedTxnSerializer.getBytes());
}

export function hexToAccountAddress(hex: string): DiemTypes.AccountAddress {
  if (hex.startsWith("0x")) {
    hex = hex.slice(2);
  }
  const senderListTuple: ListTuple<[uint8]> = [];
  for (const entry of hexToBytes(hex)) { // encode as bytes
    senderListTuple.push([entry]);
  }
  return new DiemTypes.AccountAddress(senderListTuple);
}

// deno-lint-ignore no-explicit-any
export function bufferToHex(buffer: any) {
  return [...new Uint8Array(buffer)]
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function appendBuffer(buffer1: Uint8Array, buffer2: Uint8Array): Uint8Array {
  const tmp = new Uint8Array(buffer1.byteLength + buffer2.byteLength);
  tmp.set(new Uint8Array(buffer1));
  tmp.set(new Uint8Array(buffer2), buffer1.byteLength);
  return tmp;
}

function hexToBytes(hex: string) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i !== bytes.length; i++) {
    bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
  }
  return bytes;
}

export function hexToAscii(hexx: string) {
  const hex = hexx.toString(); // normalize
  let str = "";
  for (let i = 0; i < hex.length; i += 2) {
    str += String.fromCharCode(parseInt(hex.substr(i, 2), 16));
  }
  return str;
}

export function getAddressFromPrivateKey(privateKeyBytes: Uint8Array,): Promise<string> {
  const publicKey = await ed.getPublicKey(privateKeyBytes);
  const address = await publicKey.
}