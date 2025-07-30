"use client";

import { createId } from "@paralleldrive/cuid2";
import {
	Button,
	Card,
	CardContent,
	CardHeader,
	CardTitle,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
	EventForm,
	EventTranslation,
	EventTypeConfiguration,
	type IEvent,
	type IEventMapping,
	Input,
	Label,
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
	Textarea,
	VariableConfigCard,
	VariableTypeIndicator,
	useBackend,
	useInvoke,
} from "@tm9657/flow-like-ui";
import {
	convertJsonToUint8Array,
	parseUint8ArrayToJson,
} from "@tm9657/flow-like-ui/lib/uint8";
import {
	ActivityIcon,
	CodeIcon,
	CogIcon,
	EditIcon,
	ExternalLinkIcon,
	FileTextIcon,
	GitBranchIcon,
	LayersIcon,
	Pause,
	Play,
	Plus,
	SaveIcon,
	Settings,
	StickyNote,
	Trash2,
} from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
type ViewMode = "cards" | "table";

export default function EventsPage({
	eventMapping,
}: Readonly<{ eventMapping: IEventMapping }>) {
	const searchParams = useSearchParams();
	const id = searchParams.get("id");
	const eventId = searchParams.get("eventId");

	const backend = useBackend();
	const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false);
	const [editingEvent, setEditingEvent] = useState<IEvent | null>(null);

	const router = useRouter();
	const events = useInvoke(
		backend.eventState.getEvents,
		backend.eventState,
		[id ?? ""],
		(id ?? "") !== "",
	);

	const boards = useInvoke(
		backend.boardState.getBoards,
		backend.boardState,
		[id ?? ""],
		(id ?? "") !== "",
	);

	const boardsMap = useMemo(() => {
		const map = new Map<string, string>();
		boards.data?.forEach((board) => map.set(board.id, board.name));
		return map;
	}, [boards.data]);

	useEffect(() => {
		setEditingEvent(events.data?.find((event) => event.id === eventId) ?? null);
	}, [editingEvent, id, eventId, events.data]);

	const handleCreateEvent = async (newEvent: Partial<IEvent>) => {
		if (!id) {
			console.error("App ID is required to create an event");
			return;
		}

		const event: IEvent = {
			id: createId(),
			name: newEvent.name ?? "New Event",
			description: newEvent.description ?? "",
			active: true,
			board_id: newEvent.board_id ?? "",
			board_version: newEvent.board_version ?? undefined,
			config: newEvent.config ?? [],
			created_at: {
				secs_since_epoch: Math.floor(Date.now() / 1000),
				nanos_since_epoch: 0,
			},
			updated_at: {
				secs_since_epoch: Math.floor(Date.now() / 1000),
				nanos_since_epoch: 0,
			},
			event_version: [0, 0, 0],
			node_id: newEvent.node_id ?? "",
			variables: newEvent.variables ?? {},
			event_type: newEvent.event_type ?? "default",
			priority: events.data?.length ?? 0,
			canary: null,
			notes: null,
		};
		await backend.eventState.upsertEvent(id, event);
		await events.refetch();
		setIsCreateDialogOpen(false);
	};

	const handleDeleteEvent = async (eventId: string) => {
		if (!id) {
			console.error("App ID is required to delete an event");
			return;
		}
		await backend.eventState.deleteEvent(id, eventId);
		if (editingEvent?.id === eventId) {
			setEditingEvent(null);
		}
		console.log(`Deleted event with ID: ${eventId}`);
		await events.refetch();
	};

	const handleEditingEvent = (event?: IEvent) => {
		let additionalParams = "";
		if (event?.id) {
			additionalParams = `&eventId=${event.id}`;
		}

		router.push(`/library/config/events?id=${id}${additionalParams}`);
	};

	if (id && editingEvent) {
		return (
			<EventConfiguration
				eventMapping={eventMapping}
				appId={id}
				event={editingEvent}
				onDone={() => handleEditingEvent()}
				onReload={async () => {
					await events.refetch();
				}}
			/>
		);
	}

	return (
		<div className="container mx-auto flex flex-col grow max-h-full">
			<div className="flex flex-col grow overflow-hidden max-h-full">
				<div className="flex flex-col overflow-auto overflow-x-visible grow h-full max-h-full">
					{events.data?.length === 0 ? (
						<Card>
							<CardContent className="py-12 text-center">
								<Settings className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
								<h3 className="text-lg font-semibold mb-2">
									No events configured
								</h3>
								<p className="text-muted-foreground mb-4">
									Get started by creating your first event
								</p>
								<Button
									onClick={() => setIsCreateDialogOpen(true)}
									className="gap-2"
								>
									<Plus className="h-4 w-4" />
									Create Event
								</Button>
							</CardContent>
						</Card>
					) : (
						<EventsTable
							events={events.data ?? []}
							boardsMap={boardsMap}
							onEdit={handleEditingEvent}
							onDelete={handleDeleteEvent}
							onNavigateToNode={(event, nodeId) => {
								router.push(
									`/flow?id=${event.board_id}&app=${id}&node=${nodeId}${event.board_version ? `&version=${event.board_version.join("_")}` : ""}`,
								);
							}}
							onCreateEvent={() => setIsCreateDialogOpen(true)}
						/>
					)}
				</div>
			</div>

			<Dialog open={isCreateDialogOpen} onOpenChange={setIsCreateDialogOpen}>
				<DialogContent className="max-w-2xl">
					<DialogHeader>
						<DialogTitle>Create New Event</DialogTitle>
						<DialogDescription>
							Configure a new event with its properties and settings
						</DialogDescription>
					</DialogHeader>
					{id && (
						<EventForm
							eventConfig={eventMapping}
							appId={id}
							onSubmit={handleCreateEvent}
							onCancel={() => setIsCreateDialogOpen(false)}
						/>
					)}
				</DialogContent>
			</Dialog>
		</div>
	);
}

