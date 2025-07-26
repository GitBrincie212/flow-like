"use client";
import { DragOverlay, useDroppable } from "@dnd-kit/core";
import { createId } from "@paralleldrive/cuid2";
import type { UseQueryResult } from "@tanstack/react-query";
import {
	Background,
	BackgroundVariant,
	type Connection,
	Controls,
	type Edge,
	type FinalConnectionState,
	type InternalNode,
	type IsValidConnection,
	MiniMap,
	type Node,
	type OnEdgesChange,
	type OnNodesChange,
	ReactFlow,
	type ReactFlowInstance,
	addEdge,
	applyEdgeChanges,
	applyNodeChanges,
	reconnectEdge,
	useEdgesState,
	useKeyPress,
	useNodesState,
	useReactFlow,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
	ArrowBigLeftDashIcon,
	HistoryIcon,
	NotebookPenIcon,
	PlayCircleIcon,
	Redo2Icon,
	ScrollIcon,
	SquareChevronUpIcon,
	Undo2Icon,
	VariableIcon,
	XIcon,
} from "lucide-react";
import MiniSearch from "minisearch";
import { useTheme } from "next-themes";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ImperativePanelHandle } from "react-resizable-panels";
import { useLogAggregation } from "../..";
import { CommentNode } from "../../components/flow/comment-node";
import { FlowContextMenu } from "../../components/flow/flow-context-menu";
import { FlowDock } from "../../components/flow/flow-dock";
import { FlowNode } from "../../components/flow/flow-node";
import { Traces } from "../../components/flow/traces";
import {
	Variable,
	VariablesMenu,
} from "../../components/flow/variables/variables-menu";
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "../../components/ui/resizable";
import { useInvoke } from "../../hooks/use-invoke";
import {
	type IGenericCommand,
	type ILogMetadata,
	addNodeCommand,
	connectPinsCommand,
	disconnectPinsCommand,
	moveNodeCommand,
	removeCommentCommand,
	removeLayerCommand,
	removeNodeCommand,
	upsertCommentCommand,
} from "../../lib";
import {
	handleCopy,
	handlePaste,
	isValidConnection,
	parseBoard,
} from "../../lib/flow-board-utils";
import { toastError, toastSuccess } from "../../lib/messages";
import {
	type IComment,
	ICommentType,
	type IVariable,
} from "../../lib/schema/flow/board";
import { type INode, IVariableType } from "../../lib/schema/flow/node";
import type { IPin } from "../../lib/schema/flow/pin";
import type { ILayer } from "../../lib/schema/flow/run";
import { convertJsonToUint8Array } from "../../lib/uint8";
import { useBackend } from "../../state/backend-state";
import { useFlowBoardParentState } from "../../state/flow-board-parent-state";
import { useRunExecutionStore } from "../../state/run-execution-state";
import { BoardMeta } from "./board-meta";
import { useUndoRedo } from "./flow-history";
import { FlowRuns } from "./flow-runs";
import { LayerNode } from "./layer-node";

