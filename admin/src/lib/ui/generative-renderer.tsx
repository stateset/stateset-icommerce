'use client';

import React, { Suspense, useState, useEffect, useCallback, useRef } from 'react';
import { componentRegistry, type GenerativeComponent, type ComponentContext } from './component-registry';
import { Card, Title, Text, Badge } from '@tremor/react';
import { AlertCircle, Loader2, RefreshCw, Sparkles, Zap, ChevronRight } from 'lucide-react';
import { Button } from '@/components/ui/button';

// Performance metrics for tracking component render times
interface PerformanceMetrics {
  selectionTime: number;
  dataResolutionTime: number;
  loadTime: number;
  totalTime: number;
  componentId: string;
  timestamp: Date;
}

// Enhanced props with configuration options
interface GenerativeRendererProps {
  componentId?: string;
  data?: Record<string, unknown>;
  context?: ComponentContext;
  fallback?: React.ComponentType<Record<string, unknown>>;
  onComponentSelect?: (component: GenerativeComponent, confidence: number) => void;
  onError?: (error: Error, component: GenerativeComponent | null) => void;
  onPerformance?: (metrics: PerformanceMetrics) => void;
  enablePreloading?: boolean;
  showMetadata?: boolean;
  showAlternatives?: boolean;
  maxRetries?: number;
  retryDelay?: number;
  skeleton?: 'default' | 'pulse' | 'wave' | 'none';
  transitionDuration?: number;
}

interface ComponentState {
  component: GenerativeComponent | null;
  loading: boolean;
  error: Error | null;
  LoadedComponent: React.ComponentType<Record<string, unknown>> | null;
  resolvedData: Record<string, unknown> | null;
  retryCount: number;
  phase: 'idle' | 'selecting' | 'resolving' | 'loading' | 'ready' | 'error';
}

// Component cache for faster subsequent loads
const componentCache = new Map<string, React.ComponentType<Record<string, unknown>>>();
const dataCache = new Map<string, { data: Record<string, unknown>; timestamp: number; ttl: number }>();
const DATA_CACHE_TTL = 30000; // 30 seconds

// Skeleton loading component
function SkeletonLoader({ style = 'wave' }: { style: 'default' | 'pulse' | 'wave' | 'none' }) {
  if (style === 'none') return null;

  const animationClass = style === 'wave'
    ? 'animate-pulse bg-gradient-to-r from-gray-200 via-gray-100 to-gray-200'
    : style === 'pulse'
    ? 'animate-pulse bg-gray-200'
    : 'bg-gray-100';

  return (
    <Card className="w-full overflow-hidden">
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between">
          <div className={`h-6 w-48 rounded ${animationClass}`} />
          <div className={`h-5 w-20 rounded-full ${animationClass}`} />
        </div>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="space-y-2">
              <div className={`h-4 w-16 rounded ${animationClass}`} />
              <div className={`h-8 w-24 rounded ${animationClass}`} />
            </div>
          ))}
        </div>
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className={`h-16 w-full rounded-lg ${animationClass}`} />
          ))}
        </div>
        <div className={`h-48 w-full rounded-lg ${animationClass}`} />
      </div>
    </Card>
  );
}

// Phase indicator
function PhaseIndicator({ phase }: { phase: ComponentState['phase'] }) {
  const phases = [
    { key: 'selecting', label: 'Selecting', icon: Sparkles },
    { key: 'resolving', label: 'Loading data', icon: Zap },
    { key: 'loading', label: 'Rendering', icon: Loader2 },
  ];

  const currentIndex = phases.findIndex(p => p.key === phase);

  return (
    <div className="flex items-center space-x-2 text-sm text-gray-500">
      {phases.map((p, index) => {
        const Icon = p.icon;
        const isActive = p.key === phase;
        const isComplete = currentIndex > index;

        return (
          <React.Fragment key={p.key}>
            <div className={`flex items-center space-x-1 ${
              isActive ? 'text-indigo-600' : isComplete ? 'text-emerald-600' : 'text-gray-400'
            }`}>
              <Icon className={`h-4 w-4 ${isActive ? 'animate-spin' : ''}`} />
              <span className={isActive ? 'font-medium' : ''}>{p.label}</span>
            </div>
            {index < phases.length - 1 && (
              <ChevronRight className={`h-3 w-3 ${isComplete ? 'text-emerald-600' : 'text-gray-300'}`} />
            )}
          </React.Fragment>
        );
      })}
    </div>
  );
}

