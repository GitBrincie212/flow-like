import { Channel, invoke } from "@tauri-apps/api/core";
import {
	type IEvent,
	type IEventState,
	type IIntercomEvent,
	type ILogMetadata,
	type IRunPayload,
	type IVersionType,
	injectDataFunction,
	isEqual,
} from "@tm9657/flow-like-ui";
import { fetcher } from "../../lib/api";
import type { TauriBackend } from "../tauri-provider";

export class EventState implements IEventState {
	constructor(private readonly backend: TauriBackend) {}

	async getEvent(
		appId: string,
		eventId: string,
		version?: [number, number, number],
	): Promise<IEvent> {
		const event = await invoke<IEvent>("get_event", {
			appId: appId,
			eventId: eventId,
			version: version,
		});

		const isOffline = await this.backend.isOffline(appId);
		if (
			isOffline ||
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			return event;
		}

		const promise = injectDataFunction(
			async () => {
				let url = `apps/${appId}/events/${eventId}`;
				if (version) {
					url += `?version=${version.join("_")}`;
				}
				const remoteData = await fetcher<IEvent>(
					this.backend.profile!,
					url,
					{
						method: "GET",
					},
					this.backend.auth,
				);

				if (!remoteData) {
					throw new Error("Failed to fetch event data");
				}

				if (!isEqual(remoteData, event) && typeof version === "undefined") {
					await invoke("upsert_event", {
						appId: appId,
						event: remoteData,
						enforceId: true,
					});
				}

				return remoteData;
			},
			this,
			this.backend.queryClient,
			this.getEvent,
			[appId, eventId, version],
			[],
			event,
		);

		this.backend.backgroundTaskHandler(promise);
		return event;
	}
	async getEvents(appId: string): Promise<IEvent[]> {
		const events = await invoke<IEvent[]>("get_events", {
			appId: appId,
		});
		const isOffline = await this.backend.isOffline(appId);
		if (
			isOffline ||
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			return events;
		}

		const promise = injectDataFunction(
			async () => {
				const remoteData = await fetcher<IEvent[]>(
					this.backend.profile!,
					`apps/${appId}/events`,
					{
						method: "GET",
					},
					this.backend.auth,
				);

				for (const event of remoteData) {
					await invoke("upsert_event", {
						appId: appId,
						event: event,
						enforceId: true,
					});
				}

				return remoteData;
			},
			this,
			this.backend.queryClient,
			this.getEvents,
			[appId],
			[],
			events,
		);

		this.backend.backgroundTaskHandler(promise);
		return events;
	}
	async getEventVersions(
		appId: string,
		eventId: string,
	): Promise<[number, number, number][]> {
		const versions = await invoke<[number, number, number][]>(
			"get_event_versions",
			{
				appId: appId,
				eventId: eventId,
			},
		);

		const isOffline = await this.backend.isOffline(appId);
		if (
			isOffline ||
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			return versions;
		}

		const promise = injectDataFunction(
			async () => {
				const remoteData = await fetcher<[number, number, number][]>(
					this.backend.profile!,
					`apps/${appId}/events/${eventId}/versions`,
					{
						method: "GET",
					},
					this.backend.auth,
				);

				return remoteData;
			},
			this,
			this.backend.queryClient,
			this.getEventVersions,
			[appId, eventId],
			[],
			versions,
		);

		this.backend.backgroundTaskHandler(promise);
		return versions;
	}
	async upsertEvent(
		appId: string,
		event: IEvent,
		versionType?: IVersionType,
	): Promise<IEvent> {
		const isOffline = await this.backend.isOffline(appId);
		if (isOffline) {
			return await invoke("upsert_event", {
				appId: appId,
				event: event,
				versionType: versionType,
			});
		}
		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot upsert event.",
			);
		}
		const response = await fetcher<IEvent>(
			this.backend.profile,
			`apps/${appId}/events/${event.id}`,
			{
				method: "PUT",
				body: JSON.stringify({
					event: event,
					version_type: versionType,
				}),
			},
			this.backend.auth,
		);
		await invoke("upsert_event", {
			appId: appId,
			event: response,
			versionType: versionType,
			enforceId: true,
		});
		return response;
	}
	async deleteEvent(appId: string, eventId: string): Promise<void> {
		const isOffline = await this.backend.isOffline(appId);
		if (isOffline) {
			await invoke("delete_event", {
				appId: appId,
				eventId: eventId,
			});
		}

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot delete event.",
			);
		}

		await fetcher(
			this.backend.profile,
			`apps/${appId}/events/${eventId}`,
			{
				method: "DELETE",
			},
			this.backend.auth,
		);
		await invoke("delete_event", {
			appId: appId,
			eventId: eventId,
		});
	}
	async validateEvent(
		appId: string,
		eventId: string,
		version?: [number, number, number],
	): Promise<void> {
		const isOffline = await this.backend.isOffline(appId);
		if (isOffline) {
			return await invoke("validate_event", {
				appId: appId,
				eventId: eventId,
				version: version,
			});
		}

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot validate event.",
			);
		}

		return await fetcher(
			this.backend.profile,
			`apps/${appId}/events/${eventId}/validate`,
			{
				method: "POST",
				body: JSON.stringify({
					version: version,
				}),
			},
			this.backend.auth,
		);
	}
	async upsertEventFeedback(
		appId: string,
		eventId: string,
		feedbackId: string,
		feedback: {
			rating: number;
			history?: any[];
			globalState?: Record<string, any>;
			localState?: Record<string, any>;
			comment?: string;
		},
	): Promise<string> {
		const isOffline = await this.backend.isOffline(appId);
		if (isOffline) return "";

		if (
			!this.backend.profile ||
			!this.backend.auth ||
			!this.backend.queryClient
		) {
			throw new Error(
				"Profile, auth or query client not set. Cannot upsert event feedback.",
			);
		}

		const response = await fetcher<{ feedback_id: string }>(
			this.backend.profile,
			`apps/${appId}/events/${eventId}/feedback`,
			{
				method: "PUT",
				body: JSON.stringify({
					rating: feedback.rating,
					context: {
						history: feedback.history,
						global_state: feedback.globalState,
						local_state: feedback.localState,
					},
					comment: feedback.comment,
					feedback_id: feedbackId,
				}),
			},
			this.backend.auth,
		);

		return response.feedback_id;
	}
	async executeEvent(
		appId: string,
		eventId: string,
		payload: IRunPayload,
		streamState?: boolean,
		onEventId?: (id: string) => void,
		cb?: (event: IIntercomEvent[]) => void,
	): Promise<ILogMetadata | undefined> {
		const channel = new Channel<IIntercomEvent[]>();
		let closed = false;
		let foundRunId = false;

		channel.onmessage = (events: IIntercomEvent[]) => {
			if (closed) {
				console.warn("Channel closed, ignoring events");
				return;
			}

			if (!foundRunId && events.length > 0 && eventId) {
				const runId_event = events.find(
					(event) => event.event_type === "run_initiated",
				);

				if (runId_event) {
					const runId = runId_event.payload.run_id;
					onEventId?.(runId);
					foundRunId = true;
				}
			}

			if (cb) cb(events);
		};

		const metadata: ILogMetadata | undefined = await invoke("execute_event", {
			appId: appId,
			eventId: eventId,
			payload: payload,
			events: channel,
			streamState: streamState,
		});

		closed = true;

		return metadata;
	}

	async cancelExecution(runId: string): Promise<void> {
		await invoke("cancel_execution", {
			runId: runId,
		});
	}
}
