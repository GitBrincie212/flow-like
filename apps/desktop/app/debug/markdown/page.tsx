"use client";

import { TextEditor } from "@tm9657/flow-like-ui";
import { type ChangeEvent, useState } from "react";

export default function MarkdownDebugPage() {
	const [markdown, setMarkdown] = useState("");

	return (
		<div className="grid h-full grid-cols-2 overflow-y-auto border-t">
			<div className="border-r">
				<textarea
					value={markdown}
					onChange={(e: ChangeEvent<HTMLTextAreaElement>) =>
						setMarkdown(e.target.value)
					}
					className="h-full w-full resize-none bg-muted/50 p-2 font-mono text-sm outline-none"
					// disabled={!editable}
					placeholder="Enter your markdown here..."
				/>
			</div>
			<TextEditor key={markdown} isMarkdown={true} initialContent={markdown} />
		</div>
	);
}