export function GenerativeRenderer({
  componentId,
  data,
  context = {},
  fallback: FallbackComponent,
  onComponentSelect,
  onError,
  onPerformance,
  enablePreloading = true,
  showMetadata = true,
  showAlternatives = true,
  maxRetries = 2,
  retryDelay = 1000,
  skeleton = 'wave',
  transitionDuration = 300,
}: GenerativeRendererProps) {
  const [state, setState] = useState<ComponentState>({
    component: null,
    loading: true,
    error: null,
    LoadedComponent: null,
    resolvedData: data ?? null,
    retryCount: 0,
    phase: 'idle',
  });

  const [alternatives, setAlternatives] = useState<GenerativeComponent[]>([]);
  const [confidence, setConfidence] = useState<number>(0);
  const [isTransitioning, setIsTransitioning] = useState(false);

  const abortControllerRef = useRef<AbortController | null>(null);
  const performanceRef = useRef<Partial<PerformanceMetrics>>({});
  const mountedRef = useRef(true);
  const didInitialLoadRef = useRef(false);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      abortControllerRef.current?.abort();
    };
  }, []);

  // Cache helpers
  const getCachedData = useCallback((cacheKey: string) => {
    const cached = dataCache.get(cacheKey);
    if (cached && Date.now() - cached.timestamp < cached.ttl) {
      return cached.data;
    }
    dataCache.delete(cacheKey);
    return null;
  }, []);

  const setCachedData = useCallback((cacheKey: string, data: Record<string, unknown>, ttl = DATA_CACHE_TTL) => {
    dataCache.set(cacheKey, { data, timestamp: Date.now(), ttl });
  }, []);

  // Select component based on context
  const selectComponent = useCallback(async (): Promise<GenerativeComponent | null> => {
    const startTime = performance.now();

    let selectedComponent: GenerativeComponent | null = null;

    if (componentId) {
      selectedComponent = componentRegistry.getComponent(componentId);
      if (!selectedComponent) {
        throw new Error(`Component "${componentId}" not found in registry`);
      }
    } else {
      // AI-powered component selection
      selectedComponent = componentRegistry.selectOptimalComponent({
        ...context,
        data,
      });

      if (!selectedComponent) {
        // Fallback to search
        const searchResults = componentRegistry.searchComponents(
          context.intent || context.category || 'dashboard',
          10
        );
        selectedComponent = searchResults[0] || null;
      }
    }

    performanceRef.current.selectionTime = performance.now() - startTime;
    return selectedComponent;
  }, [componentId, context, data]);

  // Load component with caching
  const loadComponent = useCallback(async (component: GenerativeComponent): Promise<React.ComponentType<Record<string, unknown>>> => {
    const startTime = performance.now();

    const cached = componentCache.get(component.id);
    if (cached) {
      performanceRef.current.loadTime = performance.now() - startTime;
      return cached;
    }

    const LoadedComponent = await component.load();
    componentCache.set(component.id, LoadedComponent);

    performanceRef.current.loadTime = performance.now() - startTime;
    return LoadedComponent;
  }, []);

  // Resolve data with caching
  const resolveData = useCallback(async (component: GenerativeComponent): Promise<Record<string, unknown> | null> => {
    const startTime = performance.now();

    if (data) {
      performanceRef.current.dataResolutionTime = performance.now() - startTime;
      return data;
    }

    if (!component.resolveData) {
      performanceRef.current.dataResolutionTime = performance.now() - startTime;
      return null;
    }

    const cacheKey = `${component.id}:${JSON.stringify(context)}`;
    const cachedData = getCachedData(cacheKey);
    if (cachedData) {
      performanceRef.current.dataResolutionTime = performance.now() - startTime;
      return cachedData;
    }

    const resolvedData = await component.resolveData(context);

    setCachedData(cacheKey, resolvedData);
    performanceRef.current.dataResolutionTime = performance.now() - startTime;
    return resolvedData;
  }, [data, context, getCachedData, setCachedData]);

  // Calculate confidence score
  const calculateConfidence = useCallback((component: GenerativeComponent, ctx: ComponentContext): number => {
    let score = 0.5;

    if (ctx.category === component.category) score += 0.2;

    if (ctx.intent) {
      const intentMatch = component.aiPrompts.some(prompt =>
        prompt.toLowerCase().includes(ctx.intent!.toLowerCase()) ||
        ctx.intent!.toLowerCase().includes(prompt.toLowerCase())
      );
      if (intentMatch) score += 0.2;
    }

    if (data && component.dataShape) {
      try {
        component.dataShape.parse(data);
        score += 0.3;
      } catch {
        score += 0.1;
      }
    }

    return Math.min(score, 1.0);
  }, [data]);

  // Get alternative components
  const getAlternatives = useCallback((current: GenerativeComponent): GenerativeComponent[] => {
    const sameCategory = componentRegistry.getComponentsByCategory(current.category)
      .filter(c => c.id !== current.id)
      .slice(0, 3);
    return sameCategory;
  }, []);

  // Main loading logic
  const selectAndLoadComponent = useCallback(async (isRetry = false, forcedComponent?: GenerativeComponent) => {
    abortControllerRef.current?.abort();
    abortControllerRef.current = new AbortController();

    const totalStartTime = performance.now();
    performanceRef.current = {};

    if (!isRetry) {
      setState(prev => ({
        ...prev,
        loading: true,
        error: null,
        phase: 'selecting',
        retryCount: 0,
      }));
    }

    let selectedComponent: GenerativeComponent | null = null;

    try {
      // Phase 1: Select
      setState(prev => ({ ...prev, phase: 'selecting' }));
      selectedComponent = forcedComponent || await selectComponent();

      if (!selectedComponent) {
        throw new Error('No suitable component found');
      }

      // Start loading in parallel with data resolution
      const loadPromise = loadComponent(selectedComponent);

      // Phase 2: Resolve data
      setState(prev => ({ ...prev, phase: 'resolving' }));
      const resolvedData = await resolveData(selectedComponent);

      // Calculate confidence
      const conf = calculateConfidence(selectedComponent, context);
      setConfidence(conf);

      // Load alternatives
      if (showAlternatives) {
        const alts = getAlternatives(selectedComponent);
        setAlternatives(alts);

        // Preload alternatives in background
        if (enablePreloading) {
          alts.slice(0, 2).forEach(alt => {
            loadComponent(alt).catch((err) => console.warn('[generative-renderer] Preload failed:', err.message));
          });
        }
      }

      // Phase 3: Load component
      setState(prev => ({ ...prev, phase: 'loading' }));
      const LoadedComponent = await loadPromise;

      // Track usage
      componentRegistry.trackUsage(selectedComponent.id);

      // Report performance
      const totalTime = performance.now() - totalStartTime;
      if (onPerformance) {
        onPerformance({
          ...performanceRef.current as PerformanceMetrics,
          totalTime,
          componentId: selectedComponent.id,
          timestamp: new Date(),
        });
      }

      // Smooth transition
      setIsTransitioning(true);

      if (mountedRef.current) {
        setState({
          component: selectedComponent,
          loading: false,
          error: null,
          LoadedComponent,
          resolvedData: resolvedData ?? null,
          retryCount: 0,
          phase: 'ready',
        });
      }

      setTimeout(() => setIsTransitioning(false), transitionDuration);
      onComponentSelect?.(selectedComponent, conf);

    } catch (error) {
      const err = error instanceof Error ? error : new Error('Component loading failed');

      if (state.retryCount < maxRetries) {
        setState(prev => ({
          ...prev,
          retryCount: prev.retryCount + 1,
          phase: 'selecting',
        }));

        setTimeout(() => {
          if (mountedRef.current) {
            selectAndLoadComponent(true);
          }
        }, retryDelay * (state.retryCount + 1));
        return;
      }

      if (mountedRef.current) {
        setState(prev => ({
          ...prev,
          loading: false,
          error: err,
          LoadedComponent: null,
          resolvedData: null,
          phase: 'error',
        }));
      }

      onError?.(err, selectedComponent);
    }
  }, [selectComponent, resolveData, loadComponent, context, showAlternatives, enablePreloading, maxRetries, retryDelay, transitionDuration, onComponentSelect, onError, onPerformance, state.retryCount, calculateConfidence, getAlternatives]);

  // Initial load
  useEffect(() => {
    if (didInitialLoadRef.current) return;
    didInitialLoadRef.current = true;
    selectAndLoadComponent();
  }, [selectAndLoadComponent]);

  const handleAlternativeSelect = (alternative: GenerativeComponent) => {
    selectAndLoadComponent(false, alternative);
  };

  const handleRetry = () => {
    selectAndLoadComponent();
  };

  // Loading state
  if (state.loading) {
    return (
      <div className="w-full space-y-3">
        <div className="flex items-center justify-between">
          <PhaseIndicator phase={state.phase} />
          {state.retryCount > 0 && (
            <Badge color="gray" className="text-xs">
              Retry {state.retryCount}/{maxRetries}
            </Badge>
          )}
        </div>
        <SkeletonLoader style={skeleton} />
      </div>
    );
  }

  // Error state
  if (state.error) {
    return (
      <Card className="w-full border-red-200 bg-red-50/50 dark:bg-red-900/20">
        <div className="p-6">
          <div className="flex items-center space-x-2 text-red-600 mb-4">
            <AlertCircle className="h-5 w-5" />
            <Title>Component Loading Error</Title>
          </div>
          <Text className="text-red-800 dark:text-red-200 mb-4">{state.error.message}</Text>

          <div className="flex items-center space-x-2">
            <Button onClick={handleRetry} variant="outline" size="sm">
              <RefreshCw className="h-4 w-4 mr-2" />
              Retry
            </Button>
          </div>

          {showAlternatives && alternatives.length > 0 && (
            <div className="mt-4 p-3 bg-white dark:bg-gray-800 rounded-lg border">
              <Text className="font-medium mb-2 flex items-center">
                <Sparkles className="h-4 w-4 mr-1 text-purple-500" />
                Try alternatives:
              </Text>
              <div className="flex flex-wrap gap-2">
                {alternatives.map((alt) => (
                  <Button
                    key={alt.id}
                    onClick={() => handleAlternativeSelect(alt)}
                    size="sm"
                    variant="secondary"
                  >
                    {alt.name}
                  </Button>
                ))}
              </div>
            </div>
          )}

          {FallbackComponent && (
            <div className="mt-4 pt-4 border-t">
              <FallbackComponent {...(state.resolvedData || data || {})} />
            </div>
          )}
        </div>
      </Card>
    );
  }

  // Success state
  if (state.component && state.LoadedComponent) {
    const { component, LoadedComponent } = state;
    const componentProps = state.resolvedData || data || {};

    const getConfidenceColor = () => {
      if (confidence >= 0.9) return 'text-emerald-600';
      if (confidence >= 0.7) return 'text-blue-600';
      return 'text-gray-600';
    };

    return (
      <div
        className={`w-full space-y-2 transition-opacity ${
          isTransitioning ? 'opacity-0' : 'opacity-100'
        }`}
        style={{ transitionDuration: `${transitionDuration}ms` }}
      >
        {showMetadata && (
          <div className="flex items-center justify-between text-sm text-gray-500 px-1">
            <div className="flex items-center space-x-2">
              <Badge color="indigo" className="capitalize">
                {component.category}
              </Badge>
              <span className="font-medium text-gray-700 dark:text-gray-300">{component.name}</span>
              {confidence > 0 && (
                <Badge color="gray" className={`text-xs ${getConfidenceColor()}`}>
                  <Sparkles className="h-3 w-3 mr-1" />
                  {Math.round(confidence * 100)}% match
                </Badge>
              )}
            </div>

            {showAlternatives && alternatives.length > 0 && (
              <div className="flex items-center space-x-1">
                <span className="text-xs text-gray-400">Switch to:</span>
                {alternatives.slice(0, 3).map((alt) => (
                  <Button
                    key={alt.id}
                    onClick={() => handleAlternativeSelect(alt)}
                    size="sm"
                    variant="ghost"
                    className="h-6 px-2 text-xs hover:bg-gray-100 dark:hover:bg-gray-800"
                  >
                    {alt.name}
                  </Button>
                ))}
              </div>
            )}
          </div>
        )}

        <Suspense fallback={<SkeletonLoader style={skeleton} />}>
          <LoadedComponent {...componentProps} />
        </Suspense>
      </div>
    );
  }

  return (
    <Card className="w-full">
      <div className="text-center p-8">
        <Text className="text-gray-500">No component available</Text>
      </div>
    </Card>
  );
}

