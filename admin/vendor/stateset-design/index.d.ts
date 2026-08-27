import type * as React from 'react';

/** Merge class names (filtered, flattened). */
export function cn(...inputs: Array<unknown>): string;

// ── Button ───────────────────────────────────────────────────────────────────
export type ButtonVariant = 'primary' | 'secondary' | 'accent' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'normal' | 'lg' | 'big';
export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    Omit<React.AnchorHTMLAttributes<HTMLAnchorElement>, keyof React.ButtonHTMLAttributes<HTMLButtonElement>> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Render as an anchor when set. */
  href?: string;
}
export const Button: React.FC<ButtonProps>;

// ── Badge ────────────────────────────────────────────────────────────────────
export type BadgeVariant =
  | 'default'
  | 'primary'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'outline';
export interface BadgeProps extends React.HTMLAttributes<HTMLElement> {
  variant?: BadgeVariant;
  as?: React.ElementType;
}
export const Badge: React.FC<BadgeProps>;

// ── StatusPill ───────────────────────────────────────────────────────────────
export type StatusTone = 'ok' | 'run' | 'warn' | 'fail' | 'review' | 'idle';
export interface StatusPillProps extends React.HTMLAttributes<HTMLSpanElement> {
  status?: StatusTone;
  pulse?: boolean;
}
export const StatusPill: React.FC<StatusPillProps>;

// ── Card ─────────────────────────────────────────────────────────────────────
export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  interactive?: boolean;
  premium?: boolean;
}
export const Card: React.FC<CardProps>;
export const CardHeader: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export interface CardTitleProps extends React.HTMLAttributes<HTMLHeadingElement> {
  as?: React.ElementType;
}
export const CardTitle: React.FC<CardTitleProps>;
export const CardDescription: React.FC<React.HTMLAttributes<HTMLParagraphElement>>;
export const CardContent: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const CardFooter: React.FC<React.HTMLAttributes<HTMLDivElement>>;

// ── Input ────────────────────────────────────────────────────────────────────
export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}
export const Input: React.FC<InputProps>;

// ── Metrics ──────────────────────────────────────────────────────────────────
export type MetricTone = 'primary' | 'accent' | 'success' | 'warning' | 'danger';
export interface MetricCardProps extends React.HTMLAttributes<HTMLDivElement> {
  label: string;
  value: React.ReactNode;
  subtitle?: string;
  trend?: string;
  icon?: React.ComponentType<{ className?: string }>;
  tone?: MetricTone;
  format?: '' | 'currency' | 'number';
}
export const MetricCard: React.FC<MetricCardProps>;
export interface MetricStripItem {
  label: string;
  value: React.ReactNode;
  detail?: string;
}
export interface MetricStripProps {
  items: MetricStripItem[];
  className?: string;
}
export const MetricStrip: React.FC<MetricStripProps>;

// ── AgentActionStream ────────────────────────────────────────────────────────
export interface AgentActionStreamProps extends React.HTMLAttributes<HTMLDivElement> {
  items?: Array<Record<string, unknown>>;
}
export const AgentActionStream: React.FC<AgentActionStreamProps>;

// ── EmptyState ───────────────────────────────────────────────────────────────
export interface EmptyStateAction extends Partial<ButtonProps> {
  label: string;
}
export interface EmptyStateProps {
  icon?: React.ComponentType<{ className?: string }>;
  eyebrow?: string;
  title: React.ReactNode;
  description?: React.ReactNode;
  action?: EmptyStateAction;
  secondaryAction?: EmptyStateAction;
  className?: string;
}
export const EmptyState: React.FC<EmptyStateProps>;

// ── Table ────────────────────────────────────────────────────────────────────
export const Table: React.FC<React.TableHTMLAttributes<HTMLTableElement>>;
export const TableHeader: React.FC<React.HTMLAttributes<HTMLTableSectionElement>>;
export const TableBody: React.FC<React.HTMLAttributes<HTMLTableSectionElement>>;
export const TableRow: React.FC<React.HTMLAttributes<HTMLTableRowElement>>;
export const TableHead: React.FC<React.ThHTMLAttributes<HTMLTableCellElement>>;
export interface TableCellProps extends React.TdHTMLAttributes<HTMLTableCellElement> {
  tone?: 'default' | 'numeric';
}
export const TableCell: React.FC<TableCellProps>;

