import { DndContext, MouseSensor, useSensor, useSensors } from "@dnd-kit/core";
import { useCallback, useState } from "react";
import type { IVariable } from "../../lib/schema/flow/variable";
import { Button } from "../ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "../ui/dialog";
import { FlowBoard } from "./flow-board";

export function FlowWrapper({
	boardId,
	appId,
	nodeId,
	version,
}: Readonly<{
	boardId: string;
	appId: string;
	nodeId?: string;
	version?: [number, number, number];
}>) {
	const mouseSensor = useSensor(MouseSensor, {
		activationConstraint: {
			distance: 10,
		},
	});

	const [detail, setDetail] = useState<
		| undefined
		| {
				variable: IVariable;
				screenPosition: { x: number; y: number };
		  }
	>();

	const sensors = useSensors(mouseSensor);

	const placeNode = useCallback(
		async (operation: "set" | "get") => {
			document.dispatchEvent(
				new CustomEvent("flow-drop", {
					detail: { ...detail, operation },
				}),
			);
			setDetail(undefined);
		},
		[detail, boardId],
	);

	return (
		<DndContext
			sensors={sensors}
			onDragEnd={(event) => {
				if (!event.over) return;
				const overId = String(event.over.id);
				const variable = event.active.data.current as IVariable | undefined;
				if (!variable) return;

				// Dropped on the canvas -> ask user whether to Get/Set
				if (overId === "flow") {
					const mouseEvent: MouseEvent = event.activatorEvent as MouseEvent;
					setDetail({
						variable,
						screenPosition: {
							x: mouseEvent.screenX + event.delta.x,
							y: mouseEvent.screenY + event.delta.y,
						},
					});
					return;
				}

				// Dropped on a folder or root -> broadcast to VariablesMenu
				document.dispatchEvent(
					new CustomEvent("variables-folder-drop", {
						detail: {
							variable,
							targetPath: overId, // "__root" for top-level
						},
					}),
				);
			}}
		>
			<FlowBoard
				boardId={boardId}
				appId={appId}
				nodeId={nodeId}
				initialVersion={version}
			/>
			<Dialog
				open={detail !== undefined}
				onOpenChange={(open) => {
					if (!open) setDetail(undefined);
				}}
			>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>Reference: {detail?.variable.name}</DialogTitle>
					</DialogHeader>
					<div className="w-full flex items-center justify-start gap-2 max-w-full">
						<Button
							className="flex-grow"
							variant={"outline"}
							onClick={() => placeNode("get")}
						>
							Get
						</Button>
						<Button
							className="flex-grow"
							variant={"outline"}
							onClick={() => placeNode("set")}
						>
							Set
						</Button>
					</div>
				</DialogContent>
			</Dialog>
		</DndContext>
	);
}
