import test from 'node:test';
import assert from 'node:assert/strict';
import * as web3 from '@solana/web3.js';
import { createSolanaTokenSdk } from '../../src/chains/solana-token.js';

const sdk = createSolanaTokenSdk(web3);
// Wire values from @solana/spl-token 0.4.14; 200 seeded derivations and
// instructions were also compared against that SDK before removing it.
const mint = new web3.PublicKey('345pmRxihz9AxapL4bzVMzgBXx2GPQLqHbu9wtHMLqLC');
const owner = new web3.PublicKey('Af8LkhPvuiiY4oVAUFJRoNe5hvegK2r2V2S1CRczHLbE');
const payer = new web3.PublicKey('6o1FQZQgK6WShgAn1D1mHCaaKDt8sKkQ3iChEttcjjp');
const expectedAta = 'EJHmtEHeV1QV2ZwgWJ9yVm9Cz6hTy4x2mMPf5N15McYF';

function wire(instruction) {
  return {
    programId: instruction.programId.toBase58(),
    keys: instruction.keys.map(({ pubkey, isSigner, isWritable }) => ({
      pubkey: pubkey.toBase58(),
      isSigner,
      isWritable,
    })),
    data: instruction.data.toString('hex'),
  };
}

test('associated token account and create instruction match the SPL Token wire format', () => {
  const ata = sdk.getAssociatedTokenAddressSync(mint, owner);
  assert.equal(ata.toBase58(), expectedAta);
  assert.deepEqual(wire(sdk.createAssociatedTokenAccountInstruction(payer, ata, owner, mint)), {
    programId: 'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL',
    keys: [
      { pubkey: payer.toBase58(), isSigner: true, isWritable: true },
      { pubkey: expectedAta, isSigner: false, isWritable: true },
      { pubkey: owner.toBase58(), isSigner: false, isWritable: false },
      { pubkey: mint.toBase58(), isSigner: false, isWritable: false },
      { pubkey: '11111111111111111111111111111111', isSigner: false, isWritable: false },
      { pubkey: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA', isSigner: false, isWritable: false },
    ],
    data: '',
  });
  assert.throws(
    () => sdk.createAssociatedTokenAccountInstruction(payer, payer, owner, mint),
    /does not match owner and mint/,
  );
});

test('transfer instruction uses exact unsigned 64-bit token units', () => {
  const ata = new web3.PublicKey(expectedAta);
  assert.deepEqual(wire(sdk.createTransferInstruction(ata, ata, owner, 123456789n)), {
    programId: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',
    keys: [
      { pubkey: expectedAta, isSigner: false, isWritable: true },
      { pubkey: expectedAta, isSigner: false, isWritable: true },
      { pubkey: owner.toBase58(), isSigner: true, isWritable: false },
    ],
    data: '0315cd5b0700000000',
  });
  assert.equal(
    sdk.createTransferInstruction(ata, ata, owner, (1n << 64n) - 1n).data.toString('hex'),
    '03ffffffffffffffff',
  );
  assert.throws(() => sdk.createTransferInstruction(ata, ata, owner, -1n), /unsigned 64-bit/);
  assert.throws(() => sdk.createTransferInstruction(ata, ata, owner, 1n << 64n), /unsigned 64-bit/);
  assert.throws(
    () => sdk.createTransferInstruction(ata, ata, owner, Number.MAX_SAFE_INTEGER + 1),
    /exact integer/,
  );
  assert.throws(
    () => sdk.createTransferInstruction(ata, ata, owner, 1n, [payer]),
    /Multisignature/,
  );
});

test('off-curve wallet owners require an explicit opt-in', () => {
  const offCurve = web3.PublicKey.findProgramAddressSync(
    [Buffer.from('owner')],
    sdk.TOKEN_PROGRAM_ID,
  )[0];
  assert.throws(() => sdk.getAssociatedTokenAddressSync(mint, offCurve), /off curve/);
  assert.doesNotThrow(() => sdk.getAssociatedTokenAddressSync(mint, offCurve, true));
});
