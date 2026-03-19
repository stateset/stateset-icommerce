const BECH32_CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';
const BECH32_GENERATORS = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];

function bech32Polymod(values) {
  let chk = 1;
  for (const value of values) {
    const top = chk >>> 25;
    chk = ((chk & 0x1ffffff) << 5) ^ value;
    for (let i = 0; i < 5; i++) {
      if ((top >>> i) & 1) {
        chk ^= BECH32_GENERATORS[i];
      }
    }
  }
  return chk >>> 0;
}

function bech32HrpExpand(hrp) {
  const out = [];
  for (let i = 0; i < hrp.length; i++) {
    out.push(hrp.charCodeAt(i) >> 5);
  }
  out.push(0);
  for (let i = 0; i < hrp.length; i++) {
    out.push(hrp.charCodeAt(i) & 31);
  }
  return out;
}

function bech32ChecksumConstant(encoding) {
  return encoding === 'bech32m' ? 0x2bc830a3 : 1;
}

function createChecksum(hrp, words, encoding) {
  const values = [...bech32HrpExpand(hrp), ...words, 0, 0, 0, 0, 0, 0];
  const polymod = bech32Polymod(values) ^ bech32ChecksumConstant(encoding);
  const checksum = [];
  for (let i = 0; i < 6; i++) {
    checksum.push((polymod >>> (5 * (5 - i))) & 31);
  }
  return checksum;
}

export function convertBits(data, fromBits, toBits, pad) {
  let acc = 0;
  let bits = 0;
  const maxV = (1 << toBits) - 1;
  const out = [];

  for (const value of data) {
    if (value < 0 || value >> fromBits !== 0) {
      throw new Error('Invalid value for convertBits');
    }
    acc = (acc << fromBits) | value;
    bits += fromBits;
    while (bits >= toBits) {
      bits -= toBits;
      out.push((acc >> bits) & maxV);
    }
  }

  if (pad) {
    if (bits > 0) {
      out.push((acc << (toBits - bits)) & maxV);
    }
  } else if (bits >= fromBits || ((acc << (toBits - bits)) & maxV) !== 0) {
    throw new Error('Invalid incomplete group in convertBits');
  }

  return out;
}

export function decodeBech32(address) {
  const normalized = address.toLowerCase();
  if (normalized !== address && address.toUpperCase() !== address) {
    throw new Error(`Mixed-case Bech32 address is invalid: ${address}`);
  }

  const separator = normalized.lastIndexOf('1');
  if (separator <= 0 || separator + 7 > normalized.length) {
    throw new Error(`Invalid Bech32 address: ${address}`);
  }

  const hrp = normalized.slice(0, separator);
  const data = normalized.slice(separator + 1);
  const words = [];
  for (const char of data) {
    const value = BECH32_CHARSET.indexOf(char);
    if (value === -1) {
      throw new Error(`Invalid Bech32 character "${char}" in ${address}`);
    }
    words.push(value);
  }

  const checksum = bech32Polymod([...bech32HrpExpand(hrp), ...words]);
  let encoding;
  if (checksum === 1) {
    encoding = 'bech32';
  } else if (checksum === 0x2bc830a3) {
    encoding = 'bech32m';
  } else {
    throw new Error(`Invalid Bech32 checksum for ${address}`);
  }

  return {
    hrp,
    words: words.slice(0, -6),
    encoding,
  };
}

export function encodeBech32(hrp, words, encoding = 'bech32') {
  const checksum = createChecksum(hrp, words, encoding);
  const combined = [...words, ...checksum];
  let encoded = `${hrp}1`;
  for (const value of combined) {
    encoded += BECH32_CHARSET[value];
  }
  return encoded;
}

export function decodeSegwitAddress(address) {
  const decoded = decodeBech32(address);
  const version = decoded.words[0];
  if (version === undefined) {
    throw new Error(`Invalid SegWit address: ${address}`);
  }

  const program = Buffer.from(convertBits(decoded.words.slice(1), 5, 8, false));
  const expectedEncoding = version === 0 ? 'bech32' : 'bech32m';
  if (decoded.encoding !== expectedEncoding) {
    throw new Error(`Invalid witness encoding for ${address}`);
  }

  return {
    hrp: decoded.hrp,
    version,
    program,
    encoding: decoded.encoding,
  };
}

export function encodeSegwitAddress(hrp, version, program) {
  const normalizedProgram = Buffer.isBuffer(program) ? program : Buffer.from(program);
  const encoding = version === 0 ? 'bech32' : 'bech32m';
  const words = [version, ...convertBits([...normalizedProgram], 8, 5, true)];
  return encodeBech32(hrp, words, encoding);
}

export default {
  convertBits,
  decodeBech32,
  encodeBech32,
  decodeSegwitAddress,
  encodeSegwitAddress,
};
