"use client";

import { createId } from "@paralleldrive/cuid2";
import {
	ArrowDownIcon,
	ArrowUpIcon,
	EllipsisVerticalIcon,
	GripIcon,
	GripVerticalIcon,
	ListIcon,
	PlusIcon,
	SaveIcon,
	SlidersHorizontalIcon,
	Trash2Icon,
} from "lucide-react";
import {
	type RefObject,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import {
	DndContext,
	type DragEndEvent,
	PointerSensor,
	useSensor,
	useSensors,
	closestCenter,
} from "@dnd-kit/core";
import {
	SortableContext,
	verticalListSortingStrategy,
	useSortable,
	arrayMove,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
	type IPin,
	type IPinOptions,
	IValueType,
	IVariableType,
} from "../../lib";
import {
	type IBoard,
	type ILayer,
	IPinType,
} from "../../lib/schema/flow/board";
import {
	Button,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
	Input,
	Label,
	ScrollArea,
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
	Separator,
	Tabs,
	TabsContent,
	TabsList,
	TabsTrigger,
} from "../ui";
import { typeToColor } from "./utils";
import { ValueTypeIcon } from "./variables/variables-menu";

type PinEdit = {
	id: string;
	name: string;
	friendly_name: string;
	description: string;
	data_type: IVariableType;
	options?: IPinOptions | null;
	schema?: string | null;
	pin_type: IPinType;
	index: number;
	value_type: IValueType;
};

function selectPreviewElement(type: IVariableType){
		return (
			<div className="flex items-center gap-2">
				<div className={`size-2 rounded-full`} style={{ backgroundColor: typeToColor(type) }} />
				<span>{type}</span>
			</div>
		);
	}

const normalizeValueType = (vt: any): IValueType => {
	const s = String(vt ?? "").toLowerCase();
	if (s === "array") return IValueType.Array;
	if (s === "hashmap" || s === "map") return IValueType.HashMap;
	if (s === "hashset" || s === "set") return IValueType.HashSet;
	return IValueType.Normal;
};

const toMachineName = (s: string) =>
	s.trim().toLowerCase().replace(/\s+/g, "_");

const sortByIndex = <T extends { index: number }>(arr: T[]) =>
	[...arr].sort((a, b) => a.index - b.index);

const reindex = <T extends { index: number }>(arr: T[]) =>
	arr.map((p, i) => ({ ...p, index: i + 1 }));

const useGroupedPins = (edits: Record<string, PinEdit>) => {
	return useMemo(() => {
		const all = Object.values(edits);
		const inputs = sortByIndex(
			all.filter((p) => p.pin_type === IPinType.Input),
		);
		const outputs = sortByIndex(
			all.filter((p) => p.pin_type === IPinType.Output),
		);
		return { inputs, outputs };
	}, [edits]);
};

const buildInitialEdits = (
	layer: ILayer,
	boardRef?: RefObject<IBoard | undefined>,
): Record<string, PinEdit> => {
	const out: Record<string, PinEdit> = {};
	for (const pin of Object.values(layer.pins)) {
		const p: any = pin;
		const friendly = p?.friendly_name ?? p?.name ?? pin.id;
		let description = p?.description ?? "";

		const ref = boardRef?.current?.refs?.[description];
		if (ref) {
			description = ref;
		}

		// Hash for empty string, empty string will not be taken into the refs.
		if (description === "16248035215404677707") {
			description = "";
		}

		out[pin.id] = {
			id: pin.id,
			name: toMachineName(friendly),
			friendly_name: friendly,
			description: description,
			data_type: p?.data_type ?? IVariableType.Generic,
			options: p?.options ?? null,
			schema: p?.schema ?? null,
			pin_type: pin.pin_type,
			index: pin.index ?? 1,
			value_type: normalizeValueType(p?.value_type ?? p?.valueType),
		};
	}
	return out;
};

interface LayerEditMenuProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	layer: ILayer;
	onApply: (updated: ILayer) => Promise<void>;
	boardRef?: RefObject<IBoard | undefined>;
}

