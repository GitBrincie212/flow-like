"use client";

import { useMemo, useRef, useState } from "react";
import type { IEvent, IEventPayloadChat } from "../../../lib";
import { ChatBox, type ChatBoxRef, type ISendMessageFunction } from "./chatbox";

interface ChatWelcomeProps {
	onSendMessage: ISendMessageFunction;
	event: IEvent;
	config?: Partial<IEventPayloadChat>;
}

const defaultExamples: string[] = [
	"Help me brainstorm ideas for a new project",
	"Explain how machine learning works",
	"Help me debug this code issue",
	"What's the latest in technology?",
	"Write a professional email",
	"Create a workout plan",
	"Explain quantum computing",
	"Help with meal planning",
];

export function ChatWelcome({
	onSendMessage,
	event,
	config = {},
}: Readonly<ChatWelcomeProps>) {
	const [currentMessage, setCurrentMessage] = useState("");
	const chatBox = useRef<ChatBoxRef>(null);

	// Fuzzy search function
	const fuzzyScore = (text: string, searchTerm: string): number => {
		const textLower = text.toLowerCase();
		const searchLower = searchTerm.toLowerCase();

		if (!searchLower) return 0;
		if (textLower.includes(searchLower)) return 100; // Exact substring match gets highest score

		let score = 0;
		let searchIndex = 0;
		let lastMatchIndex = -1;

		for (
			let i = 0;
			i < textLower.length && searchIndex < searchLower.length;
			i++
		) {
			if (textLower[i] === searchLower[searchIndex]) {
				// Award points for character match
				score += 10;

				// Bonus for consecutive matches
				if (lastMatchIndex === i - 1) {
					score += 5;
				}

				// Bonus for matches at word boundaries
				if (i === 0 || textLower[i - 1] === " ") {
					score += 3;
				}

				lastMatchIndex = i;
				searchIndex++;
			}
		}

		// Only return score if all search characters were found
		if (searchIndex === searchLower.length) {
			// Bonus for shorter text (more relevant matches)
			score += Math.max(0, 50 - textLower.length);
			return score;
		}

		return 0;
	};

	// Filter examples based on current message and show max 5
	const filteredExamples = useMemo(() => {
		const examples = config?.example_messages ?? defaultExamples;
		if (!currentMessage.trim()) {
			return examples.slice(0, 4);
		}

		const searchTerm = currentMessage.toLowerCase();

		// Score all examples and sort by relevance
		const scoredExamples = examples
			.map((example) => ({
				text: example,
				score: fuzzyScore(example, searchTerm),
			}))
			.filter((item) => item.score > 0)
			.sort((a, b) => b.score - a.score)
			.map((item) => item.text);

		return scoredExamples.slice(0, 5);
	}, [currentMessage, config?.example_messages, fuzzyScore]);

	// Function to highlight matching text with fuzzy highlighting
	const highlightMatch = (text: string, searchTerm: string) => {
		if (!searchTerm.trim()) return text;

		const textLower = text.toLowerCase();
		const searchLower = searchTerm.toLowerCase();

		// For exact substring matches, use the original highlighting
		if (textLower.includes(searchLower)) {
			const regex = new RegExp(`(${searchTerm})`, "gi");
			const parts = text.split(regex);

			return parts.map((part, index) =>
				regex.test(part) ? (
					<span
						key={part + index}
						className="bg-primary/20 text-primary rounded-sm"
					>
						{part}
					</span>
				) : (
					part
				),
			);
		}

		// For fuzzy matches, highlight individual matching characters
		const result: React.ReactNode[] = [];
		let searchIndex = 0;

		for (let i = 0; i < text.length && searchIndex < searchLower.length; i++) {
			const char = text[i];
			const isMatch = textLower[i] === searchLower[searchIndex];

			if (isMatch) {
				result.push(
					<span
						key={i}
						className="bg-primary/20 text-primary rounded-sm px-0.5"
					>
						{char}
					</span>,
				);
				searchIndex++;
			} else {
				result.push(char);
			}
		}

		// Add remaining characters
		if (result.length < text.length) {
			result.push(text.slice(result.length));
		}

		return result;
	};

	return (
		<div className="flex flex-col h-full grow bg-background">
			{/* Welcome Content */}
			<div className="flex-1 flex items-center justify-center p-8">
				<div className="max-w-2xl w-full space-y-8">
					{/* Header */}
					<div className="text-center space-y-4">
						<h1 className="text-3xl font-bold">{event.name}</h1>
						<p className="text-muted-foreground text-lg line-clamp-1">
							{event.description ?? "How can I assist you today?"}
						</p>
					</div>

					<div className="max-w-2xl mx-auto space-y-4">
						<ChatBox
							ref={chatBox}
							availableTools={config?.tools ?? []}
							defaultActiveTools={config?.default_tools ?? []}
							onSendMessage={onSendMessage}
							onContentChange={(content) => {
								setCurrentMessage(content);
							}}
							fileUpload={config?.allow_file_upload ?? false}
							audioInput={config?.allow_voice_input ?? true}
						/>

						{/* Example Prompts List */}
						{(filteredExamples.length > 0 || currentMessage.trim()) && (
							<div className="space-y-2 pt-2 px-2">
								{filteredExamples.length > 0 && (
									<p className="text-xs text-muted-foreground uppercase tracking-wide">
										Suggestions
									</p>
								)}
								<div className="space-y-1 min-h-[200px]">
									{filteredExamples.map((example, index) => (
										<button
											key={example + index}
											className="w-full text-left text-sm text-muted-foreground bg-muted/10 hover:text-foreground hover:bg-muted/50 rounded-md px-3 py-2 transition-colors cursor-pointer"
											onClick={() => chatBox.current?.setInput(example)}
										>
											{highlightMatch(example, currentMessage.trim())}
										</button>
									))}
								</div>
							</div>
						)}
					</div>
				</div>
			</div>
		</div>
	);
}