// ── Layout / navigation ──────────────────────────────────────────────────────
export const DashboardShell: React.FC<React.HTMLAttributes<HTMLDivElement> & Record<string, unknown>>;
export const DashboardSidebarSection: React.FC<React.HTMLAttributes<HTMLDivElement> & Record<string, unknown>>;
export const DashboardSectionHeader: React.FC<React.HTMLAttributes<HTMLDivElement> & Record<string, unknown>>;
export const SidebarNavItem: React.FC<Record<string, unknown>>;

export interface NavItem {
  label: string;
  href?: string;
}
export interface TopNavProps extends React.HTMLAttributes<HTMLElement> {
  brand?: string;
  brandHref?: string;
  items?: NavItem[];
  cta?: { label: string; href?: string } | null;
  sticky?: boolean;
}
export const TopNav: React.FC<TopNavProps>;

export interface FooterColumn {
  title: string;
  links: NavItem[];
}
export interface FooterProps extends React.HTMLAttributes<HTMLElement> {
  brand?: string;
  tagline?: string;
  columns?: FooterColumn[];
  legal?: string;
}
export const Footer: React.FC<FooterProps>;

export interface BannerProps extends React.HTMLAttributes<HTMLElement> {
  tone?: 'brand' | 'subtle';
  kicker?: string;
  title: React.ReactNode;
  description?: React.ReactNode;
}
export const Banner: React.FC<BannerProps>;
export interface BannerSubscribeProps {
  placeholder?: string;
  action?: string;
  onSubmit?: (event: React.FormEvent<HTMLFormElement>) => void;
  className?: string;
}
export const BannerSubscribe: React.FC<BannerSubscribeProps>;

// ── Tabs ─────────────────────────────────────────────────────────────────────
export const Tabs: React.FC<Record<string, unknown>>;
export const TabsList: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export interface TabsTriggerProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  value: string;
}
export const TabsTrigger: React.FC<TabsTriggerProps>;
export interface TabsContentProps extends React.HTMLAttributes<HTMLDivElement> {
  value: string;
}
export const TabsContent: React.FC<TabsContentProps>;

// ── Select ───────────────────────────────────────────────────────────────────
export const Select: React.FC<Record<string, unknown>>;
export const SelectTrigger: React.FC<React.ButtonHTMLAttributes<HTMLButtonElement>>;
export interface SelectValueProps {
  placeholder?: string;
  children?: React.ReactNode;
}
export const SelectValue: React.FC<SelectValueProps>;
export const SelectContent: React.FC<React.HTMLAttributes<HTMLDivElement> & { position?: string }>;
export interface SelectItemProps extends React.HTMLAttributes<HTMLDivElement> {
  value: string;
  disabled?: boolean;
}
export const SelectItem: React.FC<SelectItemProps>;
export const SelectGroup: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const SelectLabel: React.FC<React.HTMLAttributes<HTMLDivElement>>;

// ── Toggles ──────────────────────────────────────────────────────────────────
export interface SwitchProps extends React.HTMLAttributes<HTMLButtonElement> {
  checked?: boolean;
  defaultChecked?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  disabled?: boolean;
  name?: string;
  value?: string;
}
export const Switch: React.FC<SwitchProps>;
export interface CheckboxProps extends Omit<React.HTMLAttributes<HTMLButtonElement>, 'onChange'> {
  checked?: boolean | 'indeterminate';
  defaultChecked?: boolean;
  onCheckedChange?: (checked: boolean | 'indeterminate') => void;
  disabled?: boolean;
  name?: string;
  value?: string;
}
export const Checkbox: React.FC<CheckboxProps>;

// ── Avatar / Separator / Spinner / Skeleton ──────────────────────────────────
export const Avatar: React.FC<React.HTMLAttributes<HTMLSpanElement>>;
export interface AvatarImageProps extends React.ImgHTMLAttributes<HTMLImageElement> {
  onLoadingStatusChange?: (status: 'idle' | 'loading' | 'loaded' | 'error') => void;
}
export const AvatarImage: React.FC<AvatarImageProps>;
export const AvatarFallback: React.FC<React.HTMLAttributes<HTMLSpanElement> & { delayMs?: number }>;
export interface SeparatorProps extends React.HTMLAttributes<HTMLDivElement> {
  orientation?: 'horizontal' | 'vertical';
  decorative?: boolean;
}
export const Separator: React.FC<SeparatorProps>;
export interface SpinnerProps extends React.SVGAttributes<SVGSVGElement> {
  size?: 'sm' | 'md' | 'lg';
  label?: string;
}
export const Spinner: React.FC<SpinnerProps>;
export const Skeleton: React.FC<React.HTMLAttributes<HTMLDivElement>>;

