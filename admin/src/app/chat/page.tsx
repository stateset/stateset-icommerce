'use client';

import { useState, useRef, useEffect } from 'react';
import dynamic from 'next/dynamic';
import { Card, Badge } from '@stateset/design';
import { PaperAirplaneIcon, SparklesIcon, UserIcon, CpuChipIcon } from '@heroicons/react/24/outline';
import { motion, AnimatePresence } from 'framer-motion';
import { Button } from '@/components/ui/button';
import { componentRegistry } from '@/lib/ui/component-registry';

// Dynamic import for heavy GenerativeRenderer component
const GenerativeRenderer = dynamic(
  () => import('@/lib/ui/generative-renderer').then(mod => mod.GenerativeRenderer),
  {
    loading: () => (
      <div className="animate-pulse p-4 rounded-lg border border-ds-enterprise-line bg-ds-muted">
        <div className="h-32 bg-ds-muted rounded" />
      </div>
    ),
    ssr: false,
  }
);

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  component?: {
    id: string;
    data?: ChatComponentData;
  };
  timestamp: Date;
}

interface ChatComponentData {
  intent: string;
  category: string;
}

const suggestedQueries = [
  { label: 'Dashboard', query: 'Show me the unified dashboard' },
  { label: 'Orders', query: 'Show the order pipeline' },
  { label: 'Inventory', query: 'What products are low in stock?' },
  { label: 'Returns', query: 'Show returns management' },
  { label: 'Customers', query: 'Show customer health scores' },
  { label: 'Analytics', query: 'Display revenue analytics' },
];

// Simple intent classifier for demo (in production, use AI model)
function classifyIntent(query: string): { componentId: string; context: ChatComponentData } | null {
  const queryLower = query.toLowerCase();

  // Search component registry for best match
  const matches = componentRegistry.searchComponents(query, 3);
  if (matches.length > 0) {
    return {
      componentId: matches[0].id,
      context: { intent: query, category: matches[0].category },
    };
  }

  // Fallback keyword matching
  if (queryLower.includes('dashboard') || queryLower.includes('overview') || queryLower.includes('kpi')) {
    return { componentId: 'unified-dashboard', context: { intent: query, category: 'operations' } };
  }
  if (queryLower.includes('order') || queryLower.includes('fulfillment')) {
    return { componentId: 'order-pipeline', context: { intent: query, category: 'orders' } };
  }
  if (queryLower.includes('inventory') || queryLower.includes('stock') || queryLower.includes('sku')) {
    return { componentId: 'inventory-analytics', context: { intent: query, category: 'inventory' } };
  }
  if (queryLower.includes('return') || queryLower.includes('rma') || queryLower.includes('refund')) {
    return { componentId: 'returns-management', context: { intent: query, category: 'returns' } };
  }
  if (queryLower.includes('customer') || queryLower.includes('churn') || queryLower.includes('segment')) {
    return { componentId: 'customer-health-score', context: { intent: query, category: 'customers' } };
  }
  if (queryLower.includes('subscription') || queryLower.includes('mrr') || queryLower.includes('recurring')) {
    return { componentId: 'subscription-analytics', context: { intent: query, category: 'subscriptions' } };
  }

  return null;
}

