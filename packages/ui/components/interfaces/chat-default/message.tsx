"use client";

import {
	CheckIcon,
	ChevronDown,
	ChevronUp,
	CopyIcon,
	EditIcon,
	MessageSquareIcon,
	ThumbsDownIcon,
	ThumbsUpIcon,
	X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { IRole, cn } from "../../../lib";
import {
	Badge,
	Button,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Label,
	Switch,
	TextEditor,
	Textarea,
} from "../../ui";
import { FilePreview, type ProcessedAttachment } from "./attachment";
import { FileDialog, FileDialogPreview } from "./attachment-dialog";
import type { IAttachment, IMessage } from "./chat-db";
import { useProcessedAttachments } from "./hooks/use-processed-attachments";

interface MessageProps {
	message: IMessage;
	loading?: boolean;
	onMessageUpdate?: (
		messageId: string,
		updates: Partial<IMessage>,
	) => void | Promise<void>;
}

const MessageActionButton = ({
	onClick,
	children,
	className,
	title,
}: {
	onClick: () => void;
	children: React.ReactNode;
	className?: string;
	title?: string;
}) => (
	<button
		onClick={onClick}
		className={cn(
			"text-muted-foreground hover:text-foreground transition-colors",
			className,
		)}
		title={title}
	>
		{children}
	</button>
);

const FeedbackButton = ({
	onClick,
	isActive,
	children,
}: {
	onClick: () => void;
	isActive: boolean;
	children: React.ReactNode;
}) => (
	<button
		onClick={onClick}
		className={cn(
			"transition-colors",
			isActive
				? "text-emerald-500 dark:text-emerald-400 hover:text-emerald-300 dark:hover:text-emerald-300"
				: "text-muted-foreground hover:text-foreground",
		)}
	>
		{children}
	</button>
);

const FullscreenEditDialog = ({
	open,
	onOpenChange,
	content,
	onSave,
}: {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	content: string;
	onSave: (content: string) => void;
}) => {
	const [editedContent, setEditedContent] = useState(content);

	useEffect(() => {
		if (open) {
			setEditedContent(content);
			document.body.style.overflow = "hidden";
		} else {
			document.body.style.overflow = "";
		}

		return () => {
			document.body.style.overflow = "";
		};
	}, [open, content]);

	const handleSave = useCallback(() => {
		onSave(editedContent);
		onOpenChange(false);
	}, [editedContent, onSave, onOpenChange]);

	const handleCancel = useCallback(() => {
		setEditedContent(content);
		onOpenChange(false);
	}, [content, onOpenChange]);

	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === "Escape") {
				handleCancel();
			}
			if (e.key === "s" && (e.metaKey || e.ctrlKey)) {
				e.preventDefault();
				handleSave();
			}
		};

		if (open) {
			document.addEventListener("keydown", handleKeyDown);
		}

		return () => {
			document.removeEventListener("keydown", handleKeyDown);
		};
	}, [open, handleCancel, handleSave]);

	if (!open) return null;

	return (
		<div className="absolute inset-0 z-50 bg-background flex flex-col">
			<div className="flex items-center justify-between px-6 py-4 border-b bg-background">
				<h2 className="text-xl font-semibold">Edit Message</h2>
				<div className="flex gap-2">
					<Button variant="outline" onClick={handleCancel}>
						Cancel
					</Button>
					<Button onClick={handleSave}>
						<CheckIcon className="w-4 h-4 mr-2" />
						Save Changes
					</Button>
				</div>
			</div>
			<div className="flex-1 p-6 ">
				<div className="relative h-full border border-border rounded-lg">
					<TextEditor
						initialContent={content}
						onChange={setEditedContent}
						isMarkdown={true}
						editable={true}
					/>
				</div>
			</div>
		</div>
	);
};

