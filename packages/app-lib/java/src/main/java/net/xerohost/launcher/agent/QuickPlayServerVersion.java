package net.xerohost.launcher.agent;

// Must be kept up-to-date with quick_play_version.rs
public enum QuickPlayServerVersion {
    BUILTIN,
    BUILTIN_LEGACY,
    INJECTED,
    UNSUPPORTED;

    public static final QuickPlayServerVersion CURRENT =
            valueOf(System.getProperty("launcher.internal.quickPlay.serverVersion"));
}
