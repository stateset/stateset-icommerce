function flatten(values) {
  return values.flatMap((value) => {
    if (Array.isArray(value)) {
      return flatten(value);
    }
    return [value];
  });
}

export function cn(...inputs) {
  return flatten(inputs).filter(Boolean).join(' ');
}
