import type {
	IBoard,
	IExecutionStage,
	IGenericCommand,
	IIntercomEvent,
	ILog,
	ILogLevel,
	ILogMetadata,
	INode,
	IRunPayload,
	IVersionType,
} from "../../lib";

export interface IBoardState {
	getBoards(appId: string): Promise<IBoard[]>;
	getCatalog(): Promise<INode[]>;
	getBoard(
		appId: string,
		boardId: string,
		version?: [number, number, number],
	): Promise<IBoard>;
	createBoardVersion(
		appId: string,
		boardId: string,
		versionType: IVersionType,
	): Promise<[number, number, number]>;
	getBoardVersions(
		appId: string,
		boardId: string,
	): Promise<[number, number, number][]>;
	deleteBoard(appId: string, boardId: string): Promise<void>;
	// [AppId, BoardId, BoardName]
	getOpenBoards(): Promise<[string, string, string][]>;
	getBoardSettings(): Promise<"straight" | "step" | "simpleBezier">;

	executeBoard(
		appId: string,
		boardId: string,
		payload: IRunPayload,
		streamState?: boolean,
		eventId?: (id: string) => void,
		cb?: (event: IIntercomEvent[]) => void,
	): Promise<ILogMetadata | undefined>;

	listRuns(
		appId: string,
		boardId: string,
		nodeId?: string,
		from?: number,
		to?: number,
		status?: ILogLevel,
		lastMeta?: ILogMetadata,
		offset?: number,
		limit?: number,
	): Promise<ILogMetadata[]>;
	queryRun(
		logMeta: ILogMetadata,
		query: string,
		offset?: number,
		limit?: number,
	): Promise<ILog[]>;

	undoBoard(
		appId: string,
		boardId: string,
		commands: IGenericCommand[],
	): Promise<void>;
	redoBoard(
		appId: string,
		boardId: string,
		commands: IGenericCommand[],
	): Promise<void>;

	upsertBoard(
		appId: string,
		boardId: string,
		name: string,
		description: string,
		logLevel: ILogLevel,
		stage: IExecutionStage,
		template?: IBoard,
	): Promise<void>;

	closeBoard(boardId: string): Promise<void>;

	executeCommand(
		appId: string,
		boardId: string,
		command: IGenericCommand,
	): Promise<IGenericCommand>;

	executeCommands(
		appId: string,
		boardId: string,
		commands: IGenericCommand[],
	): Promise<IGenericCommand[]>;
}
