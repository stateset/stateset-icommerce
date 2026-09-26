/**
 * The small, fixed SPL Token instruction surface used by StateSet.
 *
 * Keeping these wire encodings local avoids the unpatched native bigint-buffer
 * dependency pulled in by @solana/spl-token. The associated account and token
 * program IDs and instruction layouts are defined by the on-chain programs.
 */
const TOKEN_PROGRAM_ADDRESS = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';
const ASSOCIATED_TOKEN_PROGRAM_ADDRESS = 'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL';
const MAX_U64 = (1n << 64n) - 1n;

export function createSolanaTokenSdk(web3) {
  const TOKEN_PROGRAM_ID = new web3.PublicKey(TOKEN_PROGRAM_ADDRESS);
  const ASSOCIATED_TOKEN_PROGRAM_ID = new web3.PublicKey(ASSOCIATED_TOKEN_PROGRAM_ADDRESS);

  function getAssociatedTokenAddressSync(
    mint,
    owner,
    allowOwnerOffCurve = false,
    programId = TOKEN_PROGRAM_ID,
    associatedTokenProgramId = ASSOCIATED_TOKEN_PROGRAM_ID,
  ) {
    if (!allowOwnerOffCurve && !web3.PublicKey.isOnCurve(owner.toBuffer())) {
      throw new Error('Associated token account owner is off curve');
    }
    return web3.PublicKey.findProgramAddressSync(
      [owner.toBuffer(), programId.toBuffer(), mint.toBuffer()],
      associatedTokenProgramId,
    )[0];
  }

  function createAssociatedTokenAccountInstruction(
    payer,
    associatedToken,
    owner,
    mint,
    programId = TOKEN_PROGRAM_ID,
    associatedTokenProgramId = ASSOCIATED_TOKEN_PROGRAM_ID,
  ) {
    const expected = getAssociatedTokenAddressSync(
      mint,
      owner,
      true,
      programId,
      associatedTokenProgramId,
    );
    if (!associatedToken.equals(expected)) {
      throw new Error('Associated token account address does not match owner and mint');
    }
    return new web3.TransactionInstruction({
      programId: associatedTokenProgramId,
      keys: [
        { pubkey: payer, isSigner: true, isWritable: true },
        { pubkey: associatedToken, isSigner: false, isWritable: true },
        { pubkey: owner, isSigner: false, isWritable: false },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: web3.SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: programId, isSigner: false, isWritable: false },
      ],
      data: Buffer.alloc(0),
    });
  }

  function createTransferInstruction(
    source,
    destination,
    owner,
    amount,
    multiSigners = [],
    programId = TOKEN_PROGRAM_ID,
  ) {
    if (multiSigners.length !== 0) {
      throw new Error('Multisignature SPL Token transfers are not supported by this adapter');
    }
    if (typeof amount === 'number' && !Number.isSafeInteger(amount)) {
      throw new RangeError('SPL Token transfer amount must be an exact integer');
    }
    const units = BigInt(amount);
    if (units < 0n || units > MAX_U64) {
      throw new RangeError('SPL Token transfer amount must fit unsigned 64-bit units');
    }
    const data = Buffer.alloc(9);
    data[0] = 3; // Token Program Transfer instruction discriminator.
    data.writeBigUInt64LE(units, 1);
    return new web3.TransactionInstruction({
      programId,
      keys: [
        { pubkey: source, isSigner: false, isWritable: true },
        { pubkey: destination, isSigner: false, isWritable: true },
        { pubkey: owner, isSigner: true, isWritable: false },
      ],
      data,
    });
  }

  return {
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    getAssociatedTokenAddressSync,
    createAssociatedTokenAccountInstruction,
    createTransferInstruction,
  };
}
