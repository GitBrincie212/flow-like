"use client";
import {
	type IBoard,
	type IEvent,
	type IEventMapping,
	type IEventPayload,
	type INode,
	Label,
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@tm9657/flow-like-ui";
import { useEffect, useMemo, useState } from "react";

export function EventTypeConfiguration({
	eventConfig,
	node,
	event,
	disabled,
	onUpdate,
}: Readonly<{
	eventConfig: IEventMapping;
	node: INode;
	disabled: boolean;
	event: IEvent;
	onUpdate: (type: string, config: Partial<IEventPayload>) => void;
}>) {
	const foundConfig = eventConfig[node?.name];

	useEffect(() => {
		const eventTypes = eventConfig[node?.name];
		if (!eventTypes) {
			console.warn(`No event types configured for node: ${node?.name}`);
			return;
		}

		if (!eventTypes.eventTypes.includes(event.event_type)) {
			onUpdate(
				eventTypes.defaultEventType,
				eventTypes.configs[eventTypes.defaultEventType] ?? {},
			);
		}
	}, [node?.name, event.event_type]);

	if (foundConfig?.eventTypes.length <= 1) return null;
	return (
		<div className="space-y-3">
			<Label htmlFor="event_type">Event Type</Label>
			<Select
				disabled={disabled}
				value={event.event_type}
				onValueChange={(value) => {
					onUpdate(value, foundConfig.configs[value] ?? {});
				}}
			>
				<SelectTrigger className="w-full">
					<SelectValue placeholder="Select event type" />
				</SelectTrigger>
				<SelectContent>
					{foundConfig?.eventTypes.map((type) => (
						<SelectItem key={type} value={type}>
							{type.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase())}
						</SelectItem>
					))}
				</SelectContent>
			</Select>
		</div>
	);
}

export function EventTranslation({
	eventConfig,
	eventType,
	editing,
	board,
	nodeId,
	config,
	onUpdate,
}: Readonly<{
	eventConfig: IEventMapping;
	eventType: string;
	editing: boolean;
	config: Partial<IEventPayload>;
	board: IBoard;
	nodeId?: string;
	onUpdate: (payload: Partial<IEventPayload>) => void;
}>) {
	const [intermediateConfig, setIntermediateConfig] =
		useState<Partial<IEventPayload>>(config);
	const node: INode | undefined = board.nodes[nodeId ?? ""];

	const foundEventConfig = useMemo(() => {
		return eventConfig[node?.name];
	}, [node?.name]);

	const eventConfigComponent = useMemo(() => {
		if (!foundEventConfig) return null;
		return foundEventConfig.configInterfaces[eventType]
			? foundEventConfig.configInterfaces[eventType]({
					isEditing: editing,
					appId: board.app_id,
					boardId: board.id,
					config: intermediateConfig,
					node: node,
					nodeId: nodeId ?? "",
					onConfigUpdate: (payload) => {
						setIntermediateConfig(payload);
						if (onUpdate) {
							onUpdate(payload);
						}
					},
				})
			: null;
	}, [
		foundEventConfig,
		eventType,
		intermediateConfig,
		board.app_id,
		board.id,
		node,
		nodeId,
		editing,
		onUpdate,
	]);

	if (!node) return <p className="text-red-500">Node not found.</p>;

	if (!foundEventConfig || !eventConfigComponent) {
		return (
			<div className="w-full space-y-4">
				<p className="text-sm text-muted-foreground">
					No specific configuration available for this event type.
				</p>
			</div>
		);
	}

	if (eventConfigComponent) {
		return <div className="w-full space-y-4">{eventConfigComponent}</div>;
	}
}