// ── Dialog ───────────────────────────────────────────────────────────────────
export interface DialogRootProps {
  open?: boolean;
  defaultOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
  modal?: boolean;
  children?: React.ReactNode;
}
export const Dialog: React.FC<DialogRootProps>;
export const DialogTrigger: React.FC<React.ButtonHTMLAttributes<HTMLButtonElement> & { asChild?: boolean }>;
export const DialogContent: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const DialogHeader: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const DialogTitle: React.FC<React.HTMLAttributes<HTMLHeadingElement>>;
export const DialogDescription: React.FC<React.HTMLAttributes<HTMLParagraphElement>>;
export const DialogFooter: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const DialogClose: React.FC<React.ButtonHTMLAttributes<HTMLButtonElement> & { asChild?: boolean }>;

// ── DropdownMenu ─────────────────────────────────────────────────────────────
export const DropdownMenu: React.FC<DialogRootProps>;
export const DropdownMenuTrigger: React.FC<React.ButtonHTMLAttributes<HTMLButtonElement> & { asChild?: boolean }>;
export const DropdownMenuContent: React.FC<React.HTMLAttributes<HTMLDivElement> & { sideOffset?: number; align?: string; side?: string }>;
export const DropdownMenuItem: React.FC<React.HTMLAttributes<HTMLDivElement> & { disabled?: boolean; onSelect?: (event: Event) => void }>;
export const DropdownMenuSeparator: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const DropdownMenuLabel: React.FC<React.HTMLAttributes<HTMLDivElement>>;

// ── Tooltip ──────────────────────────────────────────────────────────────────
export const TooltipProvider: React.FC<{ delayDuration?: number; skipDelayDuration?: number; disableHoverableContent?: boolean; children?: React.ReactNode }>;
export const Tooltip: React.FC<DialogRootProps & { delayDuration?: number }>;
export const TooltipTrigger: React.FC<React.ButtonHTMLAttributes<HTMLButtonElement> & { asChild?: boolean }>;
export const TooltipContent: React.FC<React.HTMLAttributes<HTMLDivElement> & { sideOffset?: number; side?: string; align?: string }>;

// ── Toast ────────────────────────────────────────────────────────────────────
export const ToastProvider: React.FC<{ duration?: number; label?: string; swipeDirection?: string; swipeThreshold?: number; children?: React.ReactNode }>;
export const ToastViewport: React.FC<React.HTMLAttributes<HTMLOListElement> & { hotkey?: string[]; label?: string }>;
export const Toast: React.FC<React.HTMLAttributes<HTMLLIElement> & { open?: boolean; defaultOpen?: boolean; onOpenChange?: (open: boolean) => void; duration?: number; type?: 'foreground' | 'background' }>;
export const ToastTitle: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const ToastDescription: React.FC<React.HTMLAttributes<HTMLDivElement>>;
export const ToastClose: React.FC<React.ButtonHTMLAttributes<HTMLButtonElement>>;
export const ToastAction: React.FC<React.ButtonHTMLAttributes<HTMLButtonElement> & { altText: string }>;

// ── Motion ───────────────────────────────────────────────────────────────────
/** Animation durations in milliseconds. */
export const DURATION: { fast: number; base: number; slow: number; page: number };
/** CSS cubic-bezier easing strings. */
export const EASING: { standard: string; emphasized: string; exit: string };
/** A framer-motion-shaped transition preset (seconds + cubic-bezier tuple). */
export interface TransitionToken {
  duration: number;
  ease: [number, number, number, number];
}
export const TRANSITION: {
  fast: TransitionToken;
  standard: TransitionToken;
  slow: TransitionToken;
  emphasized: TransitionToken;
};
export function usePrefersReducedMotion(): boolean;
export interface RevealProps extends React.HTMLAttributes<HTMLDivElement> {
  as?: React.ElementType;
  delay?: number;
}
export const Reveal: React.FC<RevealProps>;
