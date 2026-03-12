import { useState, useEffect, useRef } from 'react';
import { X, Shield, Eye, EyeOff } from 'lucide-react';

interface PasswordConfirmModalProps {
    isOpen: boolean;
    onConfirm: (password: string) => void;
    onCancel: () => void;
    title?: string;
    description?: string;
}

export function PasswordConfirmModal({
    isOpen,
    onConfirm,
    onCancel,
    title = 'Confirm Your Password',
    description = 'Enter your account password to confirm this action.',
}: PasswordConfirmModalProps) {
    const [password, setPassword] = useState('');
    const [showPassword, setShowPassword] = useState(false);
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (isOpen) {
            setPassword('');
            setShowPassword(false);
            setTimeout(() => inputRef.current?.focus(), 50);
        }
    }, [isOpen]);

    // Clear password on unmount
    useEffect(() => {
        return () => setPassword('');
    }, []);

    if (!isOpen) return null;

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        if (!password) return;
        onConfirm(password);
    };

    const handleClose = () => {
        setPassword('');
        onCancel();
    };

    return (
        <div className="fixed inset-0 z-50 overflow-y-auto">
            <div className="flex items-center justify-center min-h-screen px-4">
                <div className="fixed inset-0 bg-black/50 transition-opacity" onClick={handleClose} />

                <div className="relative bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-sm w-full p-6">
                    <div className="flex items-center justify-between mb-4">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-primary-100 dark:bg-primary-900/30 rounded-lg">
                                <Shield className="w-5 h-5 text-primary-600 dark:text-primary-400" />
                            </div>
                            <div>
                                <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                                    {title}
                                </h3>
                                <p className="text-sm text-gray-500 dark:text-gray-400">
                                    {description}
                                </p>
                            </div>
                        </div>
                        <button onClick={handleClose} className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded">
                            <X className="w-5 h-5 text-gray-500" />
                        </button>
                    </div>

                    <form onSubmit={handleSubmit} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                Password
                            </label>
                            <div className="relative">
                                <input
                                    ref={inputRef}
                                    type={showPassword ? 'text' : 'password'}
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                    placeholder="Your current password"
                                    className="w-full px-3 py-2 pr-10 border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-primary-500"
                                />
                                <button
                                    type="button"
                                    onClick={() => setShowPassword(!showPassword)}
                                    className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                                >
                                    {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                </button>
                            </div>
                        </div>

                        <div className="flex gap-3 pt-1">
                            <button
                                type="button"
                                onClick={handleClose}
                                className="flex-1 px-4 py-2 border border-gray-200 dark:border-gray-600 text-gray-700 dark:text-gray-200 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 text-sm font-medium"
                            >
                                Cancel
                            </button>
                            <button
                                type="submit"
                                disabled={!password}
                                className={`flex-1 px-4 py-2 rounded-lg text-sm font-medium text-white ${
                                    password
                                        ? 'bg-primary-600 hover:bg-primary-700'
                                        : 'bg-gray-300 dark:bg-gray-600 cursor-not-allowed'
                                }`}
                            >
                                Confirm
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </div>
    );
}
