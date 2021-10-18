// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

import * as DiemHelpers from "./helpers.ts";
import * as DiemTypes from "./generated/diemTypes/mod.ts";
import * as main from "./generated/diemStdlib/mod.ts";
import * as path from "https://deno.land/std@0.110.0/path/mod.ts";
import * as util from "https://deno.land/std@0.85.0/node/util.ts";
import { createRemote } from "https://deno.land/x/gentle_rpc@v3.0/mod.ts";

const textEncoder = new util.TextEncoder();
export const shuffleDir = Deno.env.get("SHUFFLE_HOME") || "unknown";
const privateKeyPath = path.join(shuffleDir, "accounts/latest/dev.key");
const senderAddressPath = path.join(shuffleDir, "accounts/latest/address");
const senderAddress = await Deno.readTextFile(senderAddressPath);
export const fullSenderAddress = "0x" + senderAddress;
const receiverPrivateKeyPath = path.join(shuffleDir, "accounts/testkeys/receiver.key");
let receiverPrivateKeyBytes = await Deno.readFile(receiverPrivateKeyPath);
const receiverAddressPath = path.join(shuffleDir, "accounts/testkeys/address");
const receiverAddress = await Deno.readTextFile(receiverAddressPath);
export const fullSenderAddress = "0x" + receiverAddress;

// Client side creation and signing of transactions.
// https://github.com/diem/diem/blob/main/json-rpc/docs/method_submit.md#method-submit
export async function setMessage(message: string, sequenceNumber: number) {
  if (sequenceNumber == undefined) {
    console.log(
      "Must pass in parameters: message, sequenceNumber. Try Shuffle.sequenceNumber()",
    );
    return;
  }

  const [rawTxn, signingMsg] = newRawTransactionAndSigningMsg(
    message,
    sequenceNumber,
  );
  const signedTxnHex = await newSignedTransaction(rawTxn, signingMsg);

  const remote = createRemote("http://127.0.0.1:8080/v1");
  return await remote.call("submit", [signedTxnHex]);
}

function newRawTransactionAndSigningMsg(
  message: string,
  sequenceNumber: number,
): [DiemTypes.RawTransaction, Uint8Array] {
  const rawTxn = setMessageRawTransaction(
    fullSenderAddress,
    message,
    sequenceNumber,
  );

  return [
    rawTxn,
    DiemHelpers.generateSigningMessage(rawTxn),
  ];
}

async function newSignedTransaction(
  rawTxn: DiemTypes.RawTransaction,
  signingMsg: Uint8Array,
): Promise<string> {
  let privateKeyBytes = await Deno.readFile(privateKeyPath);

  // slice off first BIP type byte, rest of 32 bytes is private key
  privateKeyBytes = privateKeyBytes.slice(1);
  return DiemHelpers.newSignedTransaction(
    privateKeyBytes,
    rawTxn,
    signingMsg,
  );
}

export function setMessageTransactionPayload(
  message: string,
): DiemTypes.TransactionPayloadVariantScript {
  const script = main.Stdlib.encodeSetMessageScript(
    textEncoder.encode(message),
  );
  return new DiemTypes.TransactionPayloadVariantScript(script);
}

// senderStr example 0x24163afcc6e33b0a9473852e18327fa9
export function setMessageRawTransaction(
  senderStr: string,
  message: string,
  sequenceNumber: number,
): DiemTypes.RawTransaction {
  const payload = setMessageTransactionPayload(message);
  return DiemHelpers.newRawTransaction(
    senderStr,
    payload,
    sequenceNumber,
  );
}

export function messagesFrom(resources: any[]) {
  return resources
    .filter(
      (entry) => entry["type"]["name"] == "MessageHolder",
    );
}

export function decodedMessages(resources: any[]) {
  return messagesFrom(resources)
    .map((entry) => DiemHelpers.hexToAscii(entry.value.message));
}
