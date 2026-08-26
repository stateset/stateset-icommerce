import React from 'react';

import { cn } from '../utils/cn.js';

export function Input({
  label = '',
  id = '',
  name = '',
  error = '',
  className = '',
  inputClassName = '',
  ...props
}) {
  const inputId = id || name;
  const errorId = error && inputId ? `${inputId}-error` : undefined;

  return (
    <div className={cn('w-full space-y-2', className)}>
      {label ? (
        <label
          htmlFor={inputId}
          className="block text-[11px] font-semibold uppercase tracking-[0.16em] text-ds-muted-foreground">
          {label}
        </label>
      ) : null}
      <input
        id={inputId}
        name={name}
        aria-invalid={error ? 'true' : undefined}
        aria-describedby={errorId}
        className={cn(
          'block h-11 w-full rounded-lg border border-ds-input bg-ds-card px-3.5 text-sm text-ds-foreground shadow-ds-card transition-all duration-150 placeholder:text-ds-muted-foreground/80 focus:border-ds-ring focus:outline-none focus:ring-2 focus:ring-ds-ring/20',
          inputClassName,
        )}
        {...props}
      />
      {error ? (
        <p id={errorId} className="text-sm text-ds-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

export default Input;