export default function ChatPage() {
  const [messages, setMessages] = useState<Message[]>([
    {
      id: '1',
      role: 'assistant',
      content: 'Hello! This is a scripted demo of the commerce assistant: your message is matched against keywords (no AI model is involved) to pick a dashboard view rendered from the embedded StateSet engine. Try asking about orders, inventory, customers, or operations.',
      timestamp: new Date(),
    },
  ]);
  const [input, setInput] = useState('');
  const [isProcessing, setIsProcessing] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isProcessing) return;

    const userMessage: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: input,
      timestamp: new Date(),
    };

    setMessages(prev => [...prev, userMessage]);
    setInput('');
    setIsProcessing(true);

    // Artificial delay so the scripted demo reads as "thinking"; there is
    // no model call here — see the scripted-demo badge in the header.
    await new Promise(resolve => setTimeout(resolve, 500));

    // Classify intent and select component (keyword/registry matching)
    const result = classifyIntent(input);

    let assistantMessage: Message;

    if (result) {
      const component = componentRegistry.getComponent(result.componentId);
      assistantMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: `Here's the ${component?.name || 'requested view'} based on your query. All data is pulled directly from the embedded commerce engine with zero latency.`,
        component: { id: result.componentId, data: result.context },
        timestamp: new Date(),
      };
    } else {
      assistantMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: 'I\'m not sure which view you\'re looking for. Try asking about orders, inventory, returns, customers, or the main dashboard. You can also use the suggested queries below.',
        timestamp: new Date(),
      };
    }

    setMessages(prev => [...prev, assistantMessage]);
    setIsProcessing(false);
  };

  const handleSuggestionClick = (query: string) => {
    setInput(query);
  };

  return (
    <div className="flex flex-col h-[calc(100vh-6rem)]">
      {/* Header.
          Honesty note: this page is a scripted demo — classifyIntent() does
          keyword/registry matching with an artificial delay; no AI model is
          called. The badge below must stay until a real model is wired in. */}
      <div className="mb-4">
        <div className="flex items-center space-x-2 mb-2">
          <SparklesIcon className="w-8 h-8 text-ds-primary" />
          <h3 className="font-ds-display text-2xl font-semibold text-ds-foreground">Commerce Assistant</h3>
          <Badge variant="warning">Scripted demo — no AI model</Badge>
        </div>
        <p className="text-sm text-ds-muted-foreground">
          Demo interface: your question is matched against keywords to pick a dashboard view. Data in the views comes from the embedded commerce engine.
        </p>
      </div>

      {/* Chat Area */}
      <Card className="flex-1 flex flex-col overflow-hidden">
        {/* Messages */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4" aria-busy={isProcessing} aria-live="polite" role="log">
          <AnimatePresence>
            {messages.map((message) => (
              <motion.div
                key={message.id}
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -20 }}
                className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div className={`flex items-start space-x-2 max-w-[85%] ${message.role === 'user' ? 'flex-row-reverse space-x-reverse' : ''}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 ${
                    message.role === 'user'
                      ? 'bg-ds-brand-100 dark:bg-ds-brand-900'
                      : 'bg-ds-muted'
                  }`}>
                    {message.role === 'user' ? (
                      <UserIcon className="w-5 h-5 text-ds-primary" />
                    ) : (
                      <CpuChipIcon className="w-5 h-5 text-ds-accent" />
                    )}
                  </div>

                  <div className="space-y-2">
                    <div className={`rounded-lg p-3 ${
                      message.role === 'user'
                        ? 'bg-ds-primary text-ds-primary-foreground'
                        : 'bg-ds-muted text-ds-foreground'
                    }`}>
                      <p className={`text-sm ${message.role === 'user' ? 'text-ds-primary-foreground' : 'text-ds-foreground'}`}>
                        {message.content}
                      </p>
                    </div>

                    {/* Generative UI Component */}
                    {message.component && (
                      <motion.div
                        initial={{ opacity: 0, scale: 0.95 }}
                        animate={{ opacity: 1, scale: 1 }}
                        transition={{ delay: 0.2 }}
                      >
                        <GenerativeRenderer
                          componentId={message.component.id}
                          context={message.component.data}
                          showMetadata
                          showAlternatives
                        />
                      </motion.div>
                    )}
                  </div>
                </div>
              </motion.div>
            ))}
          </AnimatePresence>

          {/* Processing Indicator */}
          {isProcessing && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="flex items-start space-x-2"
            >
              <div className="w-8 h-8 rounded-full bg-ds-muted flex items-center justify-center">
                <CpuChipIcon className="w-5 h-5 text-ds-accent animate-pulse" />
              </div>
              <div className="bg-ds-muted rounded-lg p-3">
                <div className="flex items-center space-x-2">
                  <div className="w-2 h-2 bg-ds-accent rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                  <div className="w-2 h-2 bg-ds-accent rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                  <div className="w-2 h-2 bg-ds-accent rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                </div>
              </div>
            </motion.div>
          )}

          <div ref={messagesEndRef} />
        </div>

        {/* Suggested Queries */}
        <div className="border-t border-ds-enterprise-line p-3">
          <p className="text-xs text-ds-muted-foreground mb-2">Suggested queries:</p>
          <div className="flex flex-wrap gap-2">
            {suggestedQueries.map((suggestion) => (
              <Badge
                key={suggestion.label}
                variant="default"
                className="cursor-pointer hover:bg-ds-brand-50 transition-colors"
                onClick={() => handleSuggestionClick(suggestion.query)}
              >
                {suggestion.label}
              </Badge>
            ))}
          </div>
        </div>

        {/* Input */}
        <form onSubmit={handleSubmit} className="border-t border-ds-enterprise-line p-4">
          <div className="flex items-center space-x-2">
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Ask about your commerce operations..."
              className="flex-1 px-4 py-2 border border-ds-enterprise-line rounded-lg bg-ds-card focus:ring-2 focus:ring-ds-primary focus:border-transparent"
              disabled={isProcessing}
            />
            <Button type="submit" variant="primary" disabled={isProcessing || !input.trim()}>
              <PaperAirplaneIcon className="w-5 h-5" />
            </Button>
          </div>
        </form>
      </Card>
    </div>
  );
}
