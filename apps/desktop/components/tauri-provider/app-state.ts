import { createId } from "@paralleldrive/cuid2";
import { invoke } from "@tauri-apps/api/core";
import {
	type IApp,
	type IAppCategory,
	type IAppState,
	IAppVisibility,
	type IBoard,
	IExecutionStage,
	ILogLevel,
	type IMetadata,
	injectDataFunction,
} from "@tm9657/flow-like-ui";
import type { IAppSearchSort } from "@tm9657/flow-like-ui/lib/schema/app/app-search-query";
import { fetcher, put } from "../../lib/api";
import { appsDB } from "../../lib/apps-db";
import type { TauriBackend } from "../tauri-provider";
import { IMediaItem } from "@tm9657/flow-like-ui/state/backend-state/app-state";
export class AppState implements IAppState {
	constructor(private readonly backend: TauriBackend) {}

	async createApp(
		metadata: IMetadata,
		bits: string[],
		online: boolean,
		template?: IBoard,
	): Promise<IApp> {
		let appId: string | undefined;
		if (online && this.backend.profile) {
			const app: IApp = await put(
				this.backend.profile,
				`apps/new`,
				{
					meta: metadata,
				},
				this.backend.auth,
			);

			await appsDB.visibility.put({
				visibility: IAppVisibility.Private,
				appId: app.id,
			});

			appId = app.id;
		}

		const app: IApp = await invoke("create_app", {
			metadata: metadata,
			bits: bits,
			id: appId,
		});

		if (appId) {
			await invoke("update_app", {
				app: { ...app, visibility: IAppVisibility.Private },
			});
		}

		await this.backend.boardState.upsertBoard(
			app.id,
			createId(),
			template?.name ?? "Initial Board",
			template?.description ?? "A blank canvas ready for your ideas",
			template?.log_level ?? ILogLevel.Debug,
			IExecutionStage.Dev,
			template,
		);

		return app;
	}
	async deleteApp(appId: string): Promise<void> {
		const isOffline = await this.backend.isOffline(appId);
		if (isOffline) {
			await invoke("delete_app", {
				appId: appId,
			});
			return;
		}

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot delete app.",
			);
		}

		await fetcher(
			this.backend.profile,
			`apps/${appId}`,
			{
				method: "DELETE",
			},
			this.backend.auth,
		);
		await invoke("delete_app", {
			appId: appId,
		});
	}

	async searchApps(
		id?: string,
		query?: string,
		language?: string,
		category?: IAppCategory,
		author?: string,
		sort?: IAppSearchSort,
		tag?: string,
		offset?: number,
		limit?: number,
	): Promise<[IApp, IMetadata | undefined][]> {
		if (!this.backend.profile) {
			throw new Error("Profile not set. Cannot search apps.");
		}

		const queryParams: Record<string, string> = {};

		if (id) queryParams["id"] = id;
		if (query) queryParams["query"] = query;
		if (language) queryParams["language"] = language;
		if (category) queryParams["category"] = category;
		if (author) queryParams["author"] = author;
		if (sort) queryParams["sort"] = sort;
		if (tag) queryParams["tag"] = tag;
		if (offset) queryParams["offset"] = offset.toString();
		if (limit) queryParams["limit"] = limit.toString();

		const length = Array.from(Object.values(queryParams)).length;
		if (length === 0) {
			return this.getApps();
		}

		return await fetcher(
			this.backend.profile,
			`apps/search?${new URLSearchParams(queryParams)}`,
			undefined,
			this.backend.auth,
		);
	}

	async getApps(): Promise<[IApp, IMetadata | undefined][]> {
		const localApps = await invoke<[IApp, IMetadata | undefined][]>("get_apps");

		if (
			!this?.backend?.queryClient ||
			!this.backend.profile ||
			!this.backend.auth
		) {
			console.warn(
				"Query client, profile or auth context not available, returning local apps only.",
			);
			console.warn({
				queryClient: this?.backend?.queryClient,
				profile: this?.backend?.profile,
				auth: this?.backend?.auth,
			});
			return localApps;
		}

		const promise = injectDataFunction(
			async () => {
				const remoteData = await fetcher<[IApp, IMetadata | undefined][]>(
					this.backend.profile!,
					"apps",
					undefined,
					this.backend.auth,
				);

				const mergedData = new Map<string, [IApp, IMetadata | undefined]>();

				for (const [app, meta] of remoteData) {
					mergedData.set(app.id, [app, meta]);
					await appsDB.visibility.put({
						visibility: app.visibility ?? IAppVisibility.Private,
						appId: app.id,
					});

					const exists = localApps.find(([localApp]) => localApp.id === app.id);
					if (exists) {
						await invoke("update_app", {
							app: app,
						});
						if (meta)
							await invoke("push_app_meta", {
								appId: app.id,
								metadata: meta,
							});
						continue;
					}

					if (meta)
						await invoke("create_app", {
							metadata: meta,
							bits: app.bits,
							template: "",
							id: app.id,
						});
				}

				localApps.forEach(([app, meta]) => {
					if (!mergedData.has(app.id)) {
						mergedData.set(app.id, [app, meta]);
					}
				});

				return Array.from(mergedData.values());
			},
			this,
			this.backend.queryClient,
			this.getApps,
			[],
			[],
		);
		this.backend.backgroundTaskHandler(promise);

		return localApps;
	}
	async getApp(appId: string): Promise<IApp> {
		const localApp: IApp = await invoke("get_app", {
			appId: appId,
		});

		if (
			!this?.backend?.queryClient ||
			!this.backend.profile ||
			!this.backend.auth?.isAuthenticated
		) {
			console.warn(
				"Query client, profile or auth context not available, returning local app only.",
			);

			return localApp;
		}

		const promise = injectDataFunction(
			async () => {
				const remoteData = await fetcher<IApp>(
					this.backend.profile!,
					`apps/${appId}`,
					undefined,
					this.backend.auth,
				);

				await invoke("update_app", {
					app: remoteData,
				});

				await appsDB.visibility.put({
					visibility: remoteData.visibility ?? IAppVisibility.Private,
					appId: remoteData.id,
				});

				return remoteData;
			},
			this,
			this.backend.queryClient,
			this.getApp,
			[appId],
			[],
		);
		this.backend.backgroundTaskHandler(promise);

		return localApp;
	}
	async updateApp(app: IApp): Promise<void> {
		const isOffline = await this.backend.isOffline(app.id);

		if (isOffline) {
			await invoke("update_app", {
				app: app,
			});
			return;
		}

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot update app.",
			);
		}

		await fetcher(
			this.backend.profile,
			`apps/${app.id}`,
			{
				method: "PUT",
				body: JSON.stringify({
					app: app,
				}),
			},
			this.backend.auth,
		);
	}

	async getAppMeta(appId: string, language?: string): Promise<IMetadata> {
		const isOffline = await this.backend.isOffline(appId);
		let meta: IMetadata | undefined = undefined;

		try {
			meta = await invoke<IMetadata>("get_app_meta", {
				appId: appId,
				language,
			});
			if (isOffline) {
				return meta;
			}
		} catch (e) {
			console.warn("Failed to get app meta from local cache:", e);
		}

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot get app meta.",
			);
		}

		const remoteDataPromise = fetcher<IMetadata>(
			this.backend.profile,
			`apps/${appId}/meta?language=${language ?? "en"}`,
			undefined,
			this.backend.auth,
		);

		if (meta) {
			const promise = injectDataFunction(
				async () => {
					const remoteMeta: IMetadata = await remoteDataPromise;

					await invoke("push_app_meta", {
						appId: appId,
						metadata: remoteMeta,
						language,
					});

					return remoteMeta;
				},
				this,
				this.backend.queryClient,
				this.getAppMeta,
				[appId, language],
				[],
			);
			this.backend.backgroundTaskHandler(promise);

			return meta;
		}

		const remoteMeta: IMetadata = await remoteDataPromise;

		if (remoteMeta) {
			await invoke("push_app_meta", {
				appId: appId,
				metadata: remoteMeta,
				language,
			});
		}

		return remoteMeta;
	}

	async pushAppMeta(
		appId: string,
		metadata: IMetadata,
		language?: string,
	): Promise<void> {
		const isOffline = await this.backend.isOffline(appId);

		if (isOffline) {
			await invoke("push_app_meta", {
				appId: appId,
				metadata: metadata,
				language,
			});
			return;
		}

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot push app meta.",
			);
		}
		await fetcher(
			this.backend.profile,
			`apps/${appId}/meta?language=${language ?? "en"}`,
			{
				method: "PUT",
				body: JSON.stringify(metadata),
			},
			this.backend.auth,
		);
		await invoke("push_app_meta", {
			appId: appId,
			metadata: metadata,
			language,
		});
	}

	async pushAppMedia(
		appId: string,
		item: IMediaItem,
		file: File,
		language?: string,
	): Promise<void> {
		const isOffline = await this.backend.isOffline(appId);

		if (isOffline) {
			// TODO: offline media handling
			return;
		}

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot push app meta.",
			);
		}
		const {signed_url} : {signed_url: string} = await fetcher(
			this.backend.profile,
			`apps/${appId}/meta/media?language=${language ?? "en"}&item=${item}&extension=${file.name.split(".").pop()}`,
			{
				method: "PUT",
			},
			this.backend.auth,
		);

		await fetch(signed_url, {
			method: "PUT",
			body: file,
			headers: {
				"Content-Type": file.type,
			},
		});

		// TODO: handle media update in local cache
	}

	async changeAppVisibility(
		appId: string,
		visibility: IAppVisibility,
	): Promise<void> {
		if (this.backend.profile && this.backend.auth && this.backend.queryClient) {
			await fetcher<IApp>(
				this.backend.profile,
				`apps/${appId}/visibility`,
				{
					method: "PATCH",
					body: JSON.stringify({
						visibility: visibility,
					}),
				},
				this.backend.auth,
			);
		}
	}
}