export const LayerEditMenu: React.FC<LayerEditMenuProps> = ({
	open,
	onOpenChange,
	layer,
	onApply,
	boardRef,
}) => {
	const [edits, setEdits] = useState<Record<string, PinEdit>>(() =>
		buildInitialEdits(layer, boardRef),
	);
	const { inputs, outputs } = useGroupedPins(edits);
	const [tab, setTab] = useState<"inputs" | "outputs">("inputs");

	useEffect(() => {
		if (open) {
			setEdits(buildInitialEdits(layer, boardRef));
			setTab("inputs");
		}
	}, [open, layer]);

	const setPin = useCallback((id: string, updater: (p: PinEdit) => PinEdit) => {
		setEdits((prev) => {
			const curr = prev[id];
			if (!curr) return prev;
			return { ...prev, [id]: updater(curr) };
		});
	}, []);

	const editPin = useCallback(
		(id: string, patch: Partial<PinEdit>) => {
			setPin(id, (p) => {
				const next: PinEdit = { ...p, ...patch };
				if (patch.friendly_name !== undefined) {
					next.name = toMachineName(patch.friendly_name);
				}
				return next;
			});
		},
		[setPin],
	);

	const reindexGroupInState = useCallback((group: PinEdit[]) => {
		const re = reindex(group);
		setEdits((prev) => {
			const copy = { ...prev };
			for (const p of re) copy[p.id] = { ...copy[p.id], index: p.index };
			return copy;
		});
	}, []);

	const movePin = useCallback(
		(id: string, dir: "up" | "down") => {
			const group = edits[id]?.pin_type === IPinType.Input ? inputs : outputs;
			const idx = group.findIndex((p: any) => p.id === id);
			if (idx < 0) return;

			const nextIdx = dir === "up" ? idx - 1 : idx + 1;
			if (nextIdx < 0 || nextIdx >= group.length) return;

			const swapped = [...group];
			[swapped[idx], swapped[nextIdx]] = [swapped[nextIdx], swapped[idx]];
			reindexGroupInState(swapped);
		},
		[edits, inputs, outputs, reindexGroupInState],
	);

	const addPin = useCallback((pin_type: IPinType) => {
		setEdits((prev) => {
			const id = createId();
			const group = Object.values(prev).filter((p) => p.pin_type === pin_type);
			const next: PinEdit = {
				id,
				name: "new_pin",
				friendly_name: "New Pin",
				description: "",
				data_type: IVariableType.Generic,
				options: null,
				schema: null,
				pin_type,
				index: group.length,
				value_type: IValueType.Normal,
			};
			return { ...prev, [id]: next };
		});
	}, []);

	const removePin = useCallback((id: string) => {
		setEdits((prev) => {
			const pin = prev[id];
			if (!pin) return prev;
			const copy = { ...prev };
			delete copy[id];

			const remaining = Object.values(copy).filter(
				(p) => p.pin_type === pin.pin_type,
			);
			const re = reindex(sortByIndex(remaining));
			for (const p of re) copy[p.id] = { ...copy[p.id], index: p.index };
			return copy;
		});
	}, []);

	const reorderByIds = useCallback((orderedIds: string[]) => {
		setEdits((prev) => {
			const copy = { ...prev };
			orderedIds.forEach((id, i) => {
				if (copy[id]) copy[id] = { ...copy[id], index: i + 1 };
			});
			return copy;
		});
	}, []);

	const applyChanges = useCallback(async () => {
		const original = layer.pins;
		const nextPins: Record<string, IPin> = {};

		const zeroIndexed = Object.values(edits).find((p) => p.index <= 0);

		for (const edit of Object.values(edits)) {
			const prev = original[edit.id] as IPin | undefined;

			const merged: IPin = {
				...(prev as IPin),
				id: edit.id,
				pin_type: edit.pin_type,
				index: zeroIndexed ? (edit.index ?? 0) + 1 : (edit.index ?? 1),
				connected_to: prev?.connected_to ?? [],
				depends_on: prev?.depends_on ?? [],
				default_value: prev?.default_value ?? null,
				data_type: edit.data_type,
				description: edit.description ?? "",
				friendly_name: edit.friendly_name ?? edit.name,
				name: toMachineName(edit.friendly_name ?? edit.name),
				options: edit.options ?? null,
				schema: edit.schema ?? null,
				value_type: edit.value_type ?? IValueType.Normal,
			};

			nextPins[edit.id] = merged;
		}

		const nextInputs = reindex(
			sortByIndex(
				Object.values(nextPins).filter((p) => p.pin_type === IPinType.Input),
			),
		);
		const nextOutputs = reindex(
			sortByIndex(
				Object.values(nextPins).filter((p) => p.pin_type === IPinType.Output),
			),
		);

		for (const p of nextInputs)
			nextPins[p.id] = { ...nextPins[p.id], index: p.index };
		for (const p of nextOutputs)
			nextPins[p.id] = { ...nextPins[p.id], index: p.index };

		const updated: ILayer = {
			...layer,
			pins: nextPins as unknown as ILayer["pins"],
		};

		await onApply(updated);
	}, [edits, layer, onApply]);

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-5xl">
				<DialogHeader>
					<DialogTitle className="flex items-center gap-2">
						<SlidersHorizontalIcon className="h-5 w-5 text-primary" />
						Edit Layer Pins
					</DialogTitle>
					<DialogDescription>
						Configure pin properties and ordering for “{layer.name}”.
					</DialogDescription>
				</DialogHeader>

				<Tabs
					value={tab}
					onValueChange={(v) => setTab(v as any)}
					className="mt-2"
				>
					<TabsList className="grid grid-cols-2 w-full">
						<TabsTrigger value="inputs">Inputs</TabsTrigger>
						<TabsTrigger value="outputs">Outputs</TabsTrigger>
					</TabsList>

					<TabsContent value="inputs" className="mt-3 space-y-2">
						<div className="flex justify-end">
							<Button
								size="sm"
								onClick={() => addPin(IPinType.Input)}
								className="gap-2"
							>
								<PlusIcon className="h-4 w-4" />
								Add Input Pin
							</Button>
						</div>
						<PinList
							items={inputs}
							onEdit={editPin}
							onMoveUp={(id) => movePin(id, "up")}
							onMoveDown={(id) => movePin(id, "down")}
							onRemove={removePin}
							onReorder={reorderByIds}
						/>
					</TabsContent>

					<TabsContent value="outputs" className="mt-3 space-y-2">
						<div className="flex justify-end">
							<Button
								size="sm"
								onClick={() => addPin(IPinType.Output)}
								className="gap-2"
							>
								<PlusIcon className="h-4 w-4" />
								Add Output Pin
							</Button>
						</div>
						<PinList
							items={outputs}
							onEdit={editPin}
							onMoveUp={(id) => movePin(id, "up")}
							onMoveDown={(id) => movePin(id, "down")}
							onRemove={removePin}
							onReorder={reorderByIds}
						/>
					</TabsContent>
				</Tabs>

				<Separator className="my-3" />

				<DialogFooter className="gap-2">
					<Button variant="secondary" onClick={() => onOpenChange(false)}>
						Close
					</Button>
					<Button onClick={applyChanges} className="gap-2">
						<SaveIcon className="h-4 w-4" />
						Save
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
};

