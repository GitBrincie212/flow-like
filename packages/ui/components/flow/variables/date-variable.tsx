import { format } from "date-fns";
import { CalendarIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Calendar } from "../../../components/ui/calendar";
import { cn } from "../../../lib";
import type { ISystemTime } from "../../../lib/schema/flow/board";
import type { IVariable } from "../../../lib/schema/flow/variable";
import {
	convertJsonToUint8Array,
	parseUint8ArrayToJson,
} from "../../../lib/uint8";
import {
	Button,
	Input,
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "../../ui";

export function DateVariable({
	disabled,
	variable,
	onChange,
}: Readonly<{
	disabled?: boolean;
	variable: IVariable;
	onChange: (variable: IVariable) => void;
}>) {
	const parsed = parseUint8ArrayToJson(variable.default_value);

	const defaultDate =
		parsed && (parsed.secs_since_epoch > 0 || parsed.nanos_since_epoch > 0)
			? new Date(parsed.nanos_since_epoch / 1_000_000)
			: new Date();

	const [selected, setSelected] = useState<Date>(defaultDate);
	const [timeValue, setTimeValue] = useState<string>(
		format(defaultDate, "HH:mm"),
	);

	useEffect(() => {
		if (!selected) return;

		console.log("Selected date:", selected);
		console.log("Time value:", timeValue);

		const [hours, minutes] = timeValue
			.split(":")
			.map((str) => Number.parseInt(str, 10));

		const newDate = new Date(
			selected.getFullYear(),
			selected.getMonth(),
			selected.getDate(),
			hours,
			minutes,
		);

		console.log("New date:", newDate);

		const rustDate: ISystemTime = {
			secs_since_epoch: newDate.getTime() / 1000,
			nanos_since_epoch: newDate.getTime() * 1000000,
		};

		onChange({
			...variable,
			default_value: convertJsonToUint8Array(rustDate),
		});
	}, [selected, timeValue]);

	return (
		<Popover>
			<PopoverTrigger disabled={disabled} asChild>
				<Button
					disabled={disabled}
					variant={"outline"}
					className={cn(
						"w-full pl-3 text-left font-normal",
						!selected && "text-muted-foreground",
					)}
				>
					{selected ? (
						`${format(selected, "PPP")} - ${timeValue}`
					) : (
						<span>Pick a date</span>
					)}
					<CalendarIcon className="ml-auto h-4 w-4 opacity-50" />
				</Button>
			</PopoverTrigger>
			<PopoverContent className="w-auto p-2">
				<div className="flex flex-col items-center space-x-2 gap-2">
					<div className="flex flex-row items-center gap-2">
						<p className="text-nowrap">Time:</p>
						<Input
							disabled={disabled}
							type="time"
							value={timeValue}
							onChange={(e) => setTimeValue(e.target.value)}
						/>
					</div>
					<Calendar
						disabled={disabled}
						showOutsideDays
						ISOWeek
						captionLayout="dropdown"
						mode="single"
						selected={selected}
						onSelect={(date) => {
							setSelected(date ?? new Date());
						}}
						className="w-full"
					/>
				</div>
			</PopoverContent>
		</Popover>
	);
}
