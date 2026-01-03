import { useState, useEffect, useCallback } from 'react';
import { MessageSquare, Send, Trash2, Pencil, X, CornerDownRight, Loader2, ChevronDown, ChevronUp } from 'lucide-react';
import clsx from 'clsx';
import { useAuthFetch, useAuth } from '../context/AuthContext';
import { format } from 'date-fns';

interface Comment {
    id: string;
    file_id: string;
    user_id: string;
    user_name: string;
    user_avatar?: string;
    content: string;
    parent_id?: string;
    is_edited: boolean;
    created_at: string;
    updated_at: string;
    replies: Comment[];
    can_edit: boolean;
    can_delete: boolean;
}

interface FileCommentsPanelProps {
    fileId: string;
    companyId: string;
    isExpanded?: boolean;
}

export function FileCommentsPanel({ fileId, companyId, isExpanded = false }: FileCommentsPanelProps) {
    const authFetch = useAuthFetch();
    const { user } = useAuth();
    const [comments, setComments] = useState<Comment[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [newComment, setNewComment] = useState('');
    const [replyingTo, setReplyingTo] = useState<string | null>(null);
    const [replyContent, setReplyContent] = useState('');
    const [editingId, setEditingId] = useState<string | null>(null);
    const [editContent, setEditContent] = useState('');
    const [submitting, setSubmitting] = useState(false);
    const [expanded, setExpanded] = useState(isExpanded);
    const [commentCount, setCommentCount] = useState(0);

    const fetchComments = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const res = await authFetch(`/api/files/${companyId}/${fileId}/comments`);
            if (res.ok) {
                const data = await res.json();
                setComments(data.comments || []);
                setCommentCount(data.total || 0);
            } else {
                setError('Failed to load comments');
            }
        } catch {
            setError('Failed to load comments');
        } finally {
            setLoading(false);
        }
    }, [authFetch, companyId, fileId]);

    useEffect(() => {
        if (expanded) {
            fetchComments();
        }
    }, [expanded, fetchComments]);

    // Fetch comment count on mount
    useEffect(() => {
        const fetchCount = async () => {
            try {
                const res = await authFetch(`/api/files/${companyId}/${fileId}/comments/count`);
                if (res.ok) {
                    const data = await res.json();
                    setCommentCount(data.count || 0);
                }
            } catch {
                // Ignore
            }
        };
        fetchCount();
    }, [authFetch, companyId, fileId]);

    const handleSubmitComment = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!newComment.trim() || submitting) return;

        setSubmitting(true);
        try {
            const res = await authFetch(`/api/files/${companyId}/${fileId}/comments`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: newComment.trim() }),
            });

            if (res.ok) {
                const comment = await res.json();
                setComments((prev) => [...prev, { ...comment, replies: [] }]);
                setNewComment('');
                setCommentCount((prev) => prev + 1);
            } else {
                setError('Failed to post comment');
            }
        } catch {
            setError('Failed to post comment');
        } finally {
            setSubmitting(false);
        }
    };

    const handleSubmitReply = async (parentId: string) => {
        if (!replyContent.trim() || submitting) return;

        setSubmitting(true);
        try {
            const res = await authFetch(`/api/files/${companyId}/${fileId}/comments`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: replyContent.trim(), parent_id: parentId }),
            });

            if (res.ok) {
                const reply = await res.json();
                setComments((prev) =>
                    prev.map((c) =>
                        c.id === parentId ? { ...c, replies: [...c.replies, { ...reply, replies: [] }] } : c
                    )
                );
                setReplyContent('');
                setReplyingTo(null);
                setCommentCount((prev) => prev + 1);
            } else {
                setError('Failed to post reply');
            }
        } catch {
            setError('Failed to post reply');
        } finally {
            setSubmitting(false);
        }
    };

    const handleUpdateComment = async (commentId: string, parentId?: string) => {
        if (!editContent.trim() || submitting) return;

        setSubmitting(true);
        try {
            const res = await authFetch(`/api/files/${companyId}/${fileId}/comments/${commentId}`, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: editContent.trim() }),
            });

            if (res.ok) {
                setComments((prev) => {
                    if (parentId) {
                        return prev.map((c) =>
                            c.id === parentId
                                ? {
                                      ...c,
                                      replies: c.replies.map((r) =>
                                          r.id === commentId ? { ...r, content: editContent.trim(), is_edited: true } : r
                                      ),
                                  }
                                : c
                        );
                    }
                    return prev.map((c) => (c.id === commentId ? { ...c, content: editContent.trim(), is_edited: true } : c));
                });
                setEditingId(null);
                setEditContent('');
            } else {
                setError('Failed to update comment');
            }
        } catch {
            setError('Failed to update comment');
        } finally {
            setSubmitting(false);
        }
    };

    const handleDeleteComment = async (commentId: string, parentId?: string) => {
        if (!window.confirm('Are you sure you want to delete this comment?')) return;

        try {
            const res = await authFetch(`/api/files/${companyId}/${fileId}/comments/${commentId}`, {
                method: 'DELETE',
            });

            if (res.ok) {
                setComments((prev) => {
                    if (parentId) {
                        return prev.map((c) =>
                            c.id === parentId ? { ...c, replies: c.replies.filter((r) => r.id !== commentId) } : c
                        );
                    }
                    return prev.filter((c) => c.id !== commentId);
                });
                setCommentCount((prev) => Math.max(0, prev - 1));
            } else {
                setError('Failed to delete comment');
            }
        } catch {
            setError('Failed to delete comment');
        }
    };

    const CommentItem = ({ comment, isReply = false, parentId }: { comment: Comment; isReply?: boolean; parentId?: string }) => {
        const isEditing = editingId === comment.id;

        return (
            <div className={clsx('py-3', isReply && 'ml-6 border-l-2 border-gray-200 dark:border-gray-700 pl-4')}>
                <div className="flex items-start gap-3">
                    {/* Avatar */}
                    {comment.user_avatar ? (
                        <img src={comment.user_avatar} alt={comment.user_name} className="w-8 h-8 rounded-full object-cover" />
                    ) : (
                        <div className="w-8 h-8 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-xs font-medium text-primary-700 dark:text-primary-300">
                            {comment.user_name?.charAt(0)?.toUpperCase() || '?'}
                        </div>
                    )}

                    <div className="flex-1 min-w-0">
                        {/* Header */}
                        <div className="flex items-center gap-2 mb-1">
                            <span className="text-sm font-medium text-gray-900 dark:text-white">{comment.user_name}</span>
                            <span className="text-xs text-gray-500 dark:text-gray-400">
                                {format(new Date(comment.created_at), 'MMM d, h:mm a')}
                            </span>
                            {comment.is_edited && <span className="text-xs text-gray-400 dark:text-gray-500">(edited)</span>}
                        </div>

                        {/* Content or Edit Form */}
                        {isEditing ? (
                            <div className="flex gap-2">
                                <input
                                    type="text"
                                    value={editContent}
                                    onChange={(e) => setEditContent(e.target.value)}
                                    className="flex-1 px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                    autoFocus
                                />
                                <button
                                    onClick={() => handleUpdateComment(comment.id, parentId)}
                                    disabled={submitting}
                                    className="px-3 py-1.5 text-xs bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50"
                                >
                                    Save
                                </button>
                                <button
                                    onClick={() => {
                                        setEditingId(null);
                                        setEditContent('');
                                    }}
                                    className="px-3 py-1.5 text-xs text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white"
                                >
                                    Cancel
                                </button>
                            </div>
                        ) : (
                            <p className="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{comment.content}</p>
                        )}

                        {/* Actions */}
                        {!isEditing && (
                            <div className="flex items-center gap-3 mt-2">
                                {!isReply && (
                                    <button
                                        onClick={() => {
                                            setReplyingTo(comment.id);
                                            setReplyContent('');
                                        }}
                                        className="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 flex items-center gap-1"
                                    >
                                        <CornerDownRight className="w-3 h-3" /> Reply
                                    </button>
                                )}
                                {comment.can_edit && (
                                    <button
                                        onClick={() => {
                                            setEditingId(comment.id);
                                            setEditContent(comment.content);
                                        }}
                                        className="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 flex items-center gap-1"
                                    >
                                        <Pencil className="w-3 h-3" /> Edit
                                    </button>
                                )}
                                {comment.can_delete && (
                                    <button
                                        onClick={() => handleDeleteComment(comment.id, parentId)}
                                        className="text-xs text-red-500 hover:text-red-700 flex items-center gap-1"
                                    >
                                        <Trash2 className="w-3 h-3" /> Delete
                                    </button>
                                )}
                            </div>
                        )}

                        {/* Reply Form */}
                        {replyingTo === comment.id && (
                            <div className="mt-3 flex gap-2">
                                <input
                                    type="text"
                                    value={replyContent}
                                    onChange={(e) => setReplyContent(e.target.value)}
                                    placeholder="Write a reply..."
                                    className="flex-1 px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                    autoFocus
                                />
                                <button
                                    onClick={() => handleSubmitReply(comment.id)}
                                    disabled={submitting || !replyContent.trim()}
                                    className="px-3 py-1.5 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50"
                                >
                                    <Send className="w-4 h-4" />
                                </button>
                                <button
                                    onClick={() => setReplyingTo(null)}
                                    className="p-1.5 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                                >
                                    <X className="w-4 h-4" />
                                </button>
                            </div>
                        )}
                    </div>
                </div>

                {/* Replies */}
                {comment.replies && comment.replies.length > 0 && (
                    <div className="mt-2">
                        {comment.replies.map((reply) => (
                            <CommentItem key={reply.id} comment={reply} isReply parentId={comment.id} />
                        ))}
                    </div>
                )}
            </div>
        );
    };

    return (
        <div className="border-t border-gray-200 dark:border-gray-700">
            {/* Header - Collapsible */}
            <button
                onClick={() => setExpanded(!expanded)}
                className="w-full flex items-center justify-between px-4 py-3 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors"
            >
                <div className="flex items-center gap-2">
                    <MessageSquare className="w-4 h-4 text-gray-500" />
                    <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Comments</span>
                    {commentCount > 0 && (
                        <span className="px-2 py-0.5 text-xs bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 rounded-full">
                            {commentCount}
                        </span>
                    )}
                </div>
                {expanded ? (
                    <ChevronUp className="w-4 h-4 text-gray-400" />
                ) : (
                    <ChevronDown className="w-4 h-4 text-gray-400" />
                )}
            </button>

            {/* Expanded Content */}
            {expanded && (
                <div className="px-4 pb-4">
                    {/* Error */}
                    {error && (
                        <div className="mb-3 p-2 bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-xs rounded-lg">
                            {error}
                        </div>
                    )}

                    {/* Loading */}
                    {loading && (
                        <div className="flex items-center justify-center py-4">
                            <Loader2 className="w-5 h-5 text-gray-400 animate-spin" />
                        </div>
                    )}

                    {/* Comments List */}
                    {!loading && comments.length > 0 && (
                        <div className="divide-y divide-gray-100 dark:divide-gray-700 mb-4 max-h-64 overflow-y-auto">
                            {comments.map((comment) => (
                                <CommentItem key={comment.id} comment={comment} />
                            ))}
                        </div>
                    )}

                    {/* Empty State */}
                    {!loading && comments.length === 0 && (
                        <div className="text-center py-4 text-sm text-gray-500 dark:text-gray-400">
                            No comments yet. Be the first to comment!
                        </div>
                    )}

                    {/* New Comment Form */}
                    <form onSubmit={handleSubmitComment} className="flex gap-2">
                        <input
                            type="text"
                            value={newComment}
                            onChange={(e) => setNewComment(e.target.value)}
                            placeholder="Write a comment..."
                            className="flex-1 px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400"
                        />
                        <button
                            type="submit"
                            disabled={submitting || !newComment.trim()}
                            className="px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                        >
                            {submitting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-4 h-4" />}
                        </button>
                    </form>
                </div>
            )}
        </div>
    );
}