// Helpers
const toCSV = (arr?: string[] | null) => (arr && arr.length > 0 ? arr.join(", ") : "");
const fromCSVStrings = (s: string): string[] =>
	s.split(",").map((x) => x.trim()).filter((x) => x.length > 0);

interface PinListProps {
	items: PinEdit[];
	onEdit: (id: string, patch: Partial<PinEdit>) => void;
	onMoveUp: (id: string) => void;
	onMoveDown: (id: string) => void;
	onRemove: (id: string) => void;
	onReorder: (orderedIds: string[]) => void;
}

const PinValueTypeDropdown: React.FC<{
	value_type: IValueType;
	data_type: IVariableType;
	onChange: (vt: IValueType) => void;
	className?: string;
}> = ({ value_type, data_type, onChange, className }) => {
	const color = typeToColor(data_type);
	return (
		<DropdownMenu>
			<DropdownMenuTrigger asChild>
				<button
					type="button"
					className={`inline-flex items-center justify-center rounded p-1 hover:bg-accent/50 ${className ?? ""}`}
					aria-label="Change value type"
				>
					<ValueTypeIcon value_type={value_type} data_type={data_type} />
				</button>
			</DropdownMenuTrigger>
			<DropdownMenuContent align="start">
				<DropdownMenuItem className="gap-2" onClick={() => onChange(IValueType.Normal)}>
					<div className="w-4 h-2 rounded-full" style={{ backgroundColor: color }} />
					Single
				</DropdownMenuItem>
				<DropdownMenuItem className="gap-2" onClick={() => onChange(IValueType.Array)}>
					<GripIcon className="w-4 h-4" style={{ color }} />
					Array
				</DropdownMenuItem>
				<DropdownMenuItem className="gap-2" onClick={() => onChange(IValueType.HashSet)}>
					<EllipsisVerticalIcon className="w-4 h-4" style={{ color }} />
					Set
				</DropdownMenuItem>
				<DropdownMenuItem className="gap-2" onClick={() => onChange(IValueType.HashMap)}>
					<ListIcon className="w-4 h-4" style={{ color }} />
					Map
				</DropdownMenuItem>
			</DropdownMenuContent>
		</DropdownMenu>
	);
};