const FeedbackDialog = ({
	open,
	onOpenChange,
	initialComment,
	initialIncludeChatHistory,
	initialCanContact,
	onSubmit,
}: {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	initialComment: string;
	initialIncludeChatHistory: boolean;
	initialCanContact: boolean;
	onSubmit: (data: {
		comment: string;
		includeChatHistory: boolean;
		canContact: boolean;
	}) => void;
}) => {
	const [feedbackComment, setFeedbackComment] = useState(initialComment);
	const [includeChatHistory, setIncludeChatHistory] = useState(
		initialIncludeChatHistory,
	);
	const [canContact, setCanContact] = useState(initialCanContact);

	useEffect(() => {
		if (open) {
			setFeedbackComment(initialComment);
			setIncludeChatHistory(initialIncludeChatHistory);
			setCanContact(initialCanContact);
		}
	}, [open, initialComment, initialIncludeChatHistory, initialCanContact]);

	const handleSubmit = useCallback(() => {
		onSubmit({ comment: feedbackComment, includeChatHistory, canContact });
		onOpenChange(false);
	}, [feedbackComment, includeChatHistory, canContact, onSubmit, onOpenChange]);

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-[500px]">
				<DialogHeader>
					<DialogTitle className="flex items-center gap-2">
						<MessageSquareIcon className="w-5 h-5 text-primary" />
						Share Additional Feedback
					</DialogTitle>
					<DialogDescription>
						Help us improve by sharing more details about your experience with
						this response.
					</DialogDescription>
				</DialogHeader>

				<div className="space-y-4 py-4">
					<div className="space-y-2">
						<Label>Your feedback</Label>
						<Textarea
							placeholder="Tell us what you think about this response..."
							value={feedbackComment}
							onChange={(e) => setFeedbackComment(e.target.value)}
							className="min-h-[100px] resize-none"
						/>
					</div>

					<div className="space-y-3">
						<div className="flex items-center space-x-2">
							<Switch
								id="chat-history"
								checked={includeChatHistory}
								onCheckedChange={setIncludeChatHistory}
							/>
							<Label htmlFor="chat-history">
								Include chat history with feedback
							</Label>
						</div>

						<div className="flex items-center space-x-2">
							<Switch
								id="can-contact"
								checked={canContact}
								onCheckedChange={setCanContact}
							/>
							<Label htmlFor="can-contact">
								You may contact me about this feedback
							</Label>
						</div>
					</div>
				</div>

				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>
						Cancel
					</Button>
					<Button onClick={handleSubmit}>Submit Feedback</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
};

const MessageActions = ({
	isUser,
	rating,
	gaveMoreFeedback,
	onThumbsUp,
	onThumbsDown,
	onFeedbackClick,
	onEdit,
	onCopy,
}: {
	isUser: boolean;
	rating: number;
	gaveMoreFeedback: boolean;
	onThumbsUp: () => void;
	onThumbsDown: () => void;
	onFeedbackClick: () => void;
	onEdit: () => void;
	onCopy: () => void;
}) => (
	<div
		className={cn(
			"flex flex-row items-center gap-3 h-6 w-full mt-2",
			isUser ? "justify-end px-2 pt-2" : "justify-start",
		)}
	>
		{!isUser && (
			<>
				<FeedbackButton onClick={onThumbsUp} isActive={rating > 0}>
					<ThumbsUpIcon className="w-4 h-4" />
				</FeedbackButton>
				<FeedbackButton onClick={onThumbsDown} isActive={rating < 0}>
					<ThumbsDownIcon className="w-4 h-4" />
				</FeedbackButton>
			</>
		)}
		{rating !== 0 && (
			<button onClick={onFeedbackClick}>
				<Badge
					variant={gaveMoreFeedback ? "outline-solid" : "default"}
					className="h-6 rounded-full"
				>
					{gaveMoreFeedback ? "✅ Feedback provided" : "Provide feedback"}
				</Badge>
			</button>
		)}
		{!isUser && (
			<MessageActionButton onClick={onEdit} title="Edit message">
				<EditIcon className="w-4 h-4" />
			</MessageActionButton>
		)}
		<MessageActionButton onClick={onCopy} title="Copy message">
			<CopyIcon className="w-4 h-4" />
		</MessageActionButton>
	</div>
);

const AttachmentSection = ({
	files,
	onFileClick,
	onFullscreen,
}: {
	files: ProcessedAttachment[];
	onFileClick: (file: ProcessedAttachment) => void;
	onFullscreen?: (file: ProcessedAttachment) => void;
}) => {
	const { visibleAudio, visibleImages, visibleVideo, allHiddenFiles } =
		useMemo(() => {
			const audioFiles = files.filter((file) => file.type === "audio");
			const imageFiles = files.filter((file) => file.type === "image");
			const videoFiles = files.filter((file) => file.type === "video");
			const otherFiles = files.filter(
				(file) => !["audio", "image", "video"].includes(file.type),
			);

			return {
				visibleAudio: audioFiles.slice(0, 1),
				visibleImages: imageFiles.slice(0, 4),
				visibleVideo: videoFiles.slice(0, 1),
				allHiddenFiles: [
					...audioFiles.slice(1),
					...imageFiles.slice(4),
					...videoFiles.slice(1),
					...otherFiles,
				],
			};
		}, [files]);

	const getImageGridClassName = useCallback((count: number) => {
		if (count === 1) return "grid-cols-1";
		if (count === 2) return "grid-cols-2";
		if (count >= 3) return "grid-cols-2";
		return "grid-cols-1";
	}, []);

	return (
		<>
			{visibleAudio.length > 0 && (
				<div className="mt-2 max-w-md">
					{visibleAudio.map((file) => (
						<FilePreview key={file.url} file={file} onClick={onFileClick} />
					))}
				</div>
			)}

			{visibleImages.length > 0 && (
				<div
					className={cn(
						"mt-2 grid gap-1.5 max-w-md",
						getImageGridClassName(visibleImages.length),
					)}
				>
					{visibleImages.map((file) => (
						<FilePreview
							key={file.url}
							file={file}
							showFullscreenButton={true}
							onFullscreen={onFullscreen}
						/>
					))}
				</div>
			)}

			{visibleVideo.length > 0 && (
				<div className="mt-2 max-w-md">
					{visibleVideo.map((file) => (
						<FilePreview key={file.url} file={file} onClick={onFileClick} />
					))}
				</div>
			)}

			{allHiddenFiles.length > 0 && (
				<FileDialog files={files} handleFileClick={onFileClick} />
			)}
		</>
	);
};

