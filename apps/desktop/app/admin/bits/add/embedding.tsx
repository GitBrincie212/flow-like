import {
	Badge,
	Button,
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
	type IBit,
	type IEmbeddingModelParameters,
	Input,
	Label,
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
	Slider,
	Textarea,
} from "@tm9657/flow-like-ui";
import { X } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";

export function EmbeddingConfiguration({
	bit,
	setBit,
}: { bit: IBit; setBit: Dispatch<SetStateAction<IBit>> }) {
	const parameters = bit.parameters as IEmbeddingModelParameters;

	const updateParameters = (updates: Partial<IEmbeddingModelParameters>) => {
		setBit((prev) => ({
			...prev,
			parameters: {
				...prev.parameters,
				...updates,
			},
		}));
	};

	const addLanguage = (language: string) => {
		if (language && !parameters.languages.includes(language)) {
			updateParameters({
				languages: [...parameters.languages, language],
			});
		}
	};

	const removeLanguage = (language: string) => {
		updateParameters({
			languages: parameters.languages.filter((lang) => lang !== language),
		});
	};

	return (
		<div className="space-y-6 w-full max-w-screen-lg">
			<Card className="w-full">
				<CardHeader>
					<CardTitle>Model Provider</CardTitle>
					<CardDescription>
						Configure the embedding model provider
					</CardDescription>
				</CardHeader>
				<CardContent className="space-y-4">
					<div className="grid grid-cols-3 gap-4">
						<div className="space-y-2">
							<Label htmlFor="provider-name">Provider Name *</Label>
							<Select
								value={parameters?.provider?.provider_name || "Local"}
								onValueChange={(value) =>
									updateParameters({
										provider: {
											...parameters.provider,
											provider_name: value,
										},
									})
								}
							>
								<SelectTrigger id="provider-name">
									<SelectValue placeholder="Select provider" />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="Local">Local</SelectItem>
									<SelectItem value="Premium">Premium</SelectItem>
								</SelectContent>
							</Select>
						</div>
						<div className="space-y-2">
							<Label htmlFor="model-id">Model ID</Label>
							<Input
								id="model-id"
								disabled={parameters.provider.provider_name === "Local"}
								value={parameters.provider.model_id || ""}
								onChange={(e) =>
									updateParameters({
										provider: {
											...parameters.provider,
											model_id: e.target.value || null,
										},
									})
								}
								placeholder="e.g., text-embedding-ada-002"
							/>
						</div>
						<div className="space-y-2">
							<Label htmlFor="version">Version</Label>
							<Input
								disabled={parameters.provider.provider_name === "Local"}
								id="version"
								value={parameters.provider.version || ""}
								onChange={(e) =>
									updateParameters({
										provider: {
											...parameters.provider,
											version: e.target.value || null,
										},
									})
								}
								placeholder="e.g., v1.0"
							/>
						</div>
					</div>
				</CardContent>
			</Card>

			<Card>
				<CardHeader>
					<CardTitle>Vector Configuration</CardTitle>
					<CardDescription>
						Configure vector dimensions and pooling
					</CardDescription>
				</CardHeader>
				<CardContent className="space-y-4">
					<div className="grid grid-cols-2 gap-4">
						<div className="space-y-2">
							<Label htmlFor="input-length">
								Input Length: {parameters.input_length}
							</Label>
							<div className="flex gap-2 items-center">
								<Slider
									id="input-length"
									min={50}
									max={8192}
									step={100}
									value={[parameters.input_length]}
									onValueChange={(value) =>
										updateParameters({ input_length: value[0] })
									}
									className="flex-1"
								/>
								<Input
									type="number"
									min={50}
									max={8192}
									step={100}
									value={parameters.input_length}
									onChange={(e) => {
										const value = Number.parseInt(e.target.value);
										if (!Number.isNaN(value) && value >= 100 && value <= 8192) {
											updateParameters({ input_length: value });
										}
									}}
									className="w-20"
								/>
							</div>
						</div>
						<div className="space-y-2">
							<Label htmlFor="vector-length">
								Vector Length: {parameters.vector_length}
							</Label>
							<div className="flex gap-2 items-center">
								<Slider
									id="vector-length"
									min={50}
									max={4096}
									step={64}
									value={[parameters.vector_length]}
									onValueChange={(value) =>
										updateParameters({ vector_length: value[0] })
									}
									className="flex-1"
								/>
								<Input
									type="number"
									min={50}
									max={4096}
									step={64}
									value={parameters.vector_length}
									onChange={(e) => {
										const value = Number.parseInt(e.target.value);
										if (!Number.isNaN(value) && value >= 128 && value <= 4096) {
											updateParameters({ vector_length: value });
										}
									}}
									className="w-20"
								/>
							</div>
						</div>
					</div>
					<div className="space-y-2">
						<Label htmlFor="pooling">Pooling Strategy</Label>
						<Select
							value={parameters.pooling}
							onValueChange={(value) =>
								updateParameters({ pooling: value as any })
							}
						>
							<SelectTrigger>
								<SelectValue placeholder="Select pooling strategy" />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="CLS">CLS Token</SelectItem>
								<SelectItem value="Mean">Mean Pooling</SelectItem>
								<SelectItem value="None">No Pooling</SelectItem>
							</SelectContent>
						</Select>
					</div>
				</CardContent>
			</Card>

			<Card>
				<CardHeader>
					<CardTitle>Text Prefixes</CardTitle>
					<CardDescription>
						Configure prefixes for different text types
					</CardDescription>
				</CardHeader>
				<CardContent className="space-y-4">
					<div className="space-y-2">
						<Label htmlFor="query-prefix">Query Prefix</Label>
						<Textarea
							id="query-prefix"
							value={parameters.prefix.query}
							onChange={(e) =>
								updateParameters({
									prefix: {
										...parameters.prefix,
										query: e.target.value,
									},
								})
							}
							placeholder="Prefix for query texts"
							rows={2}
						/>
					</div>
					<div className="space-y-2">
						<Label htmlFor="paragraph-prefix">Paragraph Prefix</Label>
						<Textarea
							id="paragraph-prefix"
							value={parameters.prefix.paragraph}
							onChange={(e) =>
								updateParameters({
									prefix: {
										...parameters.prefix,
										paragraph: e.target.value,
									},
								})
							}
							placeholder="Prefix for paragraph texts"
							rows={2}
						/>
					</div>
				</CardContent>
			</Card>

			<Card>
				<CardHeader>
					<CardTitle>Languages</CardTitle>
					<CardDescription>Configure supported languages</CardDescription>
				</CardHeader>
				<CardContent className="space-y-4">
					<div className="space-y-2">
						<Label htmlFor="add-language">Add Language</Label>
						<div className="flex gap-2">
							<Input
								id="add-language"
								placeholder="e.g., en, de, fr"
								onKeyDown={(e) => {
									if (e.key === "Enter") {
										addLanguage(e.currentTarget.value);
										e.currentTarget.value = "";
									}
								}}
							/>
							<Button
								type="button"
								onClick={() => {
									const input = document.getElementById(
										"add-language",
									) as HTMLInputElement;
									addLanguage(input.value);
									input.value = "";
								}}
							>
								Add
							</Button>
						</div>
					</div>
					<div className="flex flex-wrap gap-2">
						{parameters.languages.map((language) => (
							<Badge
								key={language}
								variant="secondary"
								className="flex items-center gap-1"
							>
								{language}
								<Button
									size="sm"
									variant="ghost"
									className="h-4 w-4 p-0"
									onClick={() => removeLanguage(language)}
								>
									<X className="h-3 w-3" />
								</Button>
							</Badge>
						))}
					</div>
				</CardContent>
			</Card>
		</div>
	);
}