const PinDataTypeSelectInline: React.FC<{
	value: IVariableType;
	onChange: (dt: IVariableType) => void;
	className?: string;
}> = ({ value, onChange, className }) => {
	return (
		<Select value={value} onValueChange={(val) => onChange(val as IVariableType)}>
			<SelectTrigger className={`h-7 w-[140px] text-xs ${className ?? ""}`}>
				<SelectValue placeholder="Data Type" />
			</SelectTrigger>
			<SelectContent>
				<SelectItem value={IVariableType.Boolean}>
					{selectPreviewElement(IVariableType.Boolean)}
				</SelectItem>
				<SelectItem value={IVariableType.Byte}>{selectPreviewElement(IVariableType.Byte)}</SelectItem>
				<SelectItem value={IVariableType.Date}>{selectPreviewElement(IVariableType.Date)}</SelectItem>
				<SelectItem value={IVariableType.Execution}>{selectPreviewElement(IVariableType.Execution)}</SelectItem>
				<SelectItem value={IVariableType.Float}>{selectPreviewElement(IVariableType.Float)}</SelectItem>
				<SelectItem value={IVariableType.Generic}>{selectPreviewElement(IVariableType.Generic)}</SelectItem>
				<SelectItem value={IVariableType.Integer}>{selectPreviewElement(IVariableType.Integer)}</SelectItem>
				<SelectItem value={IVariableType.PathBuf}>{selectPreviewElement(IVariableType.PathBuf)}</SelectItem>
				<SelectItem value={IVariableType.String}>{selectPreviewElement(IVariableType.String)}</SelectItem>
				<SelectItem value={IVariableType.Struct}>{selectPreviewElement(IVariableType.Struct)}</SelectItem>
			</SelectContent>
		</Select>
	);
};

const PinList: React.FC<PinListProps> = ({
	items,
	onEdit,
	onMoveUp,
	onMoveDown,
	onRemove,
	onReorder,
}) => {
	const sensors = useSensors(
		useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
	);
	const ids = useMemo(() => items.map((p) => p.id), [items]);

	const handleDragEnd = useCallback(
		(e: DragEndEvent) => {
			const { active, over } = e;
			if (!over || active.id === over.id) return;
			const from = ids.indexOf(String(active.id));
			const to = ids.indexOf(String(over.id));
			if (from === -1 || to === -1) return;
			const next = arrayMove(ids, from, to);
			onReorder(next);
		},
		[ids, onReorder],
	);

	return (
		<ScrollArea className="h-96 max-h-96 overflow-auto rounded-md border">
			<div className="p-2 space-y-2">
				{items.length === 0 && (
					<div className="text-sm text-muted-foreground px-2 py-8 text-center">
						No pins in this group.
					</div>
				)}
				<DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
					<SortableContext items={ids} strategy={verticalListSortingStrategy}>
						{items.map((pin, idx) => (
							<SortablePinRow
								key={pin.id}
								pin={pin}
								idx={idx}
								total={items.length}
								onEdit={onEdit}
								onMoveUp={onMoveUp}
								onMoveDown={onMoveDown}
								onRemove={onRemove}
							/>
						))}
					</SortableContext>
				</DndContext>
			</div>
		</ScrollArea>
	);
};

