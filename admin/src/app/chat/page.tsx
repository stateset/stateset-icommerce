'use client';

import { useState, useRef, useEffect } from 'react';
import dynamic from 'next/dynamic';
import { Card, Title, Text, Badge } from '@tremor/react';
import { PaperAirplaneIcon, SparklesIcon, UserIcon, CpuChipIcon } from '@heroicons/react/24/outline';
import { motion, AnimatePresence } from 'framer-motion';
import { Button } from '@/components/ui/button';
import { componentRegistry } from '@/lib/ui/component-registry';

// Dynamic import for heavy GenerativeRenderer component
const GenerativeRenderer = dynamic(
  () => import('@/lib/ui/generative-renderer').then(mod => mod.GenerativeRenderer),
  {
    loading: () => (
      <div className="animate-pulse p-4 rounded-lg border bg-gray-50 dark:bg-gray-800">
        <div className="h-32 bg-gray-200 dark:bg-gray-700 rounded" />
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
      content: 'Hello! I\'m your AI commerce assistant powered by the embedded StateSet engine. Ask me anything about your orders, inventory, customers, or operations. I\'ll generate the right visualization for you.',
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

    // Simulate AI processing
    await new Promise(resolve => setTimeout(resolve, 500));

    // Classify intent and select component
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
      {/* Header */}
      <div className="mb-4">
        <div className="flex items-center space-x-2 mb-2">
          <SparklesIcon className="w-8 h-8 text-indigo-600" />
          <Title className="text-2xl">AI Commerce Assistant</Title>
        </div>
        <Text className="text-gray-600">
          Natural language interface to your embedded commerce engine. Ask questions and get real-time generative UI responses.
        </Text>
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
                      ? 'bg-indigo-100 dark:bg-indigo-900'
                      : 'bg-purple-100 dark:bg-purple-900'
                  }`}>
                    {message.role === 'user' ? (
                      <UserIcon className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
                    ) : (
                      <CpuChipIcon className="w-5 h-5 text-purple-600 dark:text-purple-400" />
                    )}
                  </div>

                  <div className="space-y-2">
                    <div className={`rounded-lg p-3 ${
                      message.role === 'user'
                        ? 'bg-indigo-600 text-white'
                        : 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
                    }`}>
                      <Text className={message.role === 'user' ? 'text-white' : ''}>
                        {message.content}
                      </Text>
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
              <div className="w-8 h-8 rounded-full bg-purple-100 dark:bg-purple-900 flex items-center justify-center">
                <CpuChipIcon className="w-5 h-5 text-purple-600 dark:text-purple-400 animate-pulse" />
              </div>
              <div className="bg-gray-100 dark:bg-gray-800 rounded-lg p-3">
                <div className="flex items-center space-x-2">
                  <div className="w-2 h-2 bg-purple-600 rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                  <div className="w-2 h-2 bg-purple-600 rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                  <div className="w-2 h-2 bg-purple-600 rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                </div>
              </div>
            </motion.div>
          )}

          <div ref={messagesEndRef} />
        </div>

        {/* Suggested Queries */}
        <div className="border-t dark:border-gray-700 p-3">
          <Text className="text-xs text-gray-500 mb-2">Suggested queries:</Text>
          <div className="flex flex-wrap gap-2">
            {suggestedQueries.map((suggestion) => (
              <Badge
                key={suggestion.label}
                color="gray"
                className="cursor-pointer hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
                onClick={() => handleSuggestionClick(suggestion.query)}
              >
                {suggestion.label}
              </Badge>
            ))}
          </div>
        </div>

        {/* Input */}
        <form onSubmit={handleSubmit} className="border-t dark:border-gray-700 p-4">
          <div className="flex items-center space-x-2">
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Ask about your commerce operations..."
              className="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-transparent dark:bg-gray-800"
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
