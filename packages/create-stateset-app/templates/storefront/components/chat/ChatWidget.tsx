'use client';

import { useState, useRef, useEffect } from 'react';
import { useChat } from '@ai-sdk/react';
import { useAccount } from 'wagmi';

export function ChatWidget() {
  const [isOpen, setIsOpen] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const { address } = useAccount();

  const { messages, input, handleInputChange, handleSubmit, isLoading } = useChat({
    api: '/api/chat',
    body: { walletAddress: address },
  });

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  return (
    <>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="fixed bottom-6 right-6 w-14 h-14 bg-black text-white rounded-full shadow-lg hover:bg-gray-800 transition-colors flex items-center justify-center text-xl z-50"
        aria-label="Toggle chat"
      >
        {isOpen ? '\u00d7' : '?'}
      </button>

      {isOpen && (
        <div className="fixed bottom-24 right-6 w-96 h-[500px] bg-white rounded-xl shadow-2xl border flex flex-col z-50">
          <div className="px-4 py-3 border-b">
            <h3 className="font-semibold">Store Assistant</h3>
          </div>

          <div className="flex-1 overflow-y-auto p-4 space-y-4">
            {messages.length === 0 && (
              <div className="text-center py-8">
                <p className="text-gray-500 mb-4">How can I help you today?</p>
                <div className="space-y-2">
                  {['What products do you have?', 'Check my order status', 'Recommend something'].map((q) => (
                    <button
                      key={q}
                      onClick={() => {
                        handleInputChange({ target: { value: q } } as any);
                        setTimeout(() => {
                          const form = document.querySelector('#chat-form') as HTMLFormElement;
                          form?.requestSubmit();
                        }, 50);
                      }}
                      className="block w-full text-left px-3 py-2 text-sm border rounded-lg hover:bg-gray-50 transition-colors"
                    >
                      {q}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {messages.map((m) => (
              <div
                key={m.id}
                className={`flex ${m.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  className={`max-w-[80%] px-3 py-2 rounded-lg text-sm ${
                    m.role === 'user'
                      ? 'bg-black text-white'
                      : 'bg-gray-100 text-gray-900'
                  }`}
                >
                  {m.content}
                </div>
              </div>
            ))}

            {isLoading && (
              <div className="flex justify-start">
                <div className="bg-gray-100 px-3 py-2 rounded-lg text-sm text-gray-500">
                  Thinking...
                </div>
              </div>
            )}

            <div ref={messagesEndRef} />
          </div>

          <form
            id="chat-form"
            onSubmit={handleSubmit}
            className="p-4 border-t flex gap-2"
          >
            <input
              value={input}
              onChange={handleInputChange}
              placeholder="Ask a question..."
              className="flex-1 px-3 py-2 border rounded-lg text-sm focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
            <button
              type="submit"
              disabled={isLoading || !input.trim()}
              className="px-4 py-2 bg-black text-white rounded-lg text-sm hover:bg-gray-800 disabled:opacity-50"
            >
              Send
            </button>
          </form>
        </div>
      )}
    </>
  );
}
