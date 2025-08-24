"use client";
import {
	Button,
	Card,
	CardHeader,
	CardTitle,
	Input,
	ScrollArea,
	useBackend,
	useInvoke,
} from "@tm9657/flow-like-ui";
import LanceDBExplorer from "@tm9657/flow-like-ui/components/ui/lance-viewer";
import {
	ArrowDownAZ,
	ArrowLeftIcon,
	ArrowUpAZ,
	ChevronRight,
	Columns,
	Database,
	RefreshCw,
	Search,
	X,
} from "lucide-react";
import {
	type ReadonlyURLSearchParams,
	usePathname,
	useRouter,
	useSearchParams,
} from "next/navigation";
import type React from "react";
import { useCallback, useMemo, useState } from "react";
import NotFound from "../not-found";

export default function Page(): React.ReactElement {
	const router = useRouter();
	const searchParams = useSearchParams();
	const id = searchParams?.get("id") ?? null;
	const tableParam = searchParams?.get("table") ?? null;

	const pathname = usePathname();

	const table = useMemo(() => {
		if (!tableParam) return "";
		try {
			return decodeURIComponent(tableParam);
		} catch {
			return tableParam;
		}
	}, [tableParam]);

	if (!id) return <NotFound />;

	return table ? (
		<TableView
			table={table}
			appId={id}
			onBack={() => {
				const params = new URLSearchParams(searchParams?.toString() ?? "");
				params.delete("table");
				router.push(`${pathname}?${params.toString()}`);
			}}
		/>
	) : (
		<DatabaseOverview appId={id} searchParams={searchParams} />
	);
}

function TableView({
	table,
	appId,
	onBack,
}: Readonly<{ table: string; appId: string; onBack: () => void }>) {
	const backend = useBackend();
	const schema = useInvoke(backend.dbState.getSchema, backend.dbState, [
		appId,
		table,
	]);
	const [offset, setOffset] = useState(0);
	const [limit, setLimit] = useState(25);
	const list = useInvoke(backend.dbState.listItems, backend.dbState, [
		appId,
		table,
		offset,
		limit,
	]);

	return (
		<div className="flex flex-col h-full flex-grow max-h-full overflow-hidden">
			{schema.data && list.data && (
				<LanceDBExplorer
					tableName={table}
					arrowSchema={schema.data}
					rows={list.data}
					onPageRequest={(args) => {
						setOffset((args.page - 1) * args.pageSize);
						setLimit(args.pageSize);
					}}
					loading={list.isLoading}
					error={list.error?.message}
				>
					<Button
						variant={"default"}
						size={"sm"}
						onClick={() => {
							onBack();
						}}
					>
						<ArrowLeftIcon />
						Back
					</Button>
				</LanceDBExplorer>
			)}
		</div>
	);
}

interface DatabaseOverviewProps {
	appId: string;
	searchParams: ReadonlyURLSearchParams;
}

interface Table {
	name: string;
	rowCount?: number;
}

const DatabaseOverview: React.FC<DatabaseOverviewProps> = ({
	appId,
	searchParams,
}) => {
	const backend = useBackend();
	const router = useRouter();
	const pathname = usePathname();
	const tables = useInvoke(backend.dbState.listTables, backend.dbState, [
		appId,
	]);

	const [query, setQuery] = useState<string>("");
	const [sortAsc, setSortAsc] = useState<boolean>(true);

	const processedTables = useMemo(() => {
		return (tables.data ?? []).map((name): Table => ({ name }));
	}, [tables.data]);

	const filteredAndSortedTables = useMemo(() => {
		const collator = new Intl.Collator(undefined, {
			numeric: true,
			sensitivity: "base",
		});

		const queryLower = query.trim().toLowerCase();

		return processedTables
			.filter(
				(table) => !queryLower || table.name.toLowerCase().includes(queryLower),
			)
			.sort((a, b) =>
				sortAsc
					? collator.compare(a.name, b.name)
					: collator.compare(b.name, a.name),
			);
	}, [processedTables, query, sortAsc]);

	const navigateToTable = useCallback(
		(tableName: string) => {
			const params = new URLSearchParams(searchParams?.toString() ?? "");
			params.set("table", encodeURIComponent(tableName));
			router.push(`${pathname}?${params.toString()}`);
		},
		[router, pathname, searchParams],
	);

	const refreshTables = useCallback(() => {
		tables.refetch();
	}, [tables.refetch]);

	const clearSearch = useCallback(() => {
		setQuery("");
	}, []);

	const toggleSort = useCallback(() => {
		setSortAsc((prev) => !prev);
	}, []);

	if (tables.isLoading) {
		return <LoadingState />;
	}

	if (tables.error) {
		return <ErrorState onRetry={refreshTables} />;
	}

	if (!processedTables.length) {
		return <EmptyState onRetry={refreshTables} />;
	}

	return (
		<div className="p-6 space-y-6">
			<DatabaseHeader
				sortAsc={sortAsc}
				onToggleSort={toggleSort}
				onRefresh={refreshTables}
			/>

			<SearchInput value={query} onChange={setQuery} onClear={clearSearch} />

			<TableGrid
				tables={filteredAndSortedTables}
				onSelectTable={navigateToTable}
				searchQuery={query}
			/>
		</div>
	);
};

interface DatabaseHeaderProps {
	sortAsc: boolean;
	onToggleSort: () => void;
	onRefresh: () => void;
}