function EventConfiguration({
	eventMapping,
	event,
	appId,
	onDone,
	onReload,
}: Readonly<{
	eventMapping: IEventMapping;
	event: IEvent;
	appId: string;
	onDone?: () => void;
	onReload?: () => void;
}>) {
	const backend = useBackend();
	const [isEditing, setIsEditing] = useState(false);
	const [formData, setFormData] = useState<IEvent>(event);

	const boards = useInvoke(
		backend.boardState.getBoards,
		backend.boardState,
		[appId],
		!!appId && isEditing,
	);
	const board = useInvoke(
		backend.boardState.getBoard,
		backend.boardState,
		[
			appId,
			formData.board_id,
			event.board_version as [number, number, number] | undefined,
		],
		!!event.board_id,
	);
	const versions = useInvoke(
		backend.boardState.getBoardVersions,
		backend.boardState,
		[appId, formData.board_id],
		(formData.board_id ?? "") !== "" && isEditing,
	);

	const handleInputChange = (field: keyof IEvent, value: any) => {
		console.dir({
			field,
			value,
		});
		setFormData((prev) => ({ ...prev, [field]: value }));
	};

	const handleSave = async () => {
		await backend.eventState.upsertEvent(appId, formData);
		onReload?.();
		setIsEditing(false);
	};

	const handleCancel = () => {
		setFormData(event);
		setIsEditing(false);
	};

	return (
		<div className="container mx-auto py-8 space-y-8 max-h-full flex flex-col grow overflow-y-auto">
			{/* Breadcrumbs */}
			<div className="flex items-center space-x-2 text-sm text-muted-foreground">
				<Button
					variant="ghost"
					size="sm"
					onClick={onDone}
					className="p-0 h-auto font-normal hover:text-foreground"
				>
					Event Configuration
				</Button>
				<span>/</span>
				<span className="text-foreground font-medium">{event.name}</span>
			</div>

			{/* Header */}
			<div
				className={`flex items-center justify-between ${isEditing ? "sticky top-0 bg-background z-10 py-4 border-b shadow-sm" : ""}`}
			>
				<div className="space-y-1">
					<h1 className="text-3xl font-bold tracking-tight flex items-center gap-3">
						<Settings className="h-8 w-8" />
						{event.name}
						{isEditing && (
							<span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-orange-100 text-orange-800">
								Editing
							</span>
						)}
					</h1>
					<p className="text-muted-foreground">
						Configure event properties and settings
					</p>
				</div>
				<div className="flex items-center gap-2">
					{isEditing ? (
						<>
							<Button variant="outline" onClick={handleCancel}>
								Cancel
							</Button>
							<Button
								onClick={handleSave}
								className="gap-2 bg-orange-600 hover:bg-orange-700"
							>
								<SaveIcon className="h-4 w-4" />
								Save Changes
							</Button>
						</>
					) : (
						<Button onClick={() => setIsEditing(true)} className="gap-2">
							<EditIcon className="h-4 w-4" />
							Edit Event
						</Button>
					)}
				</div>
			</div>
			{/* Floating Save Button for mobile/small screens */}
			{isEditing && (
				<div className="fixed bottom-6 right-6 flex items-center gap-2 z-50 md:hidden">
					<Button
						variant="outline"
						onClick={handleCancel}
						className="shadow-lg"
					>
						Cancel
					</Button>
					<Button
						onClick={handleSave}
						className="gap-2 shadow-lg bg-orange-600 hover:bg-orange-700"
					>
						<SaveIcon className="h-4 w-4" />
						Save Changes
					</Button>
				</div>
			)}

			{/* Status Card */}
			<Card>
				<CardHeader>
					<CardTitle className="flex items-center gap-2">
						<ActivityIcon className="h-5 w-5" />
						Event Status
					</CardTitle>
				</CardHeader>
				<CardContent className="flex flex-col space-y-4">
					<div>
						{board.data?.nodes?.[formData.node_id] && formData.node_id && (
							<EventTypeConfiguration
								eventConfig={eventMapping}
								disabled={!isEditing}
								node={board.data?.nodes?.[formData.node_id]}
								event={formData}
								onUpdate={(type) => {
									handleInputChange("event_type", type);
								}}
							/>
						)}
					</div>
					<div className="flex items-center justify-between">
						<div className="flex items-center gap-3">
							<div
								className={`w-3 h-3 rounded-full ${event.active ? "bg-green-500" : "bg-orange-500"}`}
							/>
							<span className="font-medium">
								{event.active ? "Active" : "Inactive"}
							</span>
						</div>
						{isEditing && (
							<Button
								variant="outline"
								size="sm"
								onClick={() => handleInputChange("active", !formData.active)}
								className="gap-2"
							>
								{formData.active ? (
									<>
										<Pause className="h-4 w-4" />
										Deactivate
									</>
								) : (
									<>
										<Play className="h-4 w-4" />
										Activate
									</>
								)}
							</Button>
						)}
					</div>
				</CardContent>
			</Card>

			{/* Main Configuration */}
			<div className="space-y-8">
				{/* Top Row - Essential Information */}
				<div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
					{/* Basic Information */}
					<Card>
						<CardHeader>
							<CardTitle className="flex items-center gap-2">
								<FileTextIcon className="h-5 w-5" />
								Basic Information
							</CardTitle>
						</CardHeader>
						<CardContent className="space-y-4">
							<div>
								<Label>Event Name</Label>
								{isEditing ? (
									<Input
										type="text"
										value={formData.name}
										onChange={(e) => handleInputChange("name", e.target.value)}
									/>
								) : (
									<p className="mt-1 text-sm text-muted-foreground">
										{event.name}
									</p>
								)}
							</div>
							<div>
								<Label>Description</Label>
								{isEditing ? (
									<Textarea
										value={formData.description}
										onChange={(e) =>
											handleInputChange("description", e.target.value)
										}
										rows={3}
									/>
								) : (
									<p className="mt-1 text-sm text-muted-foreground">
										{event.description || "No description provided"}
									</p>
								)}
							</div>
							<div>
								<Label>Event ID</Label>
								<p className="mt-1 text-sm text-muted-foreground font-mono">
									{event.id}
								</p>
							</div>
						</CardContent>
					</Card>

					{/* Flow Configuration */}
					<Card>
						<CardHeader>
							<CardTitle className="flex items-center gap-2">
								<LayersIcon className="h-5 w-5" />
								Flow Configuration
							</CardTitle>
						</CardHeader>
						{!isEditing && (
							<CardContent className="space-y-4">
								<div>
									<Label>Flow</Label>
									<p className="mt-1 text-sm text-muted-foreground font-mono">
										{board.data?.name ?? "BOARD NOT FOUND!"}
									</p>
								</div>
								<div>
									<Label>Flow Version</Label>
									<p className="mt-1 text-sm text-muted-foreground">
										{event.board_version
											? event.board_version.join(".")
											: "Latest"}
									</p>
								</div>
								<div>
									<Label className="group flex items-center hover:underline">
										<Link
											title="Open Flow and Node"
											className="flex flex-row items-center"
											href={`/flow?id=${event.board_id}&app=${appId}&node=${event.node_id}${event.board_version ? `&version=${event.board_version.join("_")}` : ""}`}
										>
											Node ID
											<Button
												size={"icon"}
												variant={"ghost"}
												className="p-0! w-4 h-4 ml-1 mb-[0.1rem]"
											>
												<ExternalLinkIcon className="w-4 h-4 group-hover:text-primary" />
											</Button>
										</Link>
									</Label>
									<p className="mt-1 text-sm text-muted-foreground font-mono">
										{board.data?.nodes?.[event.node_id]?.friendly_name ??
											"Node not found"}{" "}
										({event.node_id})
									</p>
								</div>
							</CardContent>
						)}
						{isEditing && (
							<CardContent className="space-y-4">
								{/* Board Selection */}
								<div className="space-y-4">
									<div className="space-y-2">
										<Label htmlFor="board">Flow</Label>
										<Select
											value={formData.board_id}
											onValueChange={(value) => {
												handleInputChange("board_id", value);
												handleInputChange("board_version", undefined);
												handleInputChange("node_id", undefined);
											}}
										>
											<SelectTrigger>
												<SelectValue placeholder="Select a board" />
											</SelectTrigger>
											<SelectContent>
												{boards.data?.map((board) => (
													<SelectItem key={board.id} value={board.id}>
														{board.name}
													</SelectItem>
												))}
											</SelectContent>
										</Select>
									</div>
								</div>
								{/* Board Version Selection */}
								<div className="space-y-4">
									<div className="space-y-2">
										<Label htmlFor="board">Flow Version</Label>
										<Select
											value={formData.board_version?.join(".") ?? ""}
											onValueChange={(value) => {
												handleInputChange(
													"board_version",
													value === "" || value === "none"
														? undefined
														: value.split(".").map(Number),
												);
												handleInputChange("node_id", undefined);
											}}
										>
											<SelectTrigger>
												<SelectValue placeholder="Latest" />
											</SelectTrigger>
											<SelectContent>
												{versions.data?.map((board) => (
													<SelectItem
														key={board.join(".")}
														value={board.join(".")}
													>
														v{board.join(".")}
													</SelectItem>
												))}
												<SelectItem key={""} value={"none"}>
													Latest
												</SelectItem>
											</SelectContent>
										</Select>
									</div>
								</div>

								{/* Node and Board Selection */}
								{board.data && (
									<div className="space-y-4">
										<div className="space-y-2">
											<Label htmlFor="node">Node</Label>
											<Select
												value={formData.node_id}
												onValueChange={(value) =>
													handleInputChange("node_id", value)
												}
											>
												<SelectTrigger>
													<SelectValue placeholder="Select a node" />
												</SelectTrigger>
												<SelectContent>
													{Object.values(board.data.nodes)
														.filter((node) => node.start)
														.map((node) => (
															<SelectItem key={node.id} value={node.id}>
																{node.friendly_name || node.name}
															</SelectItem>
														))}
												</SelectContent>
											</Select>
										</div>
									</div>
								)}
							</CardContent>
						)}
					</Card>
				</div>

				{/* Version Information - Single row for metadata */}
				<Card>
					<CardHeader>
						<CardTitle className="flex items-center gap-2">
							<GitBranchIcon className="h-5 w-5" />
							Version Information
						</CardTitle>
					</CardHeader>
					<CardContent>
						<div className="grid grid-cols-1 md:grid-cols-3 gap-4">
							<div>
								<Label>Event Version</Label>
								<p className="mt-1 text-sm text-muted-foreground">
									{event.event_version.join(".")}
								</p>
							</div>
							<div>
								<Label>Created</Label>
								<p className="mt-1 text-sm text-muted-foreground">
									{new Date(
										event.created_at.secs_since_epoch * 1000,
									).toLocaleString()}
								</p>
							</div>
							<div>
								<Label>Last Updated</Label>
								<p className="mt-1 text-sm text-muted-foreground">
									{new Date(
										event.updated_at.secs_since_epoch * 1000,
									).toLocaleString()}
								</p>
							</div>
						</div>
					</CardContent>
				</Card>

				{/* Variables - Full width due to potential size */}
				<Card>
					<CardHeader>
						<CardTitle className="flex flex-row items-center gap-2">
							<CodeIcon className="h-5 w-5" />
							<p>Variables</p>
							{isEditing && (
								<Dialog>
									<DialogTrigger asChild>
										<Button variant="outline" className="gap-2 ml-2">
											<Plus className="h-4 w-4" />
											Add Flow Variables
										</Button>
									</DialogTrigger>
									<DialogContent className="max-w-lg">
										<DialogHeader>
											<DialogTitle>Add Flow Variables</DialogTitle>
											<DialogDescription>
												Select flow variables to override in this event
												configuration
											</DialogDescription>
										</DialogHeader>
										<div className="space-y-2 max-h-80 overflow-y-auto">
											{board.data?.variables &&
												Object.entries(board.data.variables)
													.filter(([key, variable]) => variable.exposed)
													.map(([key, variable]) => {
														const isAlreadyAdded =
															formData.variables.hasOwnProperty(key);
														return (
															<div
																key={key}
																className="flex items-center justify-between p-3 border rounded"
															>
																<div className="flex-1">
																	<div className="flex flex-row items-center gap-2">
																		<VariableTypeIndicator
																			valueType={variable.data_type}
																			type={variable.value_type}
																		/>
																		<div className="font-medium text-sm">
																			{variable.name}
																		</div>
																	</div>
																	{variable.default_value && (
																		<div className="text-xs text-muted-foreground mt-1">
																			Default:{" "}
																			<span>
																				{String(
																					parseUint8ArrayToJson(
																						variable.default_value,
																					),
																				)}
																			</span>
																		</div>
																	)}
																</div>
																<Button
																	variant={
																		isAlreadyAdded ? "outline" : "default"
																	}
																	size="sm"
																	onClick={() => {
																		if (isAlreadyAdded) {
																			const newVars = { ...formData.variables };
																			delete newVars[key];
																			handleInputChange("variables", newVars);
																		} else {
																			handleInputChange("variables", {
																				...formData.variables,
																				[key]: variable,
																			});
																		}
																	}}
																>
																	{isAlreadyAdded ? "Remove" : "Add"}
																</Button>
															</div>
														);
													})}
											{(!board.data?.variables ||
												Object.keys(board.data.variables).length === 0) && (
												<div className="text-center py-8 text-muted-foreground">
													No board variables available
												</div>
											)}
										</div>
									</DialogContent>
								</Dialog>
							)}
						</CardTitle>
					</CardHeader>
					<CardContent>
						{Object.keys(formData.variables).length > 0 ? (
							<div className="space-y-2">
								{Object.entries(formData.variables).map(([key, value]) => (
									<VariableConfigCard
										disabled={!isEditing}
										key={key}
										variable={value}
										onUpdate={async (variable) => {
											if (!isEditing) setIsEditing(true);
											const newVars = {
												...formData.variables,
												[key]: {
													...variable,
													default_value: variable.default_value,
												},
											};
											handleInputChange("variables", newVars);
										}}
									/>
								))}
							</div>
						) : (
							<p className="text-sm text-muted-foreground">
								{isEditing
									? "No variables configured. Click 'Add Board Variables' to get started."
									: "No variables configured"}
							</p>
						)}
					</CardContent>
				</Card>

				{/* Node Specific Configuration - Full width due to potential size */}
				{board.data && (
					<Card>
						<CardHeader>
							<CardTitle className="flex items-center gap-2">
								<CogIcon className="h-5 w-5" />
								Node Configuration
							</CardTitle>
						</CardHeader>
						<CardContent className="space-y-4 flex flex-col items-start">
							<EventTranslation
								eventType={formData.event_type}
								eventConfig={eventMapping}
								editing={isEditing}
								config={parseUint8ArrayToJson(event.config ?? []) ?? {}}
								board={board.data}
								nodeId={formData.node_id}
								onUpdate={(config) => {
									console.dir(config);
									if (!isEditing) setIsEditing(true);
									handleInputChange("config", convertJsonToUint8Array(config));
								}}
							/>
						</CardContent>
					</Card>
				)}

				{/* Notes Section - Full width at bottom */}
				{(event.notes || isEditing) && (
					<Card>
						<CardHeader>
							<CardTitle className="flex items-center gap-2">
								<StickyNote className="h-5 w-5" />
								Notes
							</CardTitle>
						</CardHeader>
						<CardContent>
							{isEditing ? (
								<Textarea
									value={formData.notes?.NOTES ?? ""}
									onChange={(e) => handleInputChange("notes", e.target.value)}
									placeholder="Add notes about this event..."
									rows={4}
								/>
							) : (
								<p className="text-sm text-muted-foreground whitespace-pre-wrap">
									{event.notes?.NOTES ?? "No notes added"}
								</p>
							)}
						</CardContent>
					</Card>
				)}
			</div>
		</div>
	);
}