// Preset configurations
export const GenerativeRendererPresets = {
  AgentResponse: ({ response, agentType, onPerformance }: {
    response: { uiComponent?: string; uiParams?: Record<string, unknown>; data?: Record<string, unknown>; action?: string };
    agentType?: string;
    onPerformance?: (metrics: PerformanceMetrics) => void;
  }) => (
    <GenerativeRenderer
      componentId={response.uiComponent}
      data={response.uiParams || response.data}
      context={{
        intent: response.action,
        agentType,
        category: agentType,
      }}
      onPerformance={onPerformance}
      enablePreloading
      showMetadata
    />
  ),

  Dashboard: ({ metrics, category, intent = 'dashboard' }: {
    metrics: Record<string, unknown>;
    category: string;
    intent?: string;
  }) => (
    <GenerativeRenderer
      context={{ category, intent }}
      data={metrics}
      showMetadata
      showAlternatives
      skeleton="wave"
    />
  ),

  Minimal: ({ componentId, data, context }: {
    componentId: string;
    data?: Record<string, unknown>;
    context?: ComponentContext;
  }) => (
    <GenerativeRenderer
      componentId={componentId}
      data={data}
      context={context}
      showMetadata={false}
      showAlternatives={false}
      skeleton="pulse"
      maxRetries={1}
    />
  ),
};

// Cache management
export const GenerativeRendererCache = {
  clearComponentCache: () => componentCache.clear(),
  clearDataCache: () => dataCache.clear(),
  clearAll: () => {
    componentCache.clear();
    dataCache.clear();
  },
  getStats: () => ({
    componentCacheSize: componentCache.size,
    dataCacheSize: dataCache.size,
  }),
  preload: async (componentIds: string[]) => {
    const results: { id: string; success: boolean }[] = [];
    for (const id of componentIds) {
      try {
        const component = componentRegistry.getComponent(id);
        if (component) {
          const LoadedComponent = await component.load();
          componentCache.set(id, LoadedComponent);
          results.push({ id, success: true });
        } else {
          results.push({ id, success: false });
        }
      } catch {
        results.push({ id, success: false });
      }
    }
    return results;
  },
};

export type { PerformanceMetrics, GenerativeRendererProps };
export default GenerativeRenderer;
