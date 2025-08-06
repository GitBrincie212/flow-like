/**
 * Represents a set of role permission flags.
 */
export class RolePermissions {
	private readonly value: bigint;

	// Permission constants
	static readonly Owner = new RolePermissions(0b00000000_00000000_00000001n);
	static readonly Admin = new RolePermissions(0b00000000_00000000_00000010n);
	static readonly ReadTeam = new RolePermissions(0b00000000_00000000_00000100n);
	static readonly ReadRoles = new RolePermissions(
		0b00000000_00000000_00001000n,
	);
	static readonly ReadFiles = new RolePermissions(
		0b00000000_00000000_00010000n,
	);
	static readonly WriteFiles = new RolePermissions(
		0b00000000_00000000_00100000n,
	);
	static readonly InvokeApi = new RolePermissions(
		0b00000000_00000000_01000000n,
	);
	static readonly WriteMeta = new RolePermissions(
		0b00000000_00000000_10000000n,
	);
	static readonly ReadBoards = new RolePermissions(
		0b00000000_00000001_00000000n,
	);
	static readonly ExecuteBoards = new RolePermissions(
		0b00000000_00000010_00000000n,
	);
	static readonly WriteBoards = new RolePermissions(
		0b00000000_00000100_00000000n,
	);
	static readonly ListReleases = new RolePermissions(
		0b00000000_00001000_00000000n,
	);
	static readonly ReadReleases = new RolePermissions(
		0b00000000_00010000_00000000n,
	);
	static readonly ExecuteReleases = new RolePermissions(
		0b00000000_00100000_00000000n,
	);
	static readonly WriteReleases = new RolePermissions(
		0b00000000_01000000_00000000n,
	);
	static readonly ReadLogs = new RolePermissions(0b00000000_10000000_00000000n);
	static readonly ReadAnalytics = new RolePermissions(
		0b00000001_00000000_00000000n,
	);
	static readonly ReadConfig = new RolePermissions(
		0b00000010_00000000_00000000n,
	);
	static readonly WriteConfig = new RolePermissions(
		0b00000100_00000000_00000000n,
	);
	static readonly ReadTemplates = new RolePermissions(
		0b00001000_00000000_00000000n,
	);
	static readonly WriteTemplates = new RolePermissions(
		0b00010000_00000000_00000000n,
	);

	constructor(value: bigint | number = 0n) {
		this.value = typeof value === "number" ? BigInt(value) : value;
	}

	hasPermission(permission: RolePermissions): boolean {
		return (
			this.contains(permission) ||
			this.contains(RolePermissions.Admin) ||
			this.contains(RolePermissions.Owner)
		);
	}

	/**
	 * Check if this permission set contains the specified permission(s)
	 */
	contains(permission: RolePermissions): boolean {
		return (this.value & permission.value) === permission.value;
	}

	/**
	 * Create a new permission set with the specified permission(s) added
	 */
	insert(permission: RolePermissions): RolePermissions {
		return new RolePermissions(this.value | permission.value);
	}

	/**
	 * Create a new permission set with the specified permission(s) removed
	 */
	remove(permission: RolePermissions): RolePermissions {
		return new RolePermissions(this.value & ~permission.value);
	}

	/**
	 * Create a new permission set with the specified permission(s) toggled
	 */
	toggle(permission: RolePermissions): RolePermissions {
		return new RolePermissions(this.value ^ permission.value);
	}

	/**
	 * Check if this permission set is empty
	 */
	isEmpty(): boolean {
		return this.value === 0n;
	}

	/**
	 * Create a new permission set from multiple permissions
	 */
	static from(...permissions: RolePermissions[]): RolePermissions {
		return permissions.reduce(
			(acc, perm) => acc.insert(perm),
			new RolePermissions(),
		);
	}

	/**
	 * Get the raw numeric value
	 */
	toBigInt(): bigint {
		return this.value;
	}

	/**
	 * Convert to number (use with caution for large values)
	 */
	toNumber(): number {
		return Number(this.value);
	}

	/**
	 * Check equality with another permission set
	 */
	equals(other: RolePermissions): boolean {
		return this.value === other.value;
	}

	/**
	 * String representation for debugging
	 */
	toString(): string {
		return `RolePermissions(${this.value.toString(2).padStart(24, "0")})`;
	}
}
