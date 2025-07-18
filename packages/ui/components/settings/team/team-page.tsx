"use client";

import { Clock, UserPlus, Users } from "lucide-react";
import { useSearchParams } from "next/navigation";
import { useState } from "react";
import { Badge, Tabs, TabsContent, TabsList, TabsTrigger } from "../../../";
import { InviteManagement } from "./invite-managment";
import { TeamJoinManagement } from "./join-management";
import { UserManagement } from "./user-managements";

interface JoinRequest {
	id: string;
	name: string;
	email: string;
	avatar?: string;
	requestedAt: string;
	message?: string;
}

export function TeamManagementPage() {
	const searchParams = useSearchParams();
	const appId = searchParams.get("id");
	const [showRequestQueue] = useState(true); // This would be determined by project type

	return (
		<div className="container mx-auto p-6 space-y-8 flex flex-col overflow-hidden h-full flex-grow">
			{/* Header */}
			<div className="flex items-center justify-between">
				<div>
					<h1 className="text-4xl font-bold bg-gradient-to-r from-primary to-tertiary bg-clip-text text-transparent">
						Team Management
					</h1>
					<p className="text-muted-foreground mt-2">
						Manage your team members, roles, and collaboration settings
					</p>
				</div>
				<div className="flex items-center gap-3">
					<Badge variant="secondary" className="px-3 py-1">
						<Users className="w-4 h-4 mr-1" />0 members
					</Badge>
				</div>
			</div>

			<Tabs
				defaultValue="members"
				className="space-y-6 flex flex-col flex-1 min-h-0"
			>
				<TabsList className="grid w-full grid-cols-3 flex-shrink-0">
					<TabsTrigger value="members" className="flex items-center gap-2">
						<Users className="w-4 h-4" />
						Team Members
					</TabsTrigger>
					<TabsTrigger value="invite" className="flex items-center gap-2">
						<UserPlus className="w-4 h-4" />
						Invite & Access
					</TabsTrigger>
					{showRequestQueue && (
						<TabsTrigger value="requests" className="flex items-center gap-2">
							<Clock className="w-4 h-4" />
							Join Requests
						</TabsTrigger>
					)}
				</TabsList>

				{/* Team Members Tab */}
				{appId && (
					<TabsContent value="members" className="flex-1 min-h-0">
						<div className="h-full overflow-y-auto">
							<UserManagement appId={appId} />
						</div>
					</TabsContent>
				)}

				{/* Invite & Access Tab */}
				{appId && (
					<TabsContent value="invite" className="flex-1 min-h-0">
						<div className="h-full overflow-y-auto">
							<InviteManagement appId={appId} />
						</div>
					</TabsContent>
				)}

				{/* Join Requests Tab */}
				{showRequestQueue && appId && (
					<TabsContent value="requests" className="flex-1 min-h-0">
						<TeamJoinManagement appId={appId} />
					</TabsContent>
				)}
			</Tabs>
		</div>
	);
}
