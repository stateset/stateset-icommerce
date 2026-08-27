module.exports = {
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        'ds-border': 'hsl(var(--ds-border) / <alpha-value>)',
        'ds-input': 'hsl(var(--ds-input) / <alpha-value>)',
        'ds-ring': 'hsl(var(--ds-ring) / <alpha-value>)',
        'ds-background': 'hsl(var(--ds-background) / <alpha-value>)',
        'ds-foreground': 'hsl(var(--ds-foreground) / <alpha-value>)',
        'ds-card': {
          DEFAULT: 'hsl(var(--ds-card) / <alpha-value>)',
          foreground: 'hsl(var(--ds-card-foreground) / <alpha-value>)',
        },
        'ds-popover': {
          DEFAULT: 'hsl(var(--ds-popover) / <alpha-value>)',
          foreground: 'hsl(var(--ds-popover-foreground) / <alpha-value>)',
        },
        'ds-primary': {
          DEFAULT: 'hsl(var(--ds-primary) / <alpha-value>)',
          foreground: 'hsl(var(--ds-primary-foreground) / <alpha-value>)',
        },
        'ds-secondary': {
          DEFAULT: 'hsl(var(--ds-secondary) / <alpha-value>)',
          foreground: 'hsl(var(--ds-secondary-foreground) / <alpha-value>)',
        },
        'ds-muted': {
          DEFAULT: 'hsl(var(--ds-muted) / <alpha-value>)',
          foreground: 'hsl(var(--ds-muted-foreground) / <alpha-value>)',
        },
        'ds-accent': {
          DEFAULT: 'hsl(var(--ds-accent) / <alpha-value>)',
          foreground: 'hsl(var(--ds-accent-foreground) / <alpha-value>)',
        },
        'ds-destructive': {
          DEFAULT: 'hsl(var(--ds-destructive) / <alpha-value>)',
          foreground: 'hsl(var(--ds-destructive-foreground) / <alpha-value>)',
        },
        'ds-success': {
          DEFAULT: 'hsl(var(--ds-success) / <alpha-value>)',
          foreground: 'hsl(var(--ds-success-foreground) / <alpha-value>)',
        },
        'ds-warning': {
          DEFAULT: 'hsl(var(--ds-warning) / <alpha-value>)',
          foreground: 'hsl(var(--ds-warning-foreground) / <alpha-value>)',
        },
        'ds-info': {
          DEFAULT: 'hsl(var(--ds-info) / <alpha-value>)',
          foreground: 'hsl(var(--ds-info-foreground) / <alpha-value>)',
        },
        'ds-sidebar': {
          DEFAULT: 'hsl(var(--ds-sidebar-bg) / <alpha-value>)',
          foreground: 'hsl(var(--ds-sidebar-foreground) / <alpha-value>)',
          border: 'hsl(var(--ds-sidebar-border) / <alpha-value>)',
          accent: 'hsl(var(--ds-sidebar-accent) / <alpha-value>)',
          'accent-foreground': 'hsl(var(--ds-sidebar-accent-foreground) / <alpha-value>)',
        },
        'ds-brand': {
          50: 'hsl(var(--ds-brand-50) / <alpha-value>)',
          100: 'hsl(var(--ds-brand-100) / <alpha-value>)',
          200: 'hsl(var(--ds-brand-200) / <alpha-value>)',
          300: 'hsl(var(--ds-brand-300) / <alpha-value>)',
          400: 'hsl(var(--ds-brand-400) / <alpha-value>)',
          500: 'hsl(var(--ds-brand-500) / <alpha-value>)',
          600: 'hsl(var(--ds-brand-600) / <alpha-value>)',
          700: 'hsl(var(--ds-brand-700) / <alpha-value>)',
          800: 'hsl(var(--ds-brand-800) / <alpha-value>)',
          900: 'hsl(var(--ds-brand-900) / <alpha-value>)',
          950: 'hsl(var(--ds-brand-950) / <alpha-value>)',
        },
        'ds-coral': {
          50: 'hsl(var(--ds-coral-50) / <alpha-value>)',
          100: 'hsl(var(--ds-coral-100) / <alpha-value>)',
          200: 'hsl(var(--ds-coral-200) / <alpha-value>)',
          300: 'hsl(var(--ds-coral-300) / <alpha-value>)',
          400: 'hsl(var(--ds-coral-400) / <alpha-value>)',
          500: 'hsl(var(--ds-coral-500) / <alpha-value>)',
          600: 'hsl(var(--ds-coral-600) / <alpha-value>)',
          700: 'hsl(var(--ds-coral-700) / <alpha-value>)',
          800: 'hsl(var(--ds-coral-800) / <alpha-value>)',
          900: 'hsl(var(--ds-coral-900) / <alpha-value>)',
          950: 'hsl(var(--ds-coral-950) / <alpha-value>)',
        },
        'ds-chart': {
          1: 'hsl(var(--ds-chart-1) / <alpha-value>)',
          2: 'hsl(var(--ds-chart-2) / <alpha-value>)',
          3: 'hsl(var(--ds-chart-3) / <alpha-value>)',
          4: 'hsl(var(--ds-chart-4) / <alpha-value>)',
          5: 'hsl(var(--ds-chart-5) / <alpha-value>)',
          grid: 'hsl(var(--ds-chart-grid) / <alpha-value>)',
        },
        'ds-enterprise': {
          canvas: 'hsl(var(--ds-enterprise-canvas) / <alpha-value>)',
          surface: 'hsl(var(--ds-enterprise-surface) / <alpha-value>)',
          raised: 'hsl(var(--ds-enterprise-surface-raised) / <alpha-value>)',
          line: 'hsl(var(--ds-enterprise-line) / <alpha-value>)',
          'line-strong': 'hsl(var(--ds-enterprise-line-strong) / <alpha-value>)',
          header: 'hsl(var(--ds-enterprise-header) / <alpha-value>)',
          'header-foreground': 'hsl(var(--ds-enterprise-header-foreground) / <alpha-value>)',
          focus: 'hsl(var(--ds-enterprise-focus) / <alpha-value>)',
        },
        'ds-status': {
          ok: 'hsl(var(--ds-status-ok) / <alpha-value>)',
          run: 'hsl(var(--ds-status-run) / <alpha-value>)',
          warn: 'hsl(var(--ds-status-warn) / <alpha-value>)',
          fail: 'hsl(var(--ds-status-fail) / <alpha-value>)',
          review: 'hsl(var(--ds-status-review) / <alpha-value>)',
          idle: 'hsl(var(--ds-status-idle) / <alpha-value>)',
        },
        'ds-agent': {
          DEFAULT: 'hsl(var(--ds-agent) / <alpha-value>)',
          foreground: 'hsl(var(--ds-agent-foreground) / <alpha-value>)',
        },
      },
      fontFamily: {
        'ds-sans': ['var(--ds-font-sans)'],
        'ds-display': ['var(--ds-font-display)'],
        'ds-mono': ['var(--ds-font-mono)'],
      },
      letterSpacing: {
        'ds-display': 'var(--ds-tracking-display)',
        'ds-heading': 'var(--ds-tracking-heading)',
        'ds-tight': 'var(--ds-tracking-tight)',
        'ds-normal': 'var(--ds-tracking-normal)',
        'ds-kicker': 'var(--ds-tracking-kicker)',
      },
      boxShadow: {
        'ds-card': 'var(--ds-shadow-card)',
        'ds-card-hover': 'var(--ds-shadow-card-hover)',
        'ds-panel': 'var(--ds-shadow-panel)',
        'ds-focus': 'var(--ds-shadow-focus)',
      },
      backgroundImage: {
        'ds-canvas': 'var(--ds-gradient-canvas)',
        'ds-panel': 'var(--ds-gradient-panel)',
        'ds-accent': 'var(--ds-gradient-accent)',
      },
      maxWidth: {
        'ds-shell': '96rem',
      },
      width: {
        'ds-sidebar': 'var(--ds-sidebar-width)',
      },
      spacing: {
        'ds-compact': 'var(--ds-density-compact)',
        'ds-comfortable': 'var(--ds-density-comfortable)',
        'ds-spacious': 'var(--ds-density-spacious)',
      },
      keyframes: {
        'fade-up': {
          from: { opacity: '0', transform: 'translateY(8px)' },
          to: { opacity: '1', transform: 'translateY(0)' },
        },
        'soft-pulse': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.72' },
        },
      },
      animation: {
        'ds-fade-up': 'fade-up 180ms ease-out',
        'ds-soft-pulse': 'soft-pulse 2.4s ease-in-out infinite',
      },
    },
  },
};
