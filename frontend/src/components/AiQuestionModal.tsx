import { useState, useRef, useEffect } from 'react';
import { X, MessageSquare, Loader2, AlertCircle, Send, Sparkles } from 'lucide-react';
import { useAuthFetch } from '../context/AuthContext';
import clsx from 'clsx';

interface AiQuestionModalProps {
    isOpen: boolean;
    onClose: () => void;
    file: {
        id: string;
        name: string;
    };
}

interface Message {
    role: 'user' | 'assistant';
    content: string;
}

export function AiQuestionModal({ isOpen, onClose, file }: AiQuestionModalProps) {
    const authFetch = useAuthFetch();
    const [messages, setMessages] = useState<Message[]>([]);
    const [question, setQuestion] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (isOpen) {
            inputRef.current?.focus();
        }
        return () => {
            setMessages([]);
            setQuestion('');
            setError(null);
        };
    }, [isOpen, file?.id]);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!question.trim() || loading) return;

        const userQuestion = question.trim();
        setQuestion('');
        setLoading(true);
        setError(null);

        // Add user message
        setMessages((prev) => [...prev, { role: 'user', content: userQuestion }]);

        try {
            const res = await authFetch('/api/ai/answer', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    file_id: file.id,
                    question: userQuestion,
                }),
            });

            const data = await res.json();

            if (res.ok && data.success) {
                setMessages((prev) => [
                    ...prev,
                    { role: 'assistant', content: data.content },
                ]);
            } else {
                setError(data.error || 'Failed to get answer');
            }
        } catch (err) {
            setError('Unable to connect to AI service. Please try again later.');
        } finally {
            setLoading(false);
        }
    };

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-[60] overflow-y-auto">
            <div className="flex min-h-screen items-center justify-center p-4">
                {/* Backdrop */}
                <div
                    className="fixed inset-0 bg-black/50 transition-opacity"
                    onClick={onClose}
                />

                {/* Modal */}
                <div className="relative w-full max-w-2xl bg-white dark:bg-gray-800 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[80vh]">
                    {/* Header */}
                    <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-purple-100 dark:bg-purple-900/30 rounded-xl">
                                <MessageSquare className="w-5 h-5 text-purple-600 dark:text-purple-400" />
                            </div>
                            <div>
                                <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
                                    Ask AI
                                </h2>
                                <p className="text-sm text-gray-500 dark:text-gray-400 truncate max-w-[300px]">
                                    About: {file.name}
                                </p>
                            </div>
                        </div>
                        <button
                            onClick={onClose}
                            className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                        >
                            <X className="w-5 h-5 text-gray-500" />
                        </button>
                    </div>

                    {/* Messages */}
                    <div className="flex-1 overflow-y-auto p-6 space-y-4 min-h-[300px]">
                        {messages.length === 0 ? (
                            <div className="flex flex-col items-center justify-center h-full text-center py-8">
                                <div className="p-4 bg-purple-50 dark:bg-purple-900/20 rounded-2xl mb-4">
                                    <Sparkles className="w-8 h-8 text-purple-500" />
                                </div>
                                <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">
                                    Ask anything about this file
                                </h3>
                                <p className="text-gray-500 dark:text-gray-400 max-w-sm">
                                    Get instant answers about the content, structure, or meaning of this document.
                                </p>
                                <div className="mt-4 flex flex-wrap gap-2 justify-center">
                                    {['What is this document about?', 'Summarize the key points', 'What are the main topics?'].map((suggestion) => (
                                        <button
                                            key={suggestion}
                                            onClick={() => setQuestion(suggestion)}
                                            className="px-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-full hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                                        >
                                            {suggestion}
                                        </button>
                                    ))}
                                </div>
                            </div>
                        ) : (
                            messages.map((msg, idx) => (
                                <div
                                    key={idx}
                                    className={clsx(
                                        'flex',
                                        msg.role === 'user' ? 'justify-end' : 'justify-start'
                                    )}
                                >
                                    <div
                                        className={clsx(
                                            'max-w-[80%] rounded-2xl px-4 py-3',
                                            msg.role === 'user'
                                                ? 'bg-purple-600 text-white rounded-br-md'
                                                : 'bg-gray-100 dark:bg-gray-700 text-gray-900 dark:text-white rounded-bl-md'
                                        )}
                                    >
                                        <p className="whitespace-pre-wrap">{msg.content}</p>
                                    </div>
                                </div>
                            ))
                        )}
                        {loading && (
                            <div className="flex justify-start">
                                <div className="bg-gray-100 dark:bg-gray-700 rounded-2xl rounded-bl-md px-4 py-3">
                                    <Loader2 className="w-5 h-5 text-purple-500 animate-spin" />
                                </div>
                            </div>
                        )}
                        {error && (
                            <div className="flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-300 text-sm">
                                <AlertCircle className="w-4 h-4 flex-shrink-0" />
                                {error}
                            </div>
                        )}
                        <div ref={messagesEndRef} />
                    </div>

                    {/* Input */}
                    <form onSubmit={handleSubmit} className="p-4 border-t border-gray-200 dark:border-gray-700">
                        <div className="flex items-center gap-3">
                            <input
                                ref={inputRef}
                                type="text"
                                value={question}
                                onChange={(e) => setQuestion(e.target.value)}
                                placeholder="Ask a question about this file..."
                                disabled={loading}
                                className="flex-1 px-4 py-2.5 border border-gray-300 dark:border-gray-600 rounded-xl bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400 focus:ring-2 focus:ring-purple-500 focus:border-purple-500 disabled:opacity-50"
                            />
                            <button
                                type="submit"
                                disabled={!question.trim() || loading}
                                className={clsx(
                                    'p-2.5 rounded-xl transition-colors',
                                    question.trim() && !loading
                                        ? 'bg-purple-600 text-white hover:bg-purple-700'
                                        : 'bg-gray-200 dark:bg-gray-700 text-gray-400 cursor-not-allowed'
                                )}
                            >
                                {loading ? (
                                    <Loader2 className="w-5 h-5 animate-spin" />
                                ) : (
                                    <Send className="w-5 h-5" />
                                )}
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </div>
    );
}