const DatabaseHeader: React.FC<DatabaseHeaderProps> = ({
	sortAsc,
	onToggleSort,
	onRefresh,
}) => (
	<header className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between w-full flex-grow">
		<div className="flex items-center gap-4 w-full">
			<Database className="h-8 w-8 text-primary" />
			<div>
				<h1 className="text-2xl font-semibold">Database Tables</h1>
				<p className="text-sm text-muted-foreground">
					Browse and inspect your project&apos;s database schema
				</p>
			</div>
		</div>

		<div className="flex flex-row items-center gap-2 justify-end w-full">
			<Button
				variant="ghost"
				size="icon"
				onClick={onToggleSort}
				title={`Sort ${sortAsc ? "descending" : "ascending"}`}
			>
				{sortAsc ? (
					<ArrowUpAZ className="h-4 w-4" />
				) : (
					<ArrowDownAZ className="h-4 w-4" />
				)}
			</Button>
			<Button variant="outline" size="sm" onClick={onRefresh}>
				<RefreshCw className="mr-2 h-4 w-4" />
				Refresh
			</Button>
		</div>
	</header>
);

interface SearchInputProps {
	value: string;
	onChange: (value: string) => void;
	onClear: () => void;
}

const SearchInput: React.FC<SearchInputProps> = ({
	value,
	onChange,
	onClear,
}) => (
	<div className="relative max-w-xl">
		<Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground pointer-events-none" />
		<Input
			value={value}
			onChange={(e) => onChange(e.target.value)}
			placeholder="Search tables..."
			className="pl-9 pr-9"
		/>
		{value && (
			<Button
				variant="ghost"
				size="sm"
				onClick={onClear}
				className="absolute right-1 top-1 h-8 w-8 p-0"
				title="Clear search"
			>
				<X className="h-4 w-4" />
			</Button>
		)}
	</div>
);

interface TableGridProps {
	tables: Table[];
	onSelectTable: (tableName: string) => void;
	searchQuery: string;
}

const TableGrid: React.FC<TableGridProps> = ({
	tables,
	onSelectTable,
	searchQuery,
}) => {
	if (!tables.length && searchQuery) {
		return (
			<div className="rounded-lg border bg-card p-8 text-center">
				<Search className="mx-auto h-10 w-10 text-muted-foreground mb-4" />
				<h3 className="text-lg font-semibold mb-2">No matches found</h3>
				<p className="text-sm text-muted-foreground">
					No tables match &quot;<span className="font-medium">{searchQuery}</span>&quot;.
				</p>
			</div>
		);
	}

	return (
		<ScrollArea className="max-h-[calc(100vh-16rem)]">
			<div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pr-2 py-1">
				{tables.map((table) => (
					<TableCard
						key={table.name}
						table={table}
						onSelect={() => onSelectTable(table.name)}
					/>
				))}
			</div>
		</ScrollArea>
	);
};

interface TableCardProps {
	table: Table;
	onSelect: () => void;
}

const TableCard: React.FC<TableCardProps> = ({ table, onSelect }) => (
	<Card className="group cursor-pointer transition-all duration-200 hover:shadow-md hover:-translate-y-0.5 hover:bg-primary/50">
		<button
			onClick={onSelect}
			className="w-full h-auto p-0 rounded-lg"
			title={`Open table: ${table.name}`}
		>
			<CardHeader className="w-full">
				<div className="flex items-center justify-between gap-3 w-full">
					<div className="flex items-center gap-3 min-w-0 flex-1">
						<div className="rounded-md bg-muted p-2 transition-colors group-hover:bg-primary/10">
							<Columns className="h-5 w-5 text-muted-foreground transition-colors group-hover:text-primary" />
						</div>
						<CardTitle className="text-base text-left truncate flex-1">
							{table.name}
						</CardTitle>
					</div>
					<ChevronRight className="h-4 w-4 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
				</div>
			</CardHeader>
		</button>
	</Card>
);

const LoadingState: React.FC = () => (
	<div className="p-6">
		<div className="flex items-center gap-4 mb-6">
			<Database className="h-8 w-8 text-muted-foreground animate-pulse" />
			<div>
				<div className="h-8 w-48 bg-muted animate-pulse rounded mb-2" />
				<div className="h-4 w-72 bg-muted animate-pulse rounded" />
			</div>
		</div>
		<div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
			{Array.from({ length: 8 }).map((_, i) => (
				<Card key={i} className="h-20 animate-pulse bg-muted/50" />
			))}
		</div>
	</div>
);

const ErrorState: React.FC<{ onRetry: () => void }> = ({ onRetry }) => (
	<div className="p-6">
		<div className="rounded-lg border bg-card p-8 text-center">
			<Database className="mx-auto h-10 w-10 text-destructive mb-4" />
			<h3 className="text-lg font-semibold mb-2">Failed to load tables</h3>
			<p className="text-sm text-muted-foreground mb-4">
				There was an error loading the database tables.
			</p>
			<Button onClick={onRetry}>
				<RefreshCw className="mr-2 h-4 w-4" />
				Try again
			</Button>
		</div>
	</div>
);

const EmptyState: React.FC<{ onRetry: () => void }> = ({ onRetry }) => (
	<div className="p-6">
		<div className="rounded-lg border bg-card p-8 text-center">
			<Database className="mx-auto h-10 w-10 text-muted-foreground mb-4" />
			<h3 className="text-lg font-semibold mb-2">No tables found</h3>
			<p className="text-sm text-muted-foreground mb-4">
				This project doesn&apos;t appear to have any database tables yet.
			</p>
			<Button onClick={onRetry}>
				<RefreshCw className="mr-2 h-4 w-4" />
				Refresh
			</Button>
		</div>
	</div>
);