export function MessageComponent({
	message,
	loading,
	onMessageUpdate,
}: Readonly<MessageProps>) {
	const isUser = message.inner.role === IRole.User;
	const [isExpanded, setIsExpanded] = useState(false);
	const [showToggle, setShowToggle] = useState(false);
	const [fullscreenFile, setFullscreenFile] =
		useState<ProcessedAttachment | null>(null);
	const [showFeedbackDialog, setShowFeedbackDialog] = useState(false);
	const [showEditDialog, setShowEditDialog] = useState(false);
	const contentRef = useRef<HTMLDivElement>(null);

	const maxCollapsedHeight = "4rem";

	const messageContent = useMemo(() => {
		if (typeof message.inner.content === "string") {
			return { text: message.inner.content, attachments: message.files ?? [] };
		}

		let text = "";
		const attachments: IAttachment[] = [];

		for (const part of message.inner.content) {
			if (part.text) {
				text += `${part.text}\n`;
				continue;
			}
			if (part.image_url?.url) attachments.push(part.image_url?.url);
		}

		return { text, attachments: [...attachments, ...(message.files ?? [])] };
	}, [message.inner.content, message.files]);

	const processedAttachments = useProcessedAttachments(
		messageContent.attachments,
	);

	useEffect(() => {
		if (isUser && contentRef.current) {
			const timer = setTimeout(() => {
				if (contentRef.current) {
					const actualHeight = contentRef.current.scrollHeight;
					const maxHeight = Number.parseFloat(maxCollapsedHeight) * 16;
					setShowToggle(actualHeight > maxHeight);
				}
			}, 100);
			return () => clearTimeout(timer);
		}
	}, [message.inner, isUser]);

	const handleFileClick = useCallback((file: ProcessedAttachment) => {
		if (file.isDataUrl) {
			if (file.type === "image") {
				const newWindow = window.open();
				if (newWindow) {
					newWindow.document.write(
						`<img src="${file.url}" style="max-width: 100%; height: auto;" />`,
					);
				}
			} else {
				const link = document.createElement("a");
				link.href = file.url;
				link.download = file.name || "file";
				document.body.appendChild(link);
				link.click();
				document.body.removeChild(link);
			}
		} else {
			window.open(file.url, "_blank", "noopener,noreferrer");
		}
	}, []);

	const copyToClipboard = useCallback(() => {
		if (messageContent.text) {
			navigator.clipboard
				.writeText(messageContent.text)
				.then(() => toast.success("Message copied to clipboard"))
				.catch((err) => console.error("Failed to copy message: ", err));
		}
	}, [messageContent.text]);

	const upsertFeedback = useCallback(
		(rating: number) => {
			if (!onMessageUpdate) return;

			const currentRating = message.rating ?? 0;
			const newRating = currentRating === rating ? 0 : rating;

			onMessageUpdate(message.id, {
				rating: newRating,
				ratingSettings: newRating === 0 ? undefined : message.ratingSettings,
			});

			toast.success("Thanks for the feedback! ❤️");
		},
		[message.id, message.rating, message.ratingSettings, onMessageUpdate],
	);

	const handleFeedbackSubmit = useCallback(
		(data: {
			comment: string;
			includeChatHistory: boolean;
			canContact: boolean;
		}) => {
			if (!onMessageUpdate) return;

			onMessageUpdate(message.id, {
				ratingSettings: {
					comment: data.comment.trim(),
					includeChatHistory: data.includeChatHistory,
					canContact: data.canContact,
				},
			});

			toast.success("Feedback submitted successfully!");
		},
		[message.id, onMessageUpdate],
	);

	const handleEditSave = useCallback(
		(content: string) => {
			if (!onMessageUpdate) return;

			const trimmedContent = content.trim();
			if (trimmedContent !== messageContent.text) {
				onMessageUpdate(message.id, {
					inner: {
						...message.inner,
						content: trimmedContent,
					},
				});
				toast.success("Message updated successfully!");
			}
		},
		[messageContent.text, message.id, message.inner, onMessageUpdate],
	);

	const gaveMoreFeedback = useMemo(() => {
		return Boolean(
			message.ratingSettings &&
				(message.ratingSettings.comment ||
					message.ratingSettings.includeChatHistory ||
					message.ratingSettings.canContact),
		);
	}, [message.ratingSettings]);

	return (
		<>
			<div
				className={cn(
					"max-w-(--breakpoint-lg) flex gap-1 flex-col transition-all duration-300 ease-in-out",
					isUser ? "items-end" : "items-start",
				)}
			>
				<div
					className={cn(
						"rounded-xl rounded-tr-sm p-4 pt-2 whitespace-break-spaces transition-all duration-300 ease-in-out",
						isUser
							? "bg-muted dark:bg-muted/30 text-foreground max-w-(--breakpoint-md)"
							: "bg-background text-foreground max-w-full w-full pb-0",
					)}
				>
					<div
						ref={contentRef}
						className={cn(
							"text-sm leading-relaxed whitespace-break-spaces text-wrap max-w-full w-full",
							isUser && !isExpanded && "overflow-hidden",
						)}
						style={
							isUser && !isExpanded
								? { maxHeight: maxCollapsedHeight }
								: undefined
						}
					>
						<TextEditor
							initialContent={
								messageContent.text === "" && loading
									? "🚀 Sending Message..."
									: messageContent.text
							}
							isMarkdown={true}
							editable={false}
						/>
					</div>

					{isUser && showToggle && (
						<Button
							variant="ghost"
							size="sm"
							onClick={() => setIsExpanded(!isExpanded)}
							className="h-auto p-0 text-xs text-foreground hover:text-foreground/80 mt-1"
						>
							{isExpanded ? (
								<>
									<ChevronUp className="w-3 h-3 mr-1" />
									Show less
								</>
							) : (
								<>
									<ChevronDown className="w-3 h-3 mr-1" />
									Show more
								</>
							)}
						</Button>
					)}

					<AttachmentSection
						files={processedAttachments}
						onFileClick={handleFileClick}
						onFullscreen={setFullscreenFile}
					/>

					{!loading && (
						<MessageActions
							isUser={isUser}
							rating={message.rating ?? 0}
							gaveMoreFeedback={gaveMoreFeedback}
							onThumbsUp={() => upsertFeedback(1)}
							onThumbsDown={() => upsertFeedback(-1)}
							onFeedbackClick={() => setShowFeedbackDialog(true)}
							onEdit={() => setShowEditDialog(true)}
							onCopy={copyToClipboard}
						/>
					)}
				</div>
			</div>

			{fullscreenFile && (
				<Dialog
					open={!!fullscreenFile}
					onOpenChange={() => setFullscreenFile(null)}
				>
					<DialogContent className="max-w-[90vw] max-h-[90vh] p-0 bg-black">
						<div className="relative h-full">
							<Button
								variant="ghost"
								size="sm"
								onClick={() => setFullscreenFile(null)}
								className="absolute top-4 right-4 z-10 bg-black/50 text-white hover:bg-black/70 rounded-full h-8 w-8 p-0"
							>
								<X className="w-4 h-4" />
							</Button>
							<div className="p-4">
								<FileDialogPreview file={fullscreenFile} />
							</div>
							<div className="absolute bottom-4 left-4 right-4 text-center">
								<p className="text-white text-sm bg-black/50 rounded px-2 py-1 inline-block">
									{fullscreenFile.name}
								</p>
							</div>
						</div>
					</DialogContent>
				</Dialog>
			)}

			<FullscreenEditDialog
				open={showEditDialog}
				onOpenChange={setShowEditDialog}
				content={messageContent.text}
				onSave={handleEditSave}
			/>

			<FeedbackDialog
				open={showFeedbackDialog}
				onOpenChange={setShowFeedbackDialog}
				initialComment={message.ratingSettings?.comment ?? ""}
				initialIncludeChatHistory={
					message.ratingSettings?.includeChatHistory ?? false
				}
				initialCanContact={message.ratingSettings?.canContact ?? false}
				onSubmit={handleFeedbackSubmit}
			/>
		</>
	);
}
