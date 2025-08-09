"use client";

import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Button, useBackend, useHub, useInvoke } from "@tm9657/flow-like-ui";
import { Amplify } from "aws-amplify";
import {
	type AuthTokens,
	type TokenProvider,
	type UpdateUserAttributesInput,
	decodeJWT,
	fetchAuthSession,
	fetchMFAPreference,
	fetchUserAttributes,
	getCurrentUser,
	updatePassword,
	updateUserAttributes,
} from "aws-amplify/auth";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { type AuthContextProps, useAuth } from "react-oidc-context";
import { toast } from "sonner";
import { fetcher } from "../../lib/api";
import { type ProfileActions, ProfilePage } from "./account";
import ChangeEmailDialog from "./change-email";
import ChangePasswordDialog from "./change-password";

class AuthTokenProvider implements TokenProvider {
	constructor(private readonly authContext: AuthContextProps) {}

	async getTokens(options?: {
		forceRefresh?: boolean;
	}): Promise<AuthTokens | null> {
		if (!this.authContext.isAuthenticated || !this.authContext.user) {
			return null;
		}

		if (
			!this.authContext.user.access_token ||
			!this.authContext.user.id_token
		) {
			return null;
		}

		const accessToken = decodeJWT(this.authContext.user?.access_token || "");
		const idToken = decodeJWT(this.authContext.user?.id_token || "");

		return {
			accessToken: accessToken,
			idToken: idToken,
		};
	}
}

const AccountPage: React.FC = () => {
	const backend = useBackend();
	const hub = useHub();
	const auth = useAuth();
	const router = useRouter();
	const [passwordDialogOpen, setPasswordDialogOpen] = useState(false);
	const [emailDialogOpen, setEmailDialogOpen] = useState(false);
	const [cognito, setCognito] = useState(false);
	const [federated, setFederated] = useState(false);
	const profile = useInvoke(
		backend.userState.getProfile,
		backend.userState,
		[],
	);

	const updateUserAttribute = useCallback(
		async (attributeKey: string, value: string) => {
			if (!cognito) {
				console.warn(
					"Cognito is not enabled, skipping attribute update ",
					cognito,
				);
				return;
			}
			try {
				const updateInput: UpdateUserAttributesInput = {
					userAttributes: {
						[attributeKey]: value,
					},
				};

				await updateUserAttributes(updateInput);
			} catch (error) {
				console.error(`Failed to update ${attributeKey}:`, error);
				throw error;
			}
		},
		[cognito],
	);

	useEffect(() => {
		setProfileActions((prev) => ({
			...prev,
			handleAttributeUpdate: cognito ? updateUserAttribute : undefined,
		}));
	}, [cognito]);

	const handleChangePassword = useCallback(async () => {
		setPasswordDialogOpen(true);
	}, []);

	const handleUpdateEmail = useCallback(async (email: string) => {
		setEmailDialogOpen(true);
	}, []);

	const configureAmplify = useCallback(async () => {
		if (!auth.isAuthenticated || !auth.user?.profile) return;
		if (hub.hub?.authentication?.openid?.cognito?.user_pool_id) {
			const provider = new AuthTokenProvider(auth);
			Amplify.configure(
				{
					Auth: {
						Cognito: {
							userPoolClientId: auth.settings.client_id,
							userPoolId: hub.hub.authentication.openid.cognito.user_pool_id,
						},
					},
				},
				{
					Auth: {
						tokenProvider: provider,
					},
				},
			);

			const currentUser = await getCurrentUser();
			const attributes = await fetchUserAttributes();
			const authSession = await fetchAuthSession();
			const mfaPreferences = await fetchMFAPreference();

			console.dir({
				currentUser,
				attributes,
				authSession,
				mfaPreferences,
			});

			const isFederated = Array.isArray(
				authSession.tokens?.idToken?.payload?.identities,
			);
			setFederated(isFederated);
			setCognito(true);

			setProfileActions((prev) => ({
				...prev,
				changePassword: isFederated ? undefined : handleChangePassword,
				updateEmail: isFederated ? undefined : handleUpdateEmail,
			}));
		}
	}, [
		hub,
		auth.settings.client_id,
		auth.isAuthenticated,
		auth.user?.profile,
		auth,
		handleChangePassword,
		handleUpdateEmail,
	]);

	useEffect(() => {
		configureAmplify();
	}, [auth.isAuthenticated, hub.hub]);

	const handlePasswordChange = useCallback(
		async (currentPassword: string, newPassword: string) => {
			try {
				await updatePassword({
					oldPassword: currentPassword,
					newPassword: newPassword,
				});

				setPasswordDialogOpen(false);
				toast.success("Password updated successfully");
			} catch (error) {
				console.error("Failed to update password:", error);
				toast.error("Failed to update password");
				throw error;
			}
		},
		[toast],
	);

	const handleViewBilling = useCallback(async () => {
		if (!profile.data) {
			toast.error("Profile data not available");
			return;
		}

		const urlRequest = await fetcher<{ url: string }>(
			profile.data,
			"user/billing",
			{ method: "GET" },
			auth,
		);

		const _view = new WebviewWindow("billing", {
			url: urlRequest.url,
			title: "Billing",
			focus: true,
			resizable: true,
			maximized: true,
			contentProtected: true,
		});
	}, [router, profile]);

	const handlePreviewProfile = useCallback(async () => {
		router.push(`/profile?sub=${auth.user?.profile?.sub}`);
	}, [router, auth.user?.profile?.sub]);

	const [profileActions, setProfileActions] = useState<ProfileActions>({
		updateEmail: undefined,
		changePassword: undefined,
		viewBilling: handleViewBilling,
		previewProfile: handlePreviewProfile,
		handleAttributeUpdate: updateUserAttribute,
	});

	if (!auth.isAuthenticated) {
		return (
			<main className="flex flex-row items-center justify-center h-screen w-full">
				<div className="text-center p-6 border rounded-lg shadow-lg bg-card">
					<h3>Please log in to view your profile.</h3>
					<Button onClick={() => auth.signinRedirect()} className="mt-4">
						Log In
					</Button>
				</div>
			</main>
		);
	}

	return (
		<>
			<p>
				{cognito ? <span>Cognito User</span> : <span>Non-Cognito User</span>}
			</p>
			<ProfilePage actions={profileActions} />

			{!federated && (
				<ChangePasswordDialog
					key={auth.user?.profile?.sub + "password"}
					open={passwordDialogOpen}
					onOpenChange={setPasswordDialogOpen}
					onPasswordChange={handlePasswordChange}
				/>
			)}

			{!federated && (
				<ChangeEmailDialog
					open={emailDialogOpen}
					onOpenChange={setEmailDialogOpen}
				/>
			)}
		</>
	);
};

export default AccountPage;
