import type {
	IApp,
	IAppCategory,
	IAppVisibility,
	IBoard,
	IMetadata,
} from "../../lib";
import type { IAppSearchSort } from "../../lib/schema/app/app-search-query";

export interface IAppState {
	createApp(
		metadata: IMetadata,
		bits: string[],
		online: boolean,
		template?: IBoard,
	): Promise<IApp>;
	deleteApp(appId: string): Promise<void>;
	searchApps(
		id?: string,
		query?: string,
		language?: string,
		category?: IAppCategory,
		author?: string,
		sort?: IAppSearchSort,
		tag?: string,
		offset?: number,
		limit?: number,
	): Promise<[IApp, IMetadata | undefined][]>;
	getApps(): Promise<[IApp, IMetadata | undefined][]>;
	getApp(appId: string): Promise<IApp>;
	updateApp(app: IApp): Promise<void>;
	getAppMeta(appId: string, language?: string): Promise<IMetadata>;
	pushAppMeta(
		appId: string,
		metadata: IMetadata,
		language?: string,
	): Promise<void>;
	changeAppVisibility(appId: string, visibility: IAppVisibility): Promise<void>;
}
