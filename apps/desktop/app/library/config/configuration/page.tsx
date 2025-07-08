"use client";

import {
	Badge,
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
	Collapsible,
	CollapsibleContent,
	CollapsibleTrigger,
	type IBoard,
	type IVariable,
	Separator,
	VariableConfigCard,
	upsertVariableCommand,
	useBackend,
	useInvalidateInvoke,
	useInvoke,
} from "@tm9657/flow-like-ui";
import {
	ChevronDownIcon,
	ChevronRightIcon,
	SettingsIcon,
	WorkflowIcon,
} from "lucide-react";
import { useSearchParams } from "next/navigation";
import { useCallback, useMemo, useState } from "react";

export default function ConfigurationPage() {
	const backend = useBackend();
	const searchParams = useSearchParams();
	const id = searchParams.get("id");

	const boards = useInvoke(
		backend.boardState.getBoards,
		backend.boardState,
		[id ?? ""],
		typeof id === "string",
	);

	const configurableBoards = useMemo(() => {
		return (boards.data ?? [])
			.map((board) => ({
				board,
				variables: Object.values(board.variables)
					.filter((variable) => variable.exposed && variable.editable)
					.sort((a, b) => a.name.localeCompare(b.name)),
			}))
			.filter(({ variables }) => variables.length > 0)
			.sort((a, b) => a.board.name.localeCompare(b.board.name));
	}, [boards.data]);

	if (configurableBoards.length === 0) {
		return (
			<main className="justify-start flex flex-col items-start w-full flex-1 max-h-full overflow-y-auto flex-grow gap-4">
				<div className="border p-6 rounded-lg bg-card w-full max-w-2xl mx-auto text-center">
					<div className="w-16 h-16 mx-auto mb-4 bg-green-100 dark:bg-green-900/20 rounded-full flex items-center justify-center">
						<SettingsIcon className="w-8 h-8 text-green-600 dark:text-green-400" />
					</div>
					<h3 className="text-xl font-semibold mb-2">
						✅ No Configuration Needed
					</h3>
					<p className="text-muted-foreground">
						Your application doesn&apos;t have any configurable parameters.
						You&apos;re ready to go! 🚀
					</p>
				</div>
			</main>
		);
	}

	const totalVariables = configurableBoards.reduce(
		(sum, { variables }) => sum + variables.length,
		0,
	);

	return (
		<main className="justify-start flex flex-col items-start w-full flex-1 max-h-full overflow-y-auto flex-grow gap-6">
			<div className="w-full py-4 border-b z-50">
				<div className="flex items-center justify-between">
					<div>
						<h2 className="text-2xl font-bold">Configuration</h2>
						<p className="text-muted-foreground mt-1">
							Configure {totalVariables} parameter
							{totalVariables !== 1 ? "s" : ""} across{" "}
							{configurableBoards.length} board
							{configurableBoards.length !== 1 ? "s" : ""}
						</p>
					</div>
					<Badge variant="secondary" className="gap-1">
						<SettingsIcon className="w-3 h-3" />
						{totalVariables} Parameters
					</Badge>
				</div>
			</div>

			<div className="w-full space-y-6">
				{id &&
					configurableBoards.map(({ board, variables }) => (
						<BoardConfig
							key={board.id}
							appId={id}
							board={board}
							variables={variables}
						/>
					))}
			</div>
		</main>
	);
}

function BoardConfig({
	appId,
	board,
	variables,
}: Readonly<{
	appId: string;
	board: IBoard;
	variables: IVariable[];
}>) {
	const backend = useBackend();
	const invalidate = useInvalidateInvoke();
	const [isOpen, setIsOpen] = useState(true);

	const upsertVariable = useCallback(
		async (variable: IVariable) => {
			if (!appId) return;

			const command = upsertVariableCommand({
				variable: variable,
			});

			await backend.boardState.executeCommand(appId, board.id, command);
			await invalidate(backend.boardState.getBoard, [appId, board.id]);
			await invalidate(backend.boardState.getBoards, [appId]);
		},
		[appId, board.id, backend, invalidate],
	);

	return (
		<Card className="w-full">
			<Collapsible open={isOpen} onOpenChange={setIsOpen}>
				<CollapsibleTrigger asChild>
					<CardHeader className="hover:bg-muted/50 transition-colors cursor-pointer">
						<div className="flex items-center justify-between">
							<div className="flex items-center gap-3">
								<div className="w-10 h-10 rounded-lg bg-primary/10 flex items-center justify-center">
									<WorkflowIcon className="w-5 h-5 text-primary" />
								</div>
								<div>
									<CardTitle className="text-left">{board.name}</CardTitle>
									<CardDescription className="text-left">
										{variables.length} configurable parameter
										{variables.length !== 1 ? "s" : ""}
									</CardDescription>
								</div>
							</div>
							<div className="flex items-center gap-2">
								<Badge variant="outline" className="gap-1">
									{variables.length}{" "}
									{variables.length === 1 ? "parameter" : "parameters"}
								</Badge>
								{isOpen ? (
									<ChevronDownIcon className="w-4 h-4 text-muted-foreground" />
								) : (
									<ChevronRightIcon className="w-4 h-4 text-muted-foreground" />
								)}
							</div>
						</div>
					</CardHeader>
				</CollapsibleTrigger>

				<CollapsibleContent>
					<CardContent className="pt-0">
						<Separator className="mb-4" />
						<div className="grid gap-4">
							{variables.map((variable) => (
								<VariableConfigCard
									key={variable.id}
									variable={variable}
									onUpdate={upsertVariable}
								/>
							))}
						</div>
					</CardContent>
				</CollapsibleContent>
			</Collapsible>
		</Card>
	);
}