const SortablePinRow: React.FC<{
	pin: PinEdit;
	idx: number;
	total: number;
	onEdit: (id: string, patch: Partial<PinEdit>) => void;
	onMoveUp: (id: string) => void;
	onMoveDown: (id: string) => void;
	onRemove: (id: string) => void;
}> = ({ pin, idx, total, onEdit, onMoveUp, onMoveDown, onRemove }) => {
	const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
		id: pin.id,
	});
	const style: React.CSSProperties = {
		transform: transform ? CSS.Transform.toString(transform) : undefined,
		transition,
	};

	return (
		<div
			ref={setNodeRef}
			style={style}
			className={`group rounded-md border bg-card hover:bg-accent/30 transition-colors ${isDragging ? "opacity-60" : ""}`}
		>
			<div className="flex items-center gap-2 px-2 py-1 border-b">
				<button
					type="button"
					title="Drag to reorder"
					className="inline-flex h-5 w-5 items-center justify-center text-muted-foreground cursor-grab active:cursor-grabbing"
					{...attributes}
					{...listeners}
					onClick={(e) => e.stopPropagation()}
				>
					<GripVerticalIcon className="h-4 w-4" />
				</button>
				<PinValueTypeDropdown
					value_type={pin.value_type}
					data_type={pin.data_type}
					onChange={(vt) => onEdit(pin.id, { value_type: vt })}
					className="shrink-0"
				/>
				<div className="flex-1 truncate text-sm font-medium">
					{pin.friendly_name ?? pin.name ?? pin.id}
					<span className="ml-2 text-[10px] text-muted-foreground">({pin.id})</span>
				</div>
				<PinDataTypeSelectInline
					value={pin.data_type}
					onChange={(dt) => onEdit(pin.id, { data_type: dt })}
					className="hidden sm:flex"
				/>
				<Button variant="ghost" size="icon" onClick={() => onMoveUp(pin.id)} disabled={idx === 0} title="Move up">
					<ArrowUpIcon className="h-4 w-4" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					onClick={() => onMoveDown(pin.id)}
					disabled={idx === total - 1}
					title="Move down"
				>
					<ArrowDownIcon className="h-4 w-4" />
				</Button>
				<Button variant="destructive" size="icon" onClick={() => onRemove(pin.id)} title="Remove pin">
					<Trash2Icon className="h-4 w-4 text-destructive-foreground" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					onClick={() => (document.getElementById(`opts-${pin.id}`) as HTMLButtonElement)?.click()}
					title="Options"
				>
					<SlidersHorizontalIcon className="h-4 w-4" />
				</Button>
			</div>

			<div className="grid grid-cols-1 md:grid-cols-2 gap-3 p-2">
				<div className="space-y-1.5">
					<Label className="text-xs">Friendly Name</Label>
					<Input
						className="h-8"
						value={pin.friendly_name}
						onChange={(e) => onEdit(pin.id, { friendly_name: e.target.value })}
					/>
					<small className="text-[10px] text-muted-foreground">
						Saved as: {toMachineName(pin.friendly_name)}
					</small>
				</div>

				<div className="space-y-1.5">
					<Label className="text-xs">Description</Label>
					<Input
						className="h-8"
						value={pin.description}
						onChange={(e) => onEdit(pin.id, { description: e.target.value })}
						placeholder="Optional"
					/>
				</div>
			</div>

			<div className="sr-only">
				<PinOptionsButton
					pin={pin}
					onApply={(opts) => onEdit(pin.id, { options: opts })}
					onSchemaChange={(schema) => onEdit(pin.id, { schema })}
				/>
			</div>
		</div>
	);
};

interface PinOptionsButtonProps {
	pin: PinEdit;
	onApply: (opts: IPinOptions | null) => void;
	onSchemaChange: (schema: string | null) => void;
}

