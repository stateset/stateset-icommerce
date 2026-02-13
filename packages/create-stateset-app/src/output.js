const isTTY = process.stdout.isTTY;

const code = (n) => (isTTY ? `\x1b[${n}m` : '');
const reset = code(0);

export const bold = (s) => `${code(1)}${s}${reset}`;
export const dim = (s) => `${code(2)}${s}${reset}`;
export const cyan = (s) => `${code(36)}${s}${reset}`;
export const green = (s) => `${code(32)}${s}${reset}`;
export const red = (s) => `${code(31)}${s}${reset}`;
export const yellow = (s) => `${code(33)}${s}${reset}`;

export const print = (s) => console.log(s);
export const info = (s) => console.log(cyan(s));
export const success = (s) => console.log(green(s));
export const error = (s) => console.error(red(s));