function hexToRgba(hex: string, alpha = 0.3): string {
	let c = hex.replace("#", "");
	if (c.length === 3) c = c[0] + c[0] + c[1] + c[1] + c[2] + c[2];
	const num = Number.parseInt(c, 16);
	const r = (num >> 16) & 255;
	const g = (num >> 8) & 255;
	const b = num & 255;
	return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function FlowBoard({
	appId,
	boardId,
	nodeId,
	initialVersion,
}: Readonly<{
	appId: string;
	boardId: string;
	nodeId?: string;
	initialVersion?: [number, number, number];
}>) {
	const { pushCommand, pushCommands, redo, undo } = useUndoRedo(appId, boardId);
	const router = useRouter();
	const backend = useBackend();
	const selected = useRef(new Set<string>());
	const edgeReconnectSuccessful = useRef(true);
	const { isOver, setNodeRef, active } = useDroppable({ id: "flow" });
	const parentRegister = useFlowBoardParentState();
	const { refetchLogs, setCurrentMetadata, currentMetadata } =
		useLogAggregation();
	const flowRef = useRef<any>(null);
	const [version, setVersion] = useState<[number, number, number] | undefined>(
		initialVersion,
	);
	const [initialized, setInitialized] = useState(false);
	const flowPanelRef = useRef<ImperativePanelHandle>(null);
	const logPanelRef = useRef<ImperativePanelHandle>(null);
	const varPanelRef = useRef<ImperativePanelHandle>(null);
	const runsPanelRef = useRef<ImperativePanelHandle>(null);

	const shiftPressed = useKeyPress("Shift");

	const { resolvedTheme } = useTheme();

	const catalog: UseQueryResult<INode[]> = useInvoke(
		backend.boardState.getCatalog,
		backend.boardState,
		[],
	);
	const board = useInvoke(
		backend.boardState.getBoard,
		backend.boardState,
		[appId, boardId, version],
		boardId !== "",
	);
	const currentProfile = useInvoke(
		backend.userState.getProfile,
		backend.userState,
		[],
	);
	const { addRun, removeRun, pushUpdate } = useRunExecutionStore();
	const { screenToFlowPosition } = useReactFlow();

	const [nodes, setNodes] = useNodesState<any>([]);
	const [edges, setEdges] = useEdgesState<any>([]);
	const [droppedPin, setDroppedPin] = useState<IPin | undefined>(undefined);
	const [clickPosition, setClickPosition] = useState({ x: 0, y: 0 });
	const [mousePosition, setMousePosition] = useState({ x: 0, y: 0 });
	const [pinCache, setPinCache] = useState<Map<string, [IPin, INode, boolean]>>(
		new Map(),
	);
	const [editBoard, setEditBoard] = useState(false);
	const [currentLayer, setCurrentLayer] = useState<string | undefined>();
	const [layerPath, setLayerPath] = useState<string | undefined>();
	const colorMode = useMemo(
		() => (resolvedTheme === "dark" ? "dark" : "light"),
		[resolvedTheme],
	);

	const pinToNode = useCallback(
		(pinId: string) => {
			const [_, node] = pinCache.get(pinId) || [];
			return node;
		},
		[nodes, pinCache],
	);

	const executeCommand = useCallback(
		async (command: IGenericCommand, append = false): Promise<any> => {
			if (typeof version !== "undefined") {
				toastError("Cannot change old version", <XIcon />);
				return;
			}
			const result = await backend.boardState.executeCommand(
				appId,
				boardId,
				command,
			);
			await pushCommand(result, append);
			await board.refetch();
			return result;
		},
		[board.refetch, appId, boardId, backend, pushCommand, version],
	);

	const executeCommands = useCallback(
		async (commands: IGenericCommand[]) => {
			if (typeof version !== "undefined") {
				toastError("Cannot change old version", <XIcon />);
				return;
			}
			if (commands.length === 0) return;
			const result = await backend.boardState.executeCommands(
				appId,
				boardId,
				commands,
			);
			await pushCommands(result);
			await board.refetch();
			return result;
		},
		[board.refetch, appId, boardId, backend, pushCommands, version],
	);

	useEffect(() => {
		if (!logPanelRef.current) return;

		logPanelRef.current.expand();
		const size = logPanelRef.current.getSize();

		if (size < 10) logPanelRef.current.resize(45);
	}, [logPanelRef.current]);

	const initializeFlow = useCallback(
		async (instance: ReactFlowInstance) => {
			if (initialized) return;
			if (!nodeId || nodeId === "") return;

			instance.fitView({
				nodes: [
					{
						id: nodeId ?? "",
					},
				],
				duration: 500,
			});
			setInitialized(true);
		},
		[nodeId, initialized],
	);

	function toggleVars() {
		if (!varPanelRef.current) return;
		const isCollapsed = varPanelRef.current.isCollapsed();
		isCollapsed ? varPanelRef.current.expand() : varPanelRef.current.collapse();

		if (!isCollapsed) {
			return;
		}

		const size = varPanelRef.current.getSize();
		if (size < 10) varPanelRef.current.resize(20);
	}

	function toggleRunHistory() {
		if (!runsPanelRef.current) return;
		const isCollapsed = runsPanelRef.current.isCollapsed();
		isCollapsed
			? runsPanelRef.current.expand()
			: runsPanelRef.current.collapse();

		if (!isCollapsed) {
			return;
		}

		const size = runsPanelRef.current.getSize();
		if (size < 10) runsPanelRef.current.resize(30);
	}

	function toggleLogs() {
		if (!logPanelRef.current) return;
		const isCollapsed = logPanelRef.current.isCollapsed();
		isCollapsed ? logPanelRef.current.expand() : logPanelRef.current.collapse();

		if (!isCollapsed) {
			return;
		}

		const size = logPanelRef.current.getSize();
		if (size < 10) logPanelRef.current.resize(20);
	}

	const pushLayer = useCallback(
		(pushedLayer: ILayer) => {
			setCurrentLayer(pushedLayer.id);
			setLayerPath((old) => {
				if (old) return `${old}/${pushedLayer.id}`;
				return pushedLayer.id;
			});
		},
		[currentLayer, setCurrentLayer, setLayerPath, layerPath],
	);

	const popLayer = useCallback(() => {
		if (!layerPath) return;
		const segments = layerPath.split("/");
		if (segments.length === 1) {
			setLayerPath(undefined);
			setCurrentLayer(undefined);
			return;
		}
		const newPath = segments.slice(0, -1).join("/");
		setLayerPath(newPath);
		const segment = newPath.split("/").pop();
		setCurrentLayer(segment);
	}, [currentLayer, setCurrentLayer, setLayerPath]);

	const executeBoard = useCallback(
		async (node: INode, payload?: object) => {
			let added = false;
			let runId = "";
			let meta: ILogMetadata | undefined = undefined;
			try {
				meta = await backend.boardState.executeBoard(
					appId,
					boardId,
					{
						id: node.id,
						payload: payload,
					},
					true,
					async (id: string) => {
						if (added) return;
						console.log("Run started", id);
						runId = id;
						added = true;
						addRun(id, boardId, [node.id]);
					},
					(update) => {
						const runUpdates = update
							.filter((item) => item.event_type.startsWith("run:"))
							.map((item) => item.payload);
						if (runUpdates.length === 0) return;
						const firstItem = runUpdates[0];
						if (!added) {
							runId = firstItem.run_id;
							addRun(firstItem.run_id, boardId, [node.id]);
							added = true;
						}

						pushUpdate(firstItem.run_id, runUpdates);
					},
				);
			} catch (error) {
				console.warn("Failed to execute board", error);
			}
			removeRun(runId);
			if (!meta) {
				toastError(
					"Failed to execute board",
					<PlayCircleIcon className="w-4 h-4" />,
				);
				return;
			}
			await refetchLogs(backend);
			if (meta) setCurrentMetadata(meta);
		},
		[
			appId,
			boardId,
			backend,
			refetchLogs,
			pushUpdate,
			addRun,
			removeRun,
			setCurrentMetadata,
		],
	);

	const handlePasteCB = useCallback(
		async (event: ClipboardEvent) => {
			if (typeof version !== "undefined") {
				toastError("Cannot change old version", <XIcon />);
				return;
			}
			const currentCursorPosition = screenToFlowPosition({
				x: mousePosition.x,
				y: mousePosition.y,
			});
			await handlePaste(
				event,
				currentCursorPosition,
				boardId,
				executeCommand,
				currentLayer,
			);
		},
		[boardId, mousePosition, executeCommand, currentLayer, version],
	);

	const handleCopyCB = useCallback(
		(event?: ClipboardEvent) => {
			if (!board.data) return;
			const currentCursorPosition = screenToFlowPosition({
				x: mousePosition.x,
				y: mousePosition.y,
			});
			handleCopy(nodes, board.data, currentCursorPosition, event, currentLayer);
		},
		[nodes, mousePosition, board.data, currentLayer],
	);

	const placeNodeShortcut = useCallback(
		async (node: INode) => {
			await placeNode(node, {
				x: mousePosition.x,
				y: mousePosition.y,
			});
		},
		[mousePosition],
	);

	const shortcutHandler = useCallback(
		async (event: KeyboardEvent) => {
			// Undo
			if (
				(event.metaKey || event.ctrlKey) &&
				event.key === "z" &&
				!event.shiftKey
			) {
				event.preventDefault();
				event.stopPropagation();
				if (typeof version !== "undefined") {
					toastError("Cannot change old version", <XIcon />);
					return;
				}
				const stack = await undo();
				if (stack) await backend.boardState.undoBoard(appId, boardId, stack);
				toastSuccess("Undo", <Undo2Icon className="w-4 h-4" />);
				await board.refetch();
				return;
			}

			// Redo
			if ((event.metaKey || event.ctrlKey) && event.key === "y") {
				event.preventDefault();
				event.stopPropagation();
				if (typeof version !== "undefined") {
					toastError("Cannot change old version", <XIcon />);
					return;
				}
				const stack = await redo();
				if (stack) await backend.boardState.redoBoard(appId, boardId, stack);
				toastSuccess("Redo", <Redo2Icon className="w-4 h-4" />);
				await board.refetch();
			}

			// Place Branch
			if (
				(event.metaKey || event.ctrlKey) &&
				event.key === "b" &&
				!event.shiftKey
			) {
				event.preventDefault();
				event.stopPropagation();
				if (typeof version !== "undefined") {
					toastError("Cannot change old version", <XIcon />);
					return;
				}
				const node = catalog.data?.find(
					(node) => node.name === "control_branch",
				);
				if (!node) return;
				await placeNodeShortcut(node);
				await board.refetch();
				return;
			}

			// Place For Each
			if (
				(event.metaKey || event.ctrlKey) &&
				event.key === "f" &&
				!event.shiftKey
			) {
				event.preventDefault();
				event.stopPropagation();
				if (typeof version !== "undefined") {
					toastError("Cannot change old version", <XIcon />);
					return;
				}
				const node = catalog.data?.find(
					(node) => node.name === "control_for_each",
				);
				if (!node) return;
				await placeNodeShortcut(node);
				await board.refetch();
				return;
			}

			if (
				(event.metaKey || event.ctrlKey) &&
				event.key === "p" &&
				!event.shiftKey
			) {
				event.preventDefault();
				event.stopPropagation();
				if (typeof version !== "undefined") {
					toastError("Cannot change old version", <XIcon />);
					return;
				}
				const node = catalog.data?.find((node) => node.name === "log_info");
				if (!node) return;
				await placeNodeShortcut(node);
				await board.refetch();
				return;
			}

			if (
				(event.metaKey || event.ctrlKey) &&
				event.key === "s" &&
				!event.shiftKey
			) {
				event.preventDefault();
				event.stopPropagation();
				if (typeof version !== "undefined") {
					toastError("Cannot change old version", <XIcon />);
					return;
				}
				const node = catalog.data?.find((node) => node.name === "reroute");
				if (!node) return;
				await placeNodeShortcut(node);
				await board.refetch();
			}
		},
		[boardId, board, backend, version],
	);

	const placeNode = useCallback(
		async (node: INode, position?: { x: number; y: number }) => {
			const refs = board.data?.refs ?? {};
			const location = screenToFlowPosition({
				x: position?.x ?? clickPosition.x,
				y: position?.y ?? clickPosition.y,
			});
			const result = addNodeCommand({
				node: { ...node, coordinates: [location.x, location.y, 0] },
				current_layer: currentLayer,
			});

			await executeCommand(result.command);
			const new_node = result.node;
			if (droppedPin) {
				const pinType = droppedPin.pin_type === "Input" ? "Output" : "Input";
				const pinValueType = droppedPin.value_type;
				const pinDataType = droppedPin.data_type;
				const schema = refs?.[droppedPin.schema ?? ""] ?? droppedPin.schema;
				const options = droppedPin.options;

				const pin = Object.values(new_node.pins).find((pin) => {
					if (typeof schema === "string" || typeof pin.schema === "string") {
						const pinSchema = refs?.[pin.schema ?? ""] ?? pin.schema;
						if (
							(pin.options?.enforce_schema || options?.enforce_schema) &&
							schema !== pinSchema &&
							pin.data_type !== IVariableType.Generic &&
							droppedPin.data_type !== IVariableType.Generic
						)
							return false;
					}
					if (pin.pin_type !== pinType) return false;
					if (pin.value_type !== pinValueType) {
						if (
							pinDataType !== IVariableType.Generic &&
							pin.data_type !== IVariableType.Generic
						)
							return false;
						if (
							(options?.enforce_generic_value_type ?? false) ||
							(pin.options?.enforce_generic_value_type ?? false)
						)
							return false;
					}
					if (
						pin.data_type === IVariableType.Generic &&
						pinDataType !== IVariableType.Execution
					)
						return true;
					if (
						pinDataType === IVariableType.Generic &&
						pin.data_type !== IVariableType.Execution
					)
						return true;
					return pin.data_type === pinDataType;
				});
				const [sourcePin, sourceNode] = pinCache.get(droppedPin.id) || [];
				if (!sourcePin || !sourceNode) return;
				if (!pin) return;

				const command = connectPinsCommand({
					from_node:
						droppedPin.pin_type === "Output" ? sourceNode.id : new_node.id,
					from_pin: droppedPin.pin_type === "Output" ? sourcePin.id : pin?.id,
					to_node:
						droppedPin.pin_type === "Input" ? sourceNode.id : new_node.id,
					to_pin: droppedPin.pin_type === "Input" ? sourcePin.id : pin?.id,
				});

				await executeCommand(command);
			}
		},
		[
			clickPosition,
			boardId,
			droppedPin,
			board.data?.refs,
			currentLayer,
			screenToFlowPosition,
		],
	);

	const handleDrop = useCallback(
		async (event: any) => {
			const variable: IVariable = event.detail.variable;
			const operation: "set" | "get" = event.detail.operation;
			const screenPosition = event.detail.screenPosition;
			const getVarNode = catalog.data?.find(
				(node) => node.name === `variable_${operation}`,
			);
			if (!getVarNode) return console.dir(catalog.data);

			const varRefPin = Object.values(getVarNode.pins).find(
				(pin) => pin.name === "var_ref",
			);
			if (!varRefPin) return;

			varRefPin.default_value = convertJsonToUint8Array(variable.id);
			getVarNode.pins[varRefPin.id] = varRefPin;

			placeNode(getVarNode, {
				x: screenPosition.x,
				y: screenPosition.y,
			});
		},
		[catalog.data, clickPosition, boardId, droppedPin],
	);

	useEffect(() => {
		document.addEventListener("copy", handleCopyCB);
		document.addEventListener("paste", handlePasteCB);

		return () => {
			document.removeEventListener("copy", handleCopyCB);
			document.removeEventListener("paste", handlePasteCB);
		};
	}, [nodes]);

	useEffect(() => {
		document.addEventListener("keydown", shortcutHandler);
		return () => {
			document.removeEventListener("keydown", shortcutHandler);
		};
	}, [shortcutHandler]);

	useEffect(() => {
		document.addEventListener("flow-drop", handleDrop);
		return () => {
			document.removeEventListener("flow-drop", handleDrop);
		};
	}, [handleDrop]);

	useEffect(() => {
		document.addEventListener("mousemove", (event) => {
			setMousePosition({ x: event.clientX, y: event.clientY });
		});

		return () => {
			document.removeEventListener("mousemove", (event) => {
				setMousePosition({ x: event.clientX, y: event.clientY });
			});
		};
	}, []);

	useEffect(() => {
		if (!board.data) return;

		const parsed = parseBoard(
			board.data,
			appId,
			handleCopyCB,
			pushLayer,
			executeBoard,
			executeCommand,
			selected.current,
			currentProfile.data?.settings?.connection_mode ?? "default",
			nodes,
			edges,
			currentLayer,
		);

		setNodes(parsed.nodes);
		setEdges(parsed.edges);
		setPinCache(new Map(parsed.cache));
	}, [board.data, currentLayer, currentProfile.data]);

	const nodeTypes = useMemo(
		() => ({
			flowNode: FlowNode,
			commentNode: CommentNode,
			layerNode: LayerNode,
			node: FlowNode,
		}),
		[],
	);

	const onConnect = useCallback(
		(params: any) =>
			setEdges((eds) => {
				const [sourcePin, sourceNode] = pinCache.get(params.sourceHandle) || [];
				const [targetPin, targetNode] = pinCache.get(params.targetHandle) || [];

				if (!sourcePin || !targetPin) return eds;
				if (!sourceNode || !targetNode) return eds;

				const command = connectPinsCommand({
					from_node: sourceNode.id,
					from_pin: sourcePin.id,
					to_node: targetNode.id,
					to_pin: targetPin.id,
				});

				executeCommand(command);

				return addEdge(params, eds);
			}),
		[setEdges, pinCache, boardId],
	);

	const onConnectEnd = useCallback(
		(
			event: MouseEvent | TouchEvent,
			connectionState: FinalConnectionState<InternalNode>,
		) => {
			// when a connection is dropped on the pane it's not valid
			if (!connectionState.isValid) {
				// we need to remove the wrapper bounds, in order to get the correct position

				const { clientX, clientY } =
					"changedTouches" in event ? event.changedTouches[0] : event;

				const handle = connectionState.fromHandle;
				if (handle?.id) {
					const [pin, _node] = pinCache.get(handle.id) || [];
					setDroppedPin(pin);
				}

				const contextMenuEvent = new MouseEvent("contextmenu", {
					bubbles: true,
					cancelable: true,
					view: window,
					clientX,
					clientY,
				});

				flowRef.current?.dispatchEvent(contextMenuEvent);
			}
		},
		[pinCache],
	);

	const onNodesChangeIntercept: OnNodesChange = useCallback(
		(changes: any[]) =>
			setNodes((nds) => {
				if (!changes) return applyNodeChanges(changes, nds);

				const selectChanges = changes.filter(
					(change: any) => change.type === "select",
				);
				for (const change of selectChanges) {
					const selectedId = change.id;

					if (change.selected) selected.current.add(selectedId);
					if (!change.selected) selected.current.delete(selectedId);
				}

				const removeChanges = changes.filter(
					(change: any) => change.type === "remove",
				);
				executeCommands(
					removeChanges
						.map((change) => {
							const foundNode = Object.values(board.data?.nodes || {}).find(
								(node) => node.id === change.id,
							);
							if (foundNode) {
								return removeNodeCommand({
									node: foundNode,
									connected_nodes: [],
								});
							}
							const foundComment = Object.values(
								board.data?.comments || {},
							).find((comment) => comment.id === change.id);
							if (foundComment) {
								return removeCommentCommand({
									comment: foundComment,
								});
							}

							const foundLayer = Object.values(board.data?.layers || {}).find(
								(layer) => layer.id === change.id,
							);

							if (foundLayer) {
								return removeLayerCommand({
									child_layers: [],
									layer: foundLayer,
									layer_nodes: [],
									layers: [],
									nodes: [],
									preserve_nodes: false,
								});
							}

							return undefined;
						})
						.filter((command) => command !== undefined) as any[],
				);

				return applyNodeChanges(changes, nds);
			}),
		[setNodes, board.data, boardId, executeCommands],
	);

	const onEdgesChange: OnEdgesChange = useCallback(
		(changes: any[]) =>
			setEdges((eds) => {
				if (!changes || changes.length === 0)
					return applyEdgeChanges(changes, eds);

				const selectChanges = changes.filter(
					(change: any) => change.type === "select",
				);
				for (const change of selectChanges) {
					const selectedId = change.id;
					const selectedEdge: any = eds.find((edge) => edge.id === selectedId);

					if (change.selected) selected.current.add(selectedId);
					if (!change.selected) selected.current.delete(selectedId);

					if (selectedEdge.data_type !== "Execution")
						eds = eds.map((edge) =>
							edge.id === selectedId
								? { ...edge, animated: !change.selected }
								: edge,
						);
				}

				const removeChanges = changes.filter(
					(change: any) => change.type === "remove",
				);
				executeCommands(
					removeChanges
						.map((change: any) => {
							const selectedId = change.id;
							const [fromPinId, toPinId] = selectedId.split("-");
							const [fromPin, fromNode] = pinCache.get(fromPinId) || [];
							const [toPin, toNode] = pinCache.get(toPinId) || [];

							if (!fromPin || !toPin) return undefined;
							if (!fromNode || !toNode) return undefined;

							return disconnectPinsCommand({
								from_node: fromNode.id,
								from_pin: fromPin.id,
								to_node: toNode.id,
								to_pin: toPin.id,
							});
						})
						.filter((command: any) => command !== undefined) as any[],
				);

				return applyEdgeChanges(changes, eds);
			}),
		[setEdges, board.data, boardId, pinCache],
	);

	const onReconnectStart = useCallback(() => {
		edgeReconnectSuccessful.current = false;
	}, []);

	const onReconnect = useCallback(
		async (oldEdge: any, newConnection: Connection) => {
			// Check if the edge is actually being moved
			const new_id = `${newConnection.sourceHandle}-${newConnection.targetHandle}`;
			if (oldEdge.id === new_id) return;

			const oldEdgeToNode = pinToNode(oldEdge.targetHandle);
			const oldEdgeFromNode = pinToNode(oldEdge.sourceHandle);

			if (!oldEdgeToNode || !oldEdgeFromNode) return;

			const commands = [];

			const disconnectCommand = disconnectPinsCommand({
				from_node: oldEdgeFromNode.id,
				from_pin: oldEdge.sourceHandle,
				to_node: oldEdgeToNode.id,
				to_pin: oldEdge.targetHandle,
			});

			commands.push(disconnectCommand);

			if (newConnection.targetHandle && newConnection.sourceHandle) {
				const newConnectionSourceNode = pinToNode(newConnection.sourceHandle);
				const newConnectionTargetNode = pinToNode(newConnection.targetHandle);

				if (newConnectionSourceNode && newConnectionTargetNode)
					commands.push(
						connectPinsCommand({
							from_node: newConnectionSourceNode.id,
							from_pin: newConnection.sourceHandle,
							to_node: newConnectionTargetNode.id,
							to_pin: newConnection.targetHandle,
						}),
					);
			}

			await executeCommands(commands);

			edgeReconnectSuccessful.current = true;
			setEdges((els) => reconnectEdge(oldEdge, newConnection, els));
		},
		[boardId, setEdges, pinToNode, executeCommands],
	);

	const onReconnectEnd = useCallback(
		async (event: any, edge: any) => {
			if (!edgeReconnectSuccessful.current) {
				const { source, target, sourceHandle, targetHandle } = edge;
				const from_node = pinToNode(sourceHandle);
				const to_node = pinToNode(targetHandle);
				if (!from_node || !to_node) return;
				const command = disconnectPinsCommand({
					from_node: from_node?.id,
					from_pin: sourceHandle,
					to_node: to_node?.id,
					to_pin: targetHandle,
				});
				await executeCommand(command);
				setEdges((eds) => eds.filter((e) => e.id !== edge.id));
			}

			edgeReconnectSuccessful.current = true;
		},
		[boardId, setEdges, pinToNode],
	);

	const onContextMenuCB = useCallback((event: any) => {
		setClickPosition({ x: event.clientX, y: event.clientY });
	}, []);

	const onNodeDragStop = useCallback(
		async (event: any, node: any, nodes: any) => {
			const commands: IGenericCommand[] = [];
			for await (const node of nodes) {
				const command = moveNodeCommand({
					node_id: node.id,
					to_coordinates: [node.position.x, node.position.y, 0],
					current_layer: currentLayer,
				});

				commands.push(command);
			}
			await executeCommands(commands);
		},
		[boardId, executeCommands, currentLayer],
	);

	const isValidConnectionCB = useCallback(
		(connection: Edge | Connection) => {
			return isValidConnection(connection, pinCache, board.data?.refs ?? {});
		},
		[pinCache, board.data?.refs],
	) as IsValidConnection<Edge>;

	const onNodeDoubleClick = useCallback(
		(event: any, node: any) => {
			const tgt = event.target as HTMLElement;
			if (tgt.closest("input, textarea")) {
				return;
			}
			const type = node?.type ?? "";
			if (type === "layerNode") {
				const layer: ILayer = node.data.layer;
				pushLayer(layer);
				return;
			}
		},
		[pushLayer],
	);

	const onCommentPlace = useCallback(async () => {
		const location = screenToFlowPosition({
			x: clickPosition.x,
			y: clickPosition.y,
		});
		const new_comment: IComment = {
			comment_type: ICommentType.Text,
			content: "",
			coordinates: [location.x, location.y, 0],
			id: createId(),
			timestamp: {
				nanos_since_epoch: 0,
				secs_since_epoch: 0,
			},
			author: "anonymous",
		};

		const command = upsertCommentCommand({
			comment: new_comment,
			current_layer: currentLayer,
		});

		await executeCommand(command);
	}, [currentLayer, clickPosition, executeCommand]);

	const onNodeDrag = useCallback(
		(event: any, node: Node, nodes: Node[]) => {
			if (shiftPressed) {
				nodes.forEach((node) => {
					if (node.type === "layerNode") {
						const layerData = node.data.layer as ILayer;
						const diffX = Math.abs(node.position.x - layerData.coordinates[0]);
						const diffY = Math.abs(node.position.y - layerData.coordinates[1]);
						if (diffX > diffY) {
							node.position.y = layerData.coordinates[1];
							return;
						}
						node.position.x = layerData.coordinates[0];
						return;
					}

					if (node.type === "commentNode") {
						const commentData = node.data.comment as IComment;
						const diffX = Math.abs(
							node.position.x - commentData.coordinates[0],
						);
						const diffY = Math.abs(
							node.position.y - commentData.coordinates[1],
						);
						if (diffX > diffY) {
							node.position.y = commentData.coordinates[1];
							return;
						}
						node.position.x = commentData.coordinates[0];
						return;
					}

					if (node.type === "node") {
						const nodeData = node.data.node as INode;
						if (!nodeData.coordinates) return;
						const diffX = Math.abs(node.position.x - nodeData.coordinates[0]);
						const diffY = Math.abs(node.position.y - nodeData.coordinates[1]);
						if (diffX > diffY) {
							node.position.y = nodeData.coordinates[1];
							return;
						}
						node.position.x = nodeData.coordinates[0];
					}
				});
			}
		},
		[shiftPressed],
	);

	return (
		<div className="min-h-dvh h-dvh max-h-dvh w-full flex-1 grow flex-col">
			<div className="flex items-center justify-center absolute translate-x-[-50%] mt-5 left-[50dvw] z-40">
				{board.data && editBoard && (
					<BoardMeta
						appId={appId}
						board={board.data}
						boardId={boardId}
						closeMeta={() => setEditBoard(false)}
						version={version}
						selectVersion={(version) => setVersion(version)}
					/>
				)}
				<FlowDock
					items={[
						...(typeof parentRegister.boardParents[boardId] === "string" &&
						!currentLayer
							? [
									{
										icon: <ArrowBigLeftDashIcon />,
										title: "Back",
										onClick: async () => {
											const urlWithQuery = parentRegister.boardParents[boardId];
											router.push(urlWithQuery);
										},
									},
								]
							: []),
						{
							icon: <VariableIcon />,
							title: "Variables",
							onClick: async () => {
								toggleVars();
							},
						},
						{
							icon: <NotebookPenIcon />,
							title: "Manage Board",
							onClick: async () => {
								setEditBoard(true);
							},
						},
						{
							icon: <HistoryIcon />,
							separator: "left",
							title: "Run History",
							onClick: async () => {
								toggleRunHistory();
							},
						},
						...(currentMetadata
							? [
									{
										icon: <ScrollIcon />,
										title: "Logs",
										onClick: async () => {
											toggleLogs();
										},
									},
								]
							: ([] as any)),
						...(currentLayer
							? [
									{
										icon: <SquareChevronUpIcon />,
										title: "Layer Up",
										separator: "left",
										highlight: true,
										onClick: async () => {
											popLayer();
										},
									},
								]
							: []),
					]}
				/>
			</div>
			<ResizablePanelGroup
				direction="horizontal"
				className="flex grow min-h-dvh h-dvh"
			>
				<ResizablePanel
					className="z-50 bg-background"
					autoSave="flow-variables"
					defaultSize={0}
					collapsible={true}
					collapsedSize={0}
					ref={varPanelRef}
				>
					{board.data && (
						<VariablesMenu board={board.data} executeCommand={executeCommand} />
					)}
				</ResizablePanel>
				<ResizableHandle withHandle />
				<ResizablePanel autoSave="flow-main-container">
					<ResizablePanelGroup
						direction="vertical"
						className="h-full flex grow"
					>
						<ResizablePanel autoSave="flow-main" ref={flowPanelRef}>
							<FlowContextMenu
								droppedPin={droppedPin}
								onCommentPlace={onCommentPlace}
								refs={board.data?.refs || {}}
								onClose={() => setDroppedPin(undefined)}
								nodes={catalog.data ?? []}
								onNodePlace={async (node) => {
									await placeNode(node);
								}}
							>
								<div
									className={`w-full h-full relative ${isOver && "border-green-400 border-2 z-10"}`}
									ref={setNodeRef}
								>
									{currentLayer && (
										<h2 className="absolute bottom-0 left-0 z-10 ml-16 mb-10 text-muted pointer-events-none select-none">
											{board.data?.layers[currentLayer]?.name}
										</h2>
									)}
									{version && (
										<h3 className="absolute top-0 mr-2 mt-2 right-0 z-10 text-muted pointer-events-none select-none">
											Version {version[0]}.{version[1]}.{version[2]} - Read-Only
										</h3>
									)}
									<ReactFlow
										suppressHydrationWarning
										onContextMenu={onContextMenuCB}
										nodesDraggable={typeof version === "undefined"}
										nodesConnectable={typeof version === "undefined"}
										onInit={initializeFlow}
										ref={flowRef}
										colorMode={colorMode}
										nodes={nodes}
										nodeTypes={nodeTypes}
										edges={edges}
										maxZoom={3}
										minZoom={0.1}
										onNodeDoubleClick={onNodeDoubleClick}
										onNodesChange={onNodesChangeIntercept}
										onEdgesChange={onEdgesChange}
										onNodeDragStop={onNodeDragStop}
										onNodeDrag={onNodeDrag}
										isValidConnection={isValidConnectionCB}
										onConnect={onConnect}
										onReconnect={onReconnect}
										onReconnectStart={onReconnectStart}
										// onEdgeDoubleClick={(e, edge) => {
										// 	console.dir({e, edge})
										// }}
										onReconnectEnd={onReconnectEnd}
										onConnectEnd={onConnectEnd}
										fitView
										proOptions={{ hideAttribution: true }}
									>
										<Controls />
										<MiniMap
											pannable
											zoomable
											nodeColor={(node) => {
												if (node.type === "layerNode")
													return "var(--foreground) / 0.5";

												if (node.type === "node") {
													const nodeData: INode = node.data.node as INode;
													if (nodeData.event_callback)
														return "var(--primary/ 0.8)";
													if (nodeData.start) return "var(--primary/ 0.8)";
													if (
														!Object.values(nodeData.pins).find(
															(pin) =>
																pin.data_type === IVariableType.Execution,
														)
													)
														return "var(--tertiary)/ 0.8";
													return "var(--muted)";
												}
												if (node.type === "commentNode") {
													const commentData: IComment = node.data
														.comment as IComment;
													let color = commentData.color ?? "var(--muted)/ 0.3";

													if (color.startsWith("#")) {
														color = hexToRgba(color, 0.3);
													}
													return color;
												}
												return "var(--primary)/ 0.6";
											}}
										/>
										<Background
											variant={
												currentLayer
													? BackgroundVariant.Lines
													: BackgroundVariant.Dots
											}
											color={currentLayer && "var(--muted) / 0.3"}
											gap={12}
											size={1}
										/>
									</ReactFlow>
									<DragOverlay
										dropAnimation={{
											duration: 500,
											easing: "cubic-bezier(0.18, 0.67, 0.6, 1.22)",
										}}
									>
										{(active?.data?.current as IVariable)?.id && (
											<Variable
												variable={active?.data?.current as IVariable}
												preview
												onVariableChange={() => {}}
												onVariableDeleted={() => {}}
											/>
										)}
									</DragOverlay>
								</div>
							</FlowContextMenu>
						</ResizablePanel>
						<ResizableHandle withHandle />
						<ResizablePanel
							className="z-50"
							hidden={!currentMetadata}
							ref={logPanelRef}
							defaultSize={0}
							collapsedSize={0}
							collapsible={true}
							autoSave="flow-logs"
						>
							{currentMetadata && <Traces appId={appId} boardId={boardId} />}
						</ResizablePanel>
					</ResizablePanelGroup>
				</ResizablePanel>
				<ResizableHandle withHandle />
				<ResizablePanel
					className="z-50"
					autoSave="flow-runs"
					defaultSize={0}
					collapsible={true}
					collapsedSize={0}
					ref={runsPanelRef}
				>
					{board.data && (
						<FlowRuns
							executeBoard={executeBoard}
							nodes={board.data.nodes}
							appId={appId}
							boardId={boardId}
							version={board.data.version as [number, number, number]}
							onVersionChange={setVersion}
						/>
					)}
				</ResizablePanel>
			</ResizablePanelGroup>
		</div>
	);
}
