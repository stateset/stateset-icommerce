/**
 * Named options inside a command's positional arguments.
 *
 * stateset-direct strips its global flags and hands every other argument to
 * the command, so a command that takes an option of its own pulls it out of
 * `args` itself.
 *
 * @module utils/cli-options
 */

/**
 * Take a `--name <value>` or `--name=<value>` option out of positional args.
 * It may appear anywhere, at most once.
 *
 * @param {string[]} args
 * @param {string} name - e.g. '--leaf-count'
 * @param {string} usage - thrown when the option is repeated or has no value
 * @returns {{ value: string|undefined, rest: string[] }}
 */
export function takeOption(args, name, usage) {
  let value;
  const rest = [];
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    let found;
    if (arg === name) {
      if (i + 1 >= args.length) throw new Error(usage);
      found = String(args[++i]);
    } else if (typeof arg === 'string' && arg.startsWith(`${name}=`)) {
      found = arg.slice(name.length + 1);
    } else {
      rest.push(arg);
      continue;
    }
    if (value !== undefined) throw new Error(usage);
    value = found;
  }
  return { value, rest };
}
