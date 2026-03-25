'use client';

import { useState } from 'react';
import { Send, Loader2 } from 'lucide-react';

interface ChatInputProps {
  onSendMessage: (message: string) => void;
  isLoading: boolean;
  placeholder?: string;
}

export default function ChatInput({
  onSendMessage,
  isLoading,
  placeholder = 'Type your message...',
}: ChatInputProps) {
  const [message, setMessage] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (message.trim() && !isLoading) {
      onSendMessage(message.trim());
      setMessage('');
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  return (
    <form
      onSubmit={handleSubmit}
      className="theme-surface p-6 transition-colors duration-300"
    >
      <div className="flex items-end space-x-3">
        <div className="flex-1 relative">
          <textarea
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            onKeyPress={handleKeyPress}
            placeholder={placeholder}
            disabled={isLoading}
            className="theme-input w-full resize-none border rounded-lg px-4 py-3 transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
            rows={1}
            style={{
              minHeight: '48px',
              maxHeight: '120px',
              height: 'auto',
            }}
            onInput={(e) => {
              const target = e.target as HTMLTextAreaElement;
              target.style.height = 'auto';
              target.style.height = `${Math.min(target.scrollHeight, 120)}px`;
            }}
          />
        </div>

        <button
          type="submit"
          disabled={!message.trim() || isLoading}
          className="theme-primary-button flex items-center justify-center w-12 h-12 disabled:bg-gray-300 text-white rounded-lg transition-all duration-200 disabled:cursor-not-allowed transform hover:scale-105 disabled:hover:scale-100 shadow-lg"
        >
          {isLoading ? (
            <Loader2 className="w-5 h-5 animate-spin" />
          ) : (
            <Send className="w-5 h-5" />
          )}
        </button>
      </div>

      {/* Quick suggestions */}
      <div className="flex flex-wrap gap-2 mt-4">
        {[
          'Convert 100 USDC to USD',
          'Check conversion rates',
          'View transaction history',
        ].map((suggestion, index) => (
          <button
            key={index}
            type="button"
            onClick={() => setMessage(suggestion)}
            className="theme-secondary-button px-3 py-2 text-sm rounded-lg transition-all duration-200 transform hover:scale-105"
          >
            {suggestion}
          </button>
        ))}
      </div>
    </form>
  );
}
