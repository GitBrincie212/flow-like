import type {
	IBit,
	IBitPack,
	IBitState,
	IDownloadProgress,
	ISettingsProfile,
} from "@tm9657/flow-like-ui";
import type { IBitSearchQuery } from "@tm9657/flow-like-ui/lib/schema/hub/bit-search-query";

export class EmptyBitState implements IBitState {
	getInstalledBit(bits: IBit[]): Promise<IBit[]> {
		throw new Error("Method not implemented.");
	}
	getPackFromBit(bit: IBit): Promise<{ bits: IBit[] }> {
		throw new Error("Method not implemented.");
	}
	downloadBit(
		bit: IBit,
		pack: IBitPack,
		cb?: (progress: IDownloadProgress[]) => void,
	): Promise<IBit[]> {
		throw new Error("Method not implemented.");
	}
	deleteBit(bit: IBit): Promise<void> {
		throw new Error("Method not implemented.");
	}
	getBit(id: string, hub?: string): Promise<IBit> {
		throw new Error("Method not implemented.");
	}
	addBit(bit: IBit, profile: ISettingsProfile): Promise<void> {
		throw new Error("Method not implemented.");
	}
	removeBit(bit: IBit, profile: ISettingsProfile): Promise<void> {
		throw new Error("Method not implemented.");
	}
	getPackSize(bits: IBit[]): Promise<number> {
		throw new Error("Method not implemented.");
	}
	getBitSize(bit: IBit): Promise<number> {
		throw new Error("Method not implemented.");
	}
	searchBits(type: IBitSearchQuery): Promise<IBit[]> {
		throw new Error("Method not implemented.");
	}
	isBitInstalled(bit: IBit): Promise<boolean> {
		throw new Error("Method not implemented.");
	}
}
