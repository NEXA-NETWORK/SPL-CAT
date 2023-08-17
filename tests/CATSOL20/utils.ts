import {
  PublicKey,
  PublicKeyInitData,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  SystemProgram,
} from "@solana/web3.js";

interface PostMessageAccounts {
  bridge: PublicKey;
  message: PublicKey;
  emitter: PublicKey;
  sequence: PublicKey;
  feeCollector: PublicKey;
  clock: PublicKey;
  rent: PublicKey;
  systemProgram: PublicKey;
}

interface EmitterAccounts {
  emitter: PublicKey;
  sequence: PublicKey;
}

function deriveWormholeLiteralKey(
  literal: string,
  wormholeProgramId: PublicKeyInitData
): PublicKey {
  const Key = PublicKey.findProgramAddressSync([Buffer.from(literal)], new PublicKey(wormholeProgramId));
  return Key[0];
}

export function deserializeSequenceTracker(data: Buffer): bigint {
  if (data.length != 8) {
    throw new Error("data.length != 8");
  }
  return data.readBigUInt64LE(0);
}

export function getEmitterKeys(
  emitterProgramId: PublicKeyInitData,
  wormholeProgramId: PublicKeyInitData,
  tokenPDA: PublicKey
): EmitterAccounts {
  const emitter = PublicKey.findProgramAddressSync(
    [Buffer.from("emitter"), tokenPDA.toBuffer()],
    new PublicKey(emitterProgramId)
  )[0];

  const sequence = PublicKey.findProgramAddressSync(
    [Buffer.from("Sequence"), new PublicKey(emitter).toBytes()],
    new PublicKey(wormholeProgramId)
  )[0];

  return {
    emitter,
    sequence,
  };
}

export function getPostMessageAccounts(
  wormholeProgramId: PublicKeyInitData,
  emitterProgramId: PublicKeyInitData,
  tokenPDA: PublicKey,
  sequenceNumber: number
): PostMessageAccounts {

  const { emitter, sequence } = getEmitterKeys(emitterProgramId, wormholeProgramId, tokenPDA);

  // Initial Sequence is 1
  const sequenceBuffer = Buffer.alloc(8);
  sequenceBuffer.writeBigUint64LE(BigInt(sequenceNumber));

  const message = PublicKey.findProgramAddressSync(
    [Buffer.from("sent"), new PublicKey(emitter).toBytes(), sequenceBuffer],
    new PublicKey(emitterProgramId)
  )[0];

  return {
    bridge: deriveWormholeLiteralKey("Bridge", wormholeProgramId),
    message: new PublicKey(message),
    emitter,
    sequence,
    feeCollector: deriveWormholeLiteralKey("fee_collector", wormholeProgramId),
    clock: SYSVAR_CLOCK_PUBKEY,
    rent: SYSVAR_RENT_PUBKEY,
    systemProgram: SystemProgram.programId,
  };
}