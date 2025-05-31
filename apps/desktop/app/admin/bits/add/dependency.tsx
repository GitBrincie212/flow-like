import {
	type IBit,
	IMetadata,
	Input,
	Slider,
	Textarea,
	humanFileSize,
} from "@tm9657/flow-like-ui";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@tm9657/flow-like-ui";
import { Label } from "@tm9657/flow-like-ui";
import {
	type Dispatch,
	type SetStateAction,
	useCallback,
	useEffect,
	useState,
} from "react";
import { getModelSize, getOriginalRepo } from "../utils";

export function DependencyConfiguration({
	name,
	defaultBit,
	bit,
	setBit,
}: {
	name: string;
	defaultBit: IBit;
	bit: IBit;
	setBit: Dispatch<SetStateAction<IBit | undefined>>;
}) {
	const prefillData = useCallback(async () => {
		if (!bit.download_link) return;

		// If the bit already has a size, we don't need to fetch it again
		if (bit.size && bit.size > 0) return;
		if (bit.size && bit.size < 0) return;

		const size = await getModelSize(bit.download_link);
		const downloadLink = bit.download_link.trim();
		const fileName = downloadLink.split("/").pop()?.split("?")[0] || "";

		setBit((old) => ({
			...(old ?? defaultBit),
			size: size,
			file_name: old?.file_name === "" ? fileName : old?.file_name,
		}));
	}, [bit]);

	useEffect(() => {
		prefillData();
	}, [prefillData, bit.download_link]);

	return (
		<div className="space-y-6 w-full max-w-screen-lg">
			<Card className="w-full">
				<CardHeader>
					<CardTitle>{name} Configuration</CardTitle>
					<CardDescription>
						Configure the {name} download and metadata
					</CardDescription>
				</CardHeader>
				<CardContent className="space-y-4">
					<div className="space-y-2">
						<Label htmlFor="projection-link">{name} Link *</Label>
						<Input
							id="projection-link"
							value={bit.download_link ?? ""}
							onChange={(e) => {
								const downloadLink = e.target.value.trim();

								setBit((old) => ({
									...(old ?? defaultBit),
									download_link: downloadLink,
									file_name: downloadLink.split("/").pop()?.split("?")[0] || "",
								}));

								if (downloadLink) {
									Promise.all([getModelSize(downloadLink)]).then(([size]) => {
										setBit((old) => ({
											...(old ?? defaultBit),
											size: size,
										}));
									});
								}
							}}
							placeholder="Projection Download Link"
							required
						/>
					</div>
					<div className="space-y-2">
						<Label htmlFor="file-name">File Name</Label>
						<Input
							id="file-name"
							value={bit.file_name ?? ""}
							onChange={(e) => {
								setBit((old) => ({
									...(old ?? defaultBit),
									file_name: e.target.value.trim(),
								}));
							}}
							placeholder="Model file name"
						/>
					</div>
					{typeof bit.size === "number" && bit.size > 0 && (
						<div className="space-y-2">
							<Label className="text-muted-foreground">{name} Size</Label>
							<div className="text-sm bg-muted px-3 py-2 rounded-md">
								{humanFileSize(bit.size)}
							</div>
						</div>
					)}
				</CardContent>
			</Card>
		</div>
	);
}
