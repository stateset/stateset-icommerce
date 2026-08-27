export { Button } from './src/primitives/button.jsx';
export { Badge } from './src/primitives/badge.jsx';
export { StatusPill } from './src/primitives/status-pill.jsx';
export {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from './src/primitives/card.jsx';
export { Input } from './src/primitives/input.jsx';
export { MetricCard } from './src/primitives/metric-card.jsx';
export { MetricStrip } from './src/primitives/metric-strip.jsx';
export { AgentActionStream } from './src/primitives/agent-action-stream.jsx';
export { EmptyState } from './src/primitives/empty-state.jsx';
export {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from './src/primitives/table.jsx';
export {
  DashboardShell,
  DashboardSidebarSection,
  DashboardSectionHeader,
} from './src/layouts/dashboard-shell.jsx';
export { SidebarNavItem } from './src/navigation/sidebar-nav-item.jsx';
export { TopNav } from './src/navigation/top-nav.jsx';

// Marketing surfaces (from the brand's TopNav / Footer / Banner components)
export { Footer } from './src/marketing/footer.jsx';
export { Banner, BannerSubscribe } from './src/marketing/banner.jsx';

// Form & display primitives
export { Tabs, TabsList, TabsTrigger, TabsContent } from './src/primitives/tabs.jsx';
export {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
  SelectGroup,
  SelectLabel,
} from './src/primitives/select.jsx';
export { Switch } from './src/primitives/switch.jsx';
export { Checkbox } from './src/primitives/checkbox.jsx';
export { Avatar, AvatarImage, AvatarFallback } from './src/primitives/avatar.jsx';
export { Separator } from './src/primitives/separator.jsx';
export { Spinner } from './src/primitives/spinner.jsx';
export { Skeleton } from './src/primitives/skeleton.jsx';

// Overlays
export {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from './src/overlays/dialog.jsx';
export {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuLabel,
} from './src/overlays/dropdown-menu.jsx';
export {
  TooltipProvider,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from './src/overlays/tooltip.jsx';

// Feedback
export {
  ToastProvider,
  ToastViewport,
  Toast,
  ToastTitle,
  ToastDescription,
  ToastClose,
  ToastAction,
} from './src/feedback/toast.jsx';

// Motion vocabulary — standardized timing + the canonical scroll-reveal entrance.
export { DURATION, EASING, TRANSITION } from './src/motion/motion-tokens.js';
export { usePrefersReducedMotion } from './src/motion/use-reduced-motion.js';
export { Reveal } from './src/motion/reveal.jsx';

export { cn } from './src/utils/cn.js';
