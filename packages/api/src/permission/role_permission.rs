use bitflags::bitflags;

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct RolePermissions: i64 {
        const Owner             =   0b00000000_00000000_00000001;
        const Admin             =   0b00000000_00000000_00000010;
        const ReadTeam          =   0b00000000_00000000_00000100;
        const ReadRoles         =   0b00000000_00000000_00001000;
        const ReadFiles         =   0b00000000_00000000_00010000;
        const WriteFiles        =   0b00000000_00000000_00100000;
        const InvokeApi         =   0b00000000_00000000_01000000;
        const WriteMeta         =   0b00000000_00000000_10000000;
        const ReadBoards        =   0b00000000_00000001_00000000;
        const ExecuteBoards     =   0b00000000_00000010_00000000;
        const WriteBoards       =   0b00000000_00000100_00000000;
        const ListReleases      =   0b00000000_00001000_00000000;
        const ReadReleases      =   0b00000000_00010000_00000000;
        const ExecuteReleases   =   0b00000000_00100000_00000000;
        const WriteReleases     =   0b00000000_01000000_00000000;
        const ReadLogs          =   0b00000000_10000000_00000000;
        const ReadAnalytics     =   0b00000001_00000000_00000000;
        const ReadConfig        =   0b00000010_00000000_00000000;
        const WriteConfig       =   0b00000100_00000000_00000000;
        const ReadTemplates     =   0b00001000_00000000_00000000;
        const WriteTemplates    =   0b00010000_00000000_00000000;
    }
}
