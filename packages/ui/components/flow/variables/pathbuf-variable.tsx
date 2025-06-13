import { FileIcon, FolderIcon } from "lucide-react";
import { useState } from "react";
import { Button } from "../../../components/ui/button";
import { Label } from "../../../components/ui/label";
import { Switch } from "../../../components/ui/switch";
import type { IVariable } from "../../../lib/schema/flow/variable";
import {
	convertJsonToUint8Array,
	parseUint8ArrayToJson,
} from "../../../lib/uint8";
import { cn } from "../../../lib/utils";
import { useBackend } from "../../../state/backend-state";

export function PathbufVariable({
	disabled,
	variable,
	onChange,
}: Readonly<{
	disabled?: boolean;
	variable: IVariable;
	onChange: (variable: IVariable) => void;
}>) {
	const backend = useBackend();
	const [fileOrFolder, setFileOrFolder] = useState<string | undefined>(
		parseUint8ArrayToJson(variable.default_value),
	);
	const [isFolder, setIsFolder] = useState<boolean>(
		!parseUint8ArrayToJson(variable.default_value)?.includes("."),
	);

	return (
		<div className="grid w-full max-w-full grid-cols-6">
			<div className="flex items-center space-x-2 max-w-full overflow-hidden col-span-2">
				<Switch
					disabled={disabled}
					checked={isFolder}
					onCheckedChange={(checked) => {
						setIsFolder(checked);
					}}
					id="is_folder"
				/>
				<Label htmlFor="is_folder">Folder</Label>
			</div>
			<Button
				variant={"outline"}
				disabled={disabled}
				className={cn(
					"w-full justify-start text-left font-normal max-w-full col-span-4",
					(!fileOrFolder || fileOrFolder?.length === 0) &&
						"text-muted-foreground",
				)}
				onClick={async () => {
					const pathBuf: any = await backend.openFileOrFolderMenu(
						false,
						isFolder,
						true,
					);
					if (!pathBuf) return;

					if (!isFolder) {
						console.dir(pathBuf);
						const fileMetadata = await backend.getPathMeta(pathBuf);
						if (!fileMetadata || fileMetadata.length === 0) return;
						setFileOrFolder(fileMetadata[0].file_path);
						onChange({
							...variable,
							default_value: convertJsonToUint8Array(fileMetadata[0].file_path),
						});
						return;
					}

					setFileOrFolder(pathBuf);
					onChange({
						...variable,
						default_value: convertJsonToUint8Array(pathBuf),
					});
				}}
			>
				{isFolder && <FolderIcon className="mr-2 min-w-4 h-4 w-4" />}
				{!isFolder && <FileIcon className="mr-2 min-w-4 h-4 w-4" />}
				{isFolder &&
					(fileOrFolder && fileOrFolder.length > 0 ? (
						<span className="text-nowrap truncate">
							{fileOrFolder.split("/").pop()}
						</span>
					) : (
						<span>Pick a folder</span>
					))}
				{!isFolder &&
					(fileOrFolder && fileOrFolder.length > 0 ? (
						<span className="text-nowrap truncate">{fileOrFolder}</span>
					) : (
						<span>Pick a file</span>
					))}
			</Button>
		</div>
	);
}