const PinOptionsButton: React.FC<PinOptionsButtonProps> = ({
	pin,
	onApply,
	onSchemaChange,
}) => {
	const [open, setOpen] = useState(false);
	const [local, setLocal] = useState<IPinOptions | null>(pin.options ?? null);
	const [localSchema, setLocalSchema] = useState<string>(pin.schema ?? "");

	useEffect(() => {
		if (open) {
			setLocal(pin.options ?? null);
			setLocalSchema(pin.schema ?? "");
		}
	}, [open, pin.options, pin.schema]);

	return (
		<>
			<Button
				id={`opts-${pin.id}`}
				variant="outline"
				size="sm"
				className="h-7 px-2"
				onClick={() => setOpen(true)}
				title="Edit pin options"
			>
				Options…
			</Button>
			<Dialog open={open} onOpenChange={setOpen}>
				<DialogContent className="sm:max-w-lg">
					<DialogHeader>
						<DialogTitle>Pin Options — {pin.friendly_name}</DialogTitle>
						<DialogDescription>Advanced, optional settings.</DialogDescription>
					</DialogHeader>

					<div className="grid grid-cols-1 md:grid-cols-6 gap-3">
						<div className="space-y-1.5 md:col-span-6">
							<Label className="text-xs">Schema</Label>
							<Input
								className="h-8"
								value={localSchema}
								onChange={(e) => setLocalSchema(e.target.value)}
								placeholder="e.g. my.schema.Identifier"
							/>
						</div>

						<div className="flex items-center gap-2 md:col-span-3">
							<input
								id={`opt-egvt-${pin.id}`}
								type="checkbox"
								className="h-4 w-4"
								checked={Boolean(local?.enforce_generic_value_type)}
								onChange={(e) =>
									setLocal({
										...(local ?? {}),
										enforce_generic_value_type: e.target.checked,
									} as IPinOptions)
								}
							/>
							<Label htmlFor={`opt-egvt-${pin.id}`} className="text-xs">
								Enforce Generic VT
							</Label>
						</div>

						<div className="flex items-center gap-2 md:col-span-3">
							<input
								id={`opt-es-${pin.id}`}
								type="checkbox"
								className="h-4 w-4"
								checked={Boolean(local?.enforce_schema)}
								onChange={(e) =>
									setLocal({
										...(local ?? {}),
										enforce_schema: e.target.checked,
									} as IPinOptions)
								}
							/>
							<Label htmlFor={`opt-es-${pin.id}`} className="text-xs">
								Enforce Schema
							</Label>
						</div>

						<div className="flex items-center gap-2 md:col-span-3">
							<input
								id={`opt-sens-${pin.id}`}
								type="checkbox"
								className="h-4 w-4"
								checked={Boolean(local?.sensitive)}
								onChange={(e) =>
									setLocal({
										...(local ?? {}),
										sensitive: e.target.checked,
									} as IPinOptions)
								}
							/>
							<Label htmlFor={`opt-sens-${pin.id}`} className="text-xs">
								Sensitive
							</Label>
						</div>

						<div className="space-y-1.5 md:col-span-3">
							<Label className="text-xs">Step</Label>
							<Input
								className="h-8"
								type="number"
								value={local?.step ?? ""}
								onChange={(e) =>
									setLocal({
										...(local ?? {}),
										step: e.target.value === "" ? null : Number(e.target.value),
									} as IPinOptions)
								}
							/>
						</div>

						<div className="space-y-1.5 md:col-span-3">
							<Label className="text-xs">Range Min</Label>
							<Input
								className="h-8"
								type="number"
								value={local?.range?.[0] ?? ""}
								onChange={(e) => {
									const min =
										e.target.value === "" ? undefined : Number(e.target.value);
									const max = local?.range?.[1];
									const nextRange = [
										Number.isFinite(min as number)
											? (min as number)
											: undefined,
										Number.isFinite(max as number)
											? (max as number)
											: undefined,
									].filter((x) => typeof x === "number") as number[];
									setLocal({
										...(local ?? {}),
										range:
											nextRange.length === 2
												? nextRange
												: nextRange.length === 1
													? [nextRange[0]]
													: null,
									} as IPinOptions);
								}}
							/>
						</div>

						<div className="space-y-1.5 md:col-span-3">
							<Label className="text-xs">Range Max</Label>
							<Input
								className="h-8"
								type="number"
								value={local?.range?.[1] ?? ""}
								onChange={(e) => {
									const min = local?.range?.[0];
									const max =
										e.target.value === "" ? undefined : Number(e.target.value);
									const nextRange = [
										Number.isFinite(min as number)
											? (min as number)
											: undefined,
										Number.isFinite(max as number)
											? (max as number)
											: undefined,
									].filter((x) => typeof x === "number") as number[];
									setLocal({
										...(local ?? {}),
										range:
											nextRange.length === 2
												? nextRange
												: nextRange.length === 1
													? [nextRange[0]]
													: null,
									} as IPinOptions);
								}}
							/>
						</div>

						<div className="space-y-1.5 md:col-span-6">
							<Label className="text-xs">Valid Values (comma-separated)</Label>
							<Input
								className="h-8"
								value={toCSV(local?.valid_values ?? null)}
								onChange={(e) =>
									setLocal({
										...(local ?? {}),
										valid_values:
											e.target.value.trim() === ""
												? null
												: fromCSVStrings(e.target.value),
									} as IPinOptions)
								}
							/>
						</div>
					</div>

					<DialogFooter className="gap-2">
						<Button variant="secondary" onClick={() => setOpen(false)}>
							Close
						</Button>
						<Button
							onClick={() => {
								onApply(local ?? null);
								onSchemaChange(
									localSchema.trim() === "" ? null : localSchema.trim(),
								);
								setOpen(false);
							}}
						>
							Save
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</>
	);
};