interface IEventsTableProps {
	events: IEvent[];
	boardsMap: Map<string, string>;
	onEdit: (event: IEvent) => void;
	onDelete: (eventId: string) => void;
	onNavigateToNode: (event: IEvent, nodeId: string) => void;
	onCreateEvent: () => void;
}

function EventsTable({
	events,
	boardsMap,
	onEdit,
	onDelete,
	onNavigateToNode,
	onCreateEvent,
}: Readonly<IEventsTableProps>) {
	const [currentPage, setCurrentPage] = useState(1);
	const [pageSize, setPageSize] = useState(50);
	const [searchTerm, setSearchTerm] = useState("");

	const filteredEvents = useMemo(() => {
		if (!searchTerm) return events;
		return events.filter(
			(event) =>
				event.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
				event.description.toLowerCase().includes(searchTerm.toLowerCase()) ||
				event.event_type.toLowerCase().includes(searchTerm.toLowerCase()) ||
				(boardsMap.get(event.board_id) ?? "")
					.toLowerCase()
					.includes(searchTerm.toLowerCase()),
		);
	}, [events, searchTerm, boardsMap]);

	const totalPages = Math.ceil(filteredEvents.length / pageSize);
	const startIndex = (currentPage - 1) * pageSize;
	const paginatedEvents = filteredEvents.slice(
		startIndex,
		startIndex + pageSize,
	);

	const formatRelativeTime = (timestamp: number) => {
		const now = Date.now();
		const eventTime = timestamp * 1000;
		const diffMs = now - eventTime;
		const diffHours = diffMs / (1000 * 60 * 60);
		const diffDays = diffMs / (1000 * 60 * 60 * 24);

		if (diffHours < 24) {
			return `${Math.floor(diffHours)}h ago`;
		}
		if (diffDays < 7) {
			return `${Math.floor(diffDays)}d ago`;
		}
		return new Date(eventTime).toLocaleDateString();
	};

	const truncateText = (text: string, maxLength = 50) => {
		return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
	};

	useEffect(() => {
		setCurrentPage(1);
	}, [searchTerm]);

	return (
		<div className="flex flex-col h-full min-h-0">
			<div className="flex items-center justify-between gap-4 mb-4 flex-shrink-0">
				<div className="flex items-center gap-2">
					<Input
						placeholder="Search events..."
						value={searchTerm}
						onChange={(e) => setSearchTerm(e.target.value)}
						className="w-64"
					/>
					<div className="text-sm text-muted-foreground">
						{filteredEvents.length} of {events.length} events
					</div>
				</div>
				<div className="flex items-center gap-2">
					<Button onClick={onCreateEvent} className="gap-2">
						<Plus className="h-4 w-4" />
						Create Event
					</Button>
					<Label htmlFor="pageSize" className="text-sm">
						Show:
					</Label>
					<Select
						value={pageSize.toString()}
						onValueChange={(value) => setPageSize(Number(value))}
					>
						<SelectTrigger className="w-20">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="25">25</SelectItem>
							<SelectItem value="50">50</SelectItem>
							<SelectItem value="100">100</SelectItem>
							<SelectItem value="200">200</SelectItem>
						</SelectContent>
					</Select>
				</div>
			</div>

			<div className="flex-1 min-h-0 rounded-md overflow-hidden flex flex-col">
				<div className="flex-1 min-h-0 overflow-auto">
					<Table>
						<TableHeader className="sticky top-0 bg-background z-10 border-b">
							<TableRow>
								<TableHead className="w-12">Status</TableHead>
								<TableHead className="min-w-[200px]">Name</TableHead>
								<TableHead className="min-w-[300px]">Description</TableHead>
								<TableHead className="min-w-[150px]">Flow</TableHead>
								<TableHead className="w-32">Event Type</TableHead>
								<TableHead className="w-32">Last Updated</TableHead>
								<TableHead className="w-24">Actions</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{paginatedEvents.map((event) => (
								<TableRow key={event.id} className="hover:bg-muted/50">
									<TableCell>
										<div className="flex items-center">
											<div
												className={`w-2 h-2 rounded-full ${
													event.active ? "bg-green-500" : "bg-orange-500"
												}`}
											/>
										</div>
									</TableCell>
									<TableCell>
										<div className="font-medium">{event.name}</div>
										<div className="text-xs text-muted-foreground font-mono">
											{event.id.slice(0, 8)}...
										</div>
									</TableCell>
									<TableCell>
										<div className="text-sm text-muted-foreground">
											{event.description
												? truncateText(event.description, 80)
												: "No description"}
										</div>
									</TableCell>
									<TableCell>
										<div className="text-sm">
											{boardsMap.get(event.board_id) ?? "Unknown"}
										</div>
										<div className="text-xs text-muted-foreground">
											{event.board_version
												? `v${event.board_version.join(".")}`
												: "Latest"}
										</div>
									</TableCell>
									<TableCell>
										<div className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-secondary text-secondary-foreground">
											{event.event_type}
										</div>
									</TableCell>
									<TableCell>
										<div className="text-sm text-muted-foreground">
											{formatRelativeTime(event.updated_at.secs_since_epoch)}
										</div>
									</TableCell>
									<TableCell>
										<div className="flex items-center gap-1">
											<Button
												variant="ghost"
												size="sm"
												onClick={() => onEdit(event)}
												className="h-8 w-8 p-0"
											>
												<EditIcon className="h-4 w-4" />
											</Button>
											<Button
												variant="ghost"
												size="sm"
												onClick={() => onNavigateToNode(event, event.node_id)}
												className="h-8 w-8 p-0"
											>
												<ExternalLinkIcon className="h-4 w-4" />
											</Button>
											<Button
												variant="ghost"
												size="sm"
												onClick={() => onDelete(event.id)}
												className="h-8 w-8 p-0 text-destructive hover:text-destructive"
											>
												<Trash2 className="h-4 w-4" />
											</Button>
										</div>
									</TableCell>
								</TableRow>
							))}
						</TableBody>
					</Table>
				</div>

				{totalPages > 1 && (
					<div className="border-t bg-background p-4 flex-shrink-0">
						<div className="flex items-center justify-between">
							<div className="text-sm text-muted-foreground">
								Showing {startIndex + 1} to{" "}
								{Math.min(startIndex + pageSize, filteredEvents.length)} of{" "}
								{filteredEvents.length} results
							</div>
							<div className="flex items-center gap-2">
								<Button
									variant="outline"
									size="sm"
									onClick={() =>
										setCurrentPage((prev) => Math.max(1, prev - 1))
									}
									disabled={currentPage === 1}
								>
									Previous
								</Button>
								<div className="flex items-center gap-1">
									{Array.from({ length: Math.min(5, totalPages) }, (_, i) => {
										let pageNum: number;
										if (totalPages <= 5) {
											pageNum = i + 1;
										} else if (currentPage <= 3) {
											pageNum = i + 1;
										} else if (currentPage >= totalPages - 2) {
											pageNum = totalPages - 4 + i;
										} else {
											pageNum = currentPage - 2 + i;
										}

										return (
											<Button
												key={pageNum}
												variant={
													currentPage === pageNum ? "default" : "outline"
												}
												size="sm"
												onClick={() => setCurrentPage(pageNum)}
												className="w-8 h-8 p-0"
											>
												{pageNum}
											</Button>
										);
									})}
								</div>
								<Button
									variant="outline"
									size="sm"
									onClick={() =>
										setCurrentPage((prev) => Math.min(totalPages, prev + 1))
									}
									disabled={currentPage === totalPages}
								>
									Next
								</Button>
							</div>
						</div>
					</div>
				)}
			</div>
		</div>
	);
}
